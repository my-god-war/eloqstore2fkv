#!/usr/bin/env bash
# V6 性能门槛：同 runner 上跑 fkv_bench，与 perf-baseline 里的 Rust/C++ 基线比较。
#
# 用法：
#   scripts/perf_gate.sh gate            # CI 默认；下载 perf-latest + perf-baseline 并比较
#   scripts/perf_gate.sh compare BASE CUR# 只比较两份 JSON（BASE=baseline，CUR=fkv history）
#   scripts/perf_gate.sh produce --cpp-log LOG --rust-history JSON --out FILE
#   scripts/perf_gate.sh selftest        # 不依赖硬件，反向验证「慢 30% 必须红」
#
# 缺 perf-baseline 时默认 **失败**（exit 78）。只有显式设置
# `PERF_GATE_REQUIRE_BASELINE=0` 才降级为告警并绿；该开关仅用于临时排障。
# 非 Linux / 非 4 KiB 页 / 非 NVMe 根目录一律 skipped-with-error（exit 78），
# 不会给出绿色结论。
#
# 固定 workload（两侧必须一致，写进 perf-baseline.json 的 workload 字段）：
#   workload=write-read rows=max_key=1000000 batch_size=100 partitions=1
#   read_per_part=4 kv_size=128 key_interval=4 del_ratio=4 test_secs=30
#   write_batchs=rows/batch_size=10000 scan_bytes=0
# C++ 侧对应：
#   simple_bench --workload write-read --kv_size 128 --batch_size 100
#     --write_batchs 10000 --partitions 1 --max_key 1000000 --read_per_part 4
#     --read_thds 1 --test_secs 30 --scan_bytes 0 --show_write_perf=true
#     --kvoptions <append-mode.ini>
# 必须用 data_append_mode=true 的 ini（Rust 恒 append），否则比值无意义。
#
# 已知口径差异（不隐藏）：C++ `simple_bench` 的 write-read 是写/读线程并发；
# fkv_bench 的 write-read 是「先写阶段、再读阶段」的顺序执行。因此
# 「current rust / baseline rust」是同形状回归门槛（主判据），
# 「current rust / baseline cpp」只作同 runner 趋势比值，不承诺绝对值等价。

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MODE=${1:-gate}
if [ "$#" -gt 0 ]; then shift; fi

FKV_REPO=${FKV_REPO:-webc-site/fkv}
PERF_ARCH=${PERF_ARCH:-$(uname -m)}
PERF_LATEST_TAG=${PERF_LATEST_TAG:-perf-latest}
PERF_BASELINE_TAG=${PERF_BASELINE_TAG:-perf-baseline}
PERF_LATEST_ASSET=${PERF_LATEST_ASSET:-fkv_bench-linux-$PERF_ARCH}
PERF_BASELINE_ASSET=${PERF_BASELINE_ASSET:-perf-baseline.json}
PERF_RUNNER_ID=${PERF_RUNNER_ID:-linux-$PERF_ARCH}
PERF_NVME_ROOT=${PERF_NVME_ROOT:-}
PERF_GATE_REQUIRE_BASELINE=${PERF_GATE_REQUIRE_BASELINE:-1}

PERF_WORKLOAD=${PERF_WORKLOAD:-write-read}
PERF_ROWS=${PERF_ROWS:-1000000}
PERF_MAX_KEY=${PERF_MAX_KEY:-1000000}
PERF_BATCH_SIZE=${PERF_BATCH_SIZE:-100}
PERF_READ_PER_PART=${PERF_READ_PER_PART:-4}
PERF_PARTITIONS=${PERF_PARTITIONS:-1}
PERF_KV_SIZE=${PERF_KV_SIZE:-128}
PERF_KEY_INTERVAL=${PERF_KEY_INTERVAL:-4}
PERF_DEL_RATIO=${PERF_DEL_RATIO:-4}
PERF_TEST_SECS=${PERF_TEST_SECS:-30}
PERF_SCAN_BYTES=${PERF_SCAN_BYTES:-0}
PERF_WRITE_BATCHS=${PERF_WRITE_BATCHS:-$((PERF_ROWS / PERF_BATCH_SIZE))}
if [ "$PERF_WRITE_BATCHS" -lt 1 ]; then PERF_WRITE_BATCHS=1; fi

PERF_MAX_REGRESSION_PCT=${PERF_MAX_REGRESSION_PCT:-5}
PERF_MAX_LATENCY_REGRESSION_PCT=${PERF_MAX_LATENCY_REGRESSION_PCT:-10}
PERF_MAX_RSS_REGRESSION_PCT=${PERF_MAX_RSS_REGRESSION_PCT:-10}

skip_with_error() {
  local msg=$1
  echo "::error title=perf-gate skipped-with-error:: $msg" >&2
  echo "PERF GATE SKIPPED-WITH-ERROR: $msg" >&2
  exit 78
}

die() {
  echo "::error title=perf-gate:: $*" >&2
  echo "PERF GATE FAILED: $*" >&2
  exit 1
}

require_linux() {
  if [ "$(uname -s)" != "Linux" ]; then
    skip_with_error "perf gate 只能在 Linux 上跑（当前 $(uname -s)）；本机是 Apple Silicon 时请留给 CI 的 4 KiB NVMe runner"
  fi
}

require_4k_nvme() {
  local page
  page=$(getconf PAGESIZE 2>/dev/null || echo 0)
  if [ "$page" != "4096" ]; then
    skip_with_error "需要 4 KiB OS 页，当前 PAGESIZE=$page；跨页大小的 manifest framing 不可互读，也不能作性能基线"
  fi
  if [ -z "$PERF_NVME_ROOT" ]; then
    skip_with_error "PERF_NVME_ROOT 未设置；需要 4 KiB 页的 NVMe 挂载点（例如 /mnt/nvme0n1）"
  fi
  if [ ! -d "$PERF_NVME_ROOT" ]; then
    skip_with_error "PERF_NVME_ROOT 不存在: $PERF_NVME_ROOT"
  fi
  local fstype source tran
  source=$(findmnt -T "$PERF_NVME_ROOT" -no SOURCE 2>/dev/null || true)
  tran=$(findmnt -T "$PERF_NVME_ROOT" -no TRAN 2>/dev/null || true)
  fstype=$(findmnt -T "$PERF_NVME_ROOT" -no FSTYPE 2>/dev/null || true)
  if [ -z "$fstype" ]; then
    fstype=$(stat -f -c %T "$PERF_NVME_ROOT" 2>/dev/null || echo unknown)
  fi
  case "$fstype" in
    tmpfs|ramfs|overlay|nfs*|cifs|fuse*)
      skip_with_error "PERF_NVME_ROOT 在 $fstype 上，不是本地 NVMe；性能基线不可用"
      ;;
  esac
  if [ -z "$source" ]; then
    source=$(df --output=source "$PERF_NVME_ROOT" 2>/dev/null | tail -n 1 || true)
  fi
  if [ -z "$tran" ]; then
    # 容器 / overlay 下 findmnt 常拿不到 TRAN，但 SOURCE 仍是宿主 NVMe。
    case "$(readlink -f "$source" 2>/dev/null || echo "$source")" in
      /dev/nvme*) tran=nvme ;;
      *) skip_with_error "无法确认 $PERF_NVME_ROOT 的传输层（findmnt TRAN 为空，source=${source:-unknown}）；需要明确的 nvme 设备" ;;
    esac
  fi
  if [ "$tran" != "nvme" ]; then
    skip_with_error "PERF_NVME_ROOT 的传输层是 $tran（$source），需要 nvme"
  fi
  local base rota
  base=$(basename "$(readlink -f "$source" 2>/dev/null || echo "$source")")
  # 分区设备（nvme0n1p1）没有自己的 queue 属性，取整盘名。
  if [ ! -r "/sys/block/${base}/queue/rotational" ]; then
    local parent
    parent=$(lsblk -no PKNAME "$source" 2>/dev/null | head -n 1 || true)
    if [ -n "$parent" ]; then
      base=$parent
    fi
  fi
  if [ -n "$base" ] && [ -r "/sys/block/${base}/queue/rotational" ]; then
    rota=$(cat "/sys/block/${base}/queue/rotational")
  else
    rota=""
  fi
  if [ -n "$rota" ] && [ "$rota" != "0" ]; then
    skip_with_error "PERF_NVME_ROOT 落在 rotational=$rota 的磁盘（$base），需要 NVMe SSD"
  fi
  echo "hardware: pagesize=$page fstype=$fstype source=$source dev=$base rotational=${rota:-unknown} tran=$tran"
}

gh_download() {
  local tag=$1 asset=$2 dest=$3
  if command -v gh >/dev/null 2>&1; then
    if gh release download "$tag" -R "$FKV_REPO" -p "$asset" -D "$dest" --clobber 2>/dev/null; then
      return 0
    fi
  fi
  local url="https://github.com/${FKV_REPO}/releases/download/${tag}/${asset}"
  if curl -fsSL -o "${dest}/${asset}" "$url"; then
    return 0
  fi
  return 1
}

run_gate() {
  require_linux
  require_4k_nvme

  GATE_TMP=$(mktemp -d "${TMPDIR:-/tmp}/fkv-perf-gate.XXXXXX")
  GATE_BENCH_DIR="${PERF_NVME_ROOT}/fkv-perf-$$"
  mkdir -p "$GATE_BENCH_DIR"
  trap 'rm -rf "$GATE_TMP" "$GATE_BENCH_DIR"' EXIT

  local baseline="${GATE_TMP}/${PERF_BASELINE_ASSET}"
  local optional=0
  case "$PERF_GATE_REQUIRE_BASELINE" in
    0|false|False|FALSE|no|off) optional=1 ;;
  esac
  if ! gh_download "$PERF_BASELINE_TAG" "$PERF_BASELINE_ASSET" "$GATE_TMP"; then
    if [ "$optional" = "1" ]; then
      echo "::warning title=perf-gate:: perf-baseline 缺失；PERF_GATE_REQUIRE_BASELINE=0 显式降级，跳过比较" >&2
      echo "PERF GATE DEGRADED: baseline missing, explicitly allowed by PERF_GATE_REQUIRE_BASELINE=0"
      return 0
    fi
    skip_with_error "缺少 ${PERF_BASELINE_TAG}/${PERF_BASELINE_ASSET}（${FKV_REPO}）。默认必须阻断；仅排障时可显式设 PERF_GATE_REQUIRE_BASELINE=0 降级"
  fi

  if ! gh_download "$PERF_LATEST_TAG" "$PERF_LATEST_ASSET" "$GATE_TMP"; then
    die "缺少 ${PERF_LATEST_TAG}/${PERF_LATEST_ASSET}（${FKV_REPO}）；先让 fkv release workflow 产出滚动二进制"
  fi
  local bench="${GATE_TMP}/${PERF_LATEST_ASSET}"
  chmod +x "$bench"

  local current="${GATE_TMP}/current.json"
  echo "== run fkv_bench (${PERF_LATEST_ASSET}) =="
  "$bench" \
    --impl rust \
    --root "$GATE_BENCH_DIR/run" \
    --workload "$PERF_WORKLOAD" \
    --rows "$PERF_ROWS" \
    --max-key "$PERF_MAX_KEY" \
    --batch-size "$PERF_BATCH_SIZE" \
    --read-per-part "$PERF_READ_PER_PART" \
    --partitions "$PERF_PARTITIONS" \
    --kv-size "$PERF_KV_SIZE" \
    --key-interval "$PERF_KEY_INTERVAL" \
    --del-ratio "$PERF_DEL_RATIO" \
    --write-batchs "$PERF_WRITE_BATCHS" \
    --scan-bytes "$PERF_SCAN_BYTES" \
    --test-secs "$PERF_TEST_SECS" \
    --history "$current"

  local base_runner
  base_runner=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8")).get("runner",""))' "$baseline")
  if [ "$base_runner" != "$PERF_RUNNER_ID" ]; then
    die "baseline runner=${base_runner:-<unset>} 与当前 PERF_RUNNER_ID=$PERF_RUNNER_ID 不一致；跨 runner 比值不可比，拒绝给出结论"
  fi

  if [ -n "${PERF_GATE_OUTPUT_DIR:-}" ]; then
    mkdir -p "$PERF_GATE_OUTPUT_DIR"
    cp "$baseline" "$PERF_GATE_OUTPUT_DIR/baseline.json"
    cp "$current" "$PERF_GATE_OUTPUT_DIR/current.json"
  fi

  bash "$0" compare "$baseline" "$current"
}

compare_json() {
  local baseline=$1 current=$2
  python3 - "$baseline" "$current" <<'PY'
import json
import os
import sys

baseline_path, current_path = sys.argv[1], sys.argv[2]
base = json.load(open(baseline_path, encoding="utf-8"))
cur_data = json.load(open(current_path, encoding="utf-8"))

if base.get("schema") != 1:
    raise SystemExit(f"baseline schema != 1: {base.get('schema')!r}")

def fail(msg):
    print(f"PERF GATE FAILED: {msg}", file=sys.stderr)
    raise SystemExit(1)

expected_workload = base.get("workload") or {}
workload_keys = [
    "workload", "rows", "max_key", "batch_size", "read_per_part",
    "partitions", "kv_size", "key_interval", "del_ratio", "test_secs",
    "write_batchs", "scan_bytes",
]
mismatch = []
for key in workload_keys:
    if key in expected_workload:
        got = cur_data.get(key)
        if got != expected_workload[key]:
            mismatch.append(f"{key}: baseline={expected_workload[key]!r} current={got!r}")
if mismatch:
    fail("workload 不一致（同 runner/同 workload 是门槛前提）：" + "; ".join(mismatch))

cur = {run["metric"]: float(run["value"]) for run in cur_data.get("runs", []) if run.get("impl") == "rust"}
base_rust = {k: float(v) for k, v in (base.get("rust", {}).get("metrics") or {}).items()}
base_cpp = {k: float(v) for k, v in (base.get("cpp", {}).get("metrics") or {}).items()}
base_ratio = {k: float(v) for k, v in (base.get("ratios") or {}).items()}

tput_pct = float(os.environ.get("PERF_MAX_REGRESSION_PCT", "5"))
lat_pct = float(os.environ.get("PERF_MAX_LATENCY_REGRESSION_PCT", "10"))
rss_pct = float(os.environ.get("PERF_MAX_RSS_REGRESSION_PCT", "10"))

def lower_better(name):
    return name == "rss_peak_kb" or (name.endswith("_us") and any(
        token in name for token in ("avg", "p50", "p90", "p99", "max")
    ))

errors = []
rows = []
for metric in sorted(base_rust):
    if metric not in cur:
        errors.append(f"current 缺少指标 {metric}")
        continue
    base_value = base_rust[metric]
    value = cur[metric]
    if base_value <= 0:
        continue
    if metric == "rss_peak_kb":
        pct = rss_pct
        limit = base_value * (1 + pct / 100.0)
        ok = value <= limit
        bound = f"<= {limit:.1f}"
    elif lower_better(metric):
        pct = lat_pct
        limit = base_value * (1 + pct / 100.0)
        ok = value <= limit
        bound = f"<= {limit:.1f}"
    else:
        pct = tput_pct
        limit = base_value * (1 - pct / 100.0)
        ok = value >= limit
        bound = f">= {limit:.1f}"
    delta = (value - base_value) / base_value * 100.0
    rows.append((metric, base_value, value, delta, bound, ok))
    if not ok:
        errors.append(
            f"{metric}: baseline={base_value:.2f} current={value:.2f} "
            f"({delta:+.1f}%) 超过阈值 {bound}"
        )

# 同 runner 相对比值：current rust / baseline cpp 不得低于 baseline ratio 的容差。
for metric, base_r in sorted(base_ratio.items()):
    if metric.endswith("_us") or metric == "rss_peak_kb":
        # 延迟 / RSS 是 lower-is-better；比值方向与吞吐相反，不放进 throughput 比值门槛。
        continue
    cpp_value = base_cpp.get(metric)
    if not cpp_value or cpp_value <= 0 or metric not in cur:
        continue
    cur_r = cur[metric] / cpp_value
    limit = base_r * (1 - tput_pct / 100.0)
    ok = cur_r >= limit
    delta = (cur_r - base_r) / base_r * 100.0 if base_r else 0.0
    rows.append((f"{metric}/cpp", base_r, cur_r, delta, f">= {limit:.4f}", ok))
    if not ok:
        errors.append(
            f"{metric}/cpp: baseline={base_r:.4f} current={cur_r:.4f} "
            f"({delta:+.1f}%) 超过阈值 >= {limit:.4f}"
        )

print(f"{'metric':<24} {'baseline':>12} {'current':>12} {'delta%':>8}  limit")
for metric, base_value, value, delta, bound, ok in rows:
    mark = "ok" if ok else "FAIL"
    print(f"{metric:<24} {base_value:>12.2f} {value:>12.2f} {delta:>+8.1f}  {bound}  {mark}")

if errors:
    print("\nregressions:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    raise SystemExit(1)
print(
    f"\nperf gate ok: runner={base.get('runner')} "
    f"tput_tol={tput_pct}% lat_tol={lat_pct}% metrics={len(rows)}"
)
PY
}

run_compare() {
  local baseline=${1:?baseline json}
  local current=${2:?current json}
  PERF_MAX_REGRESSION_PCT="$PERF_MAX_REGRESSION_PCT" \
  PERF_MAX_LATENCY_REGRESSION_PCT="$PERF_MAX_LATENCY_REGRESSION_PCT" \
  PERF_MAX_RSS_REGRESSION_PCT="$PERF_MAX_RSS_REGRESSION_PCT" \
    compare_json "$baseline" "$current"
}

run_produce() {
  local cpp_log="" rust_history="" out=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --cpp-log) cpp_log=$2; shift 2 ;;
      --rust-history) rust_history=$2; shift 2 ;;
      --out) out=$2; shift 2 ;;
      *) die "produce: unknown argument $1" ;;
    esac
  done
  [ -n "$cpp_log" ] || die "produce: --cpp-log required"
  [ -n "$rust_history" ] || die "produce: --rust-history required"
  [ -n "$out" ] || die "produce: --out required"
  [ -f "$cpp_log" ] || die "produce: cpp log not found: $cpp_log"
  [ -f "$rust_history" ] || die "produce: rust history not found: $rust_history"
  PERF_RUNNER_ID="$PERF_RUNNER_ID" python3 - "$cpp_log" "$rust_history" "$out" <<'PY'
import json
import os
import re
import sys
from datetime import datetime, timezone

cpp_log_path, rust_history_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(cpp_log_path, encoding="utf-8").read()

cpp = {}
m = re.search(
    r"write summary (\d+) kvs/s .*?average latency (\d+) microseconds "
    r".*?p99 (\d+) microseconds .*?p99\.9 (\d+) microseconds",
    text,
)
if m:
    cpp["put_kv_s"] = float(m.group(1))
    cpp["put_avg_us"] = float(m.group(2))
    cpp["put_p99_us"] = float(m.group(3))
    cpp["put_p999_us"] = float(m.group(4))

read_qps = [float(x) for x in re.findall(r"read average (\d+) QPS", text)]
if read_qps:
    cpp["get_kv_s"] = sum(read_qps)
read_p99 = [float(x) for x in re.findall(r"read speed .*?p99 (\d+) microseconds", text)]
if read_p99:
    cpp["get_p99_us"] = max(read_p99)

scan_kvs = [float(x) for x in re.findall(r"scan throughput \d+ MB/s (\d+) kvs/s", text)]
if scan_kvs:
    cpp["scan_kv_s"] = sum(scan_kvs)
scan_p99 = [float(x) for x in re.findall(r"scan speed .*?p99 (\d+) microseconds", text)]
if scan_p99:
    cpp["scan_p99_us"] = max(scan_p99)

if not cpp:
    raise SystemExit("no C++ metrics parsed; check simple_bench log format")

rust_data = json.load(open(rust_history_path, encoding="utf-8"))
rust = {}
for run in rust_data.get("runs", []):
    if run.get("impl") == "rust":
        rust[run["metric"]] = float(run["value"])
if not rust:
    raise SystemExit("no rust metrics in history")

workload_keys = [
    "workload", "rows", "max_key", "batch_size", "read_per_part",
    "partitions", "kv_size", "key_interval", "del_ratio", "test_secs",
    "write_batchs", "scan_bytes",
]
workload = {key: rust_data.get(key) for key in workload_keys if key in rust_data}

ratios = {}
for metric, value in rust.items():
    if (
        metric in cpp
        and cpp[metric] > 0
        and not metric.endswith("_us")
        and metric != "rss_peak_kb"
    ):
        ratios[metric] = value / cpp[metric]

baseline = {
    "schema": 1,
    "source": "fkv perf-baseline",
    "runner": os.environ.get("PERF_RUNNER_ID", "unknown"),
    "page_size": 4096,
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "workload": workload,
    "cpp": {"metrics": cpp},
    "rust": {"metrics": rust},
    "ratios": ratios,
}
with open(out_path, "w", encoding="utf-8") as handle:
    json.dump(baseline, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
print(f"wrote {out_path}: cpp={len(cpp)} rust={len(rust)} ratios={len(ratios)}")
PY
}

run_selftest() {
  SELFTEST_TMP=$(mktemp -d "${TMPDIR:-/tmp}/fkv-perf-selftest.XXXXXX")
  trap 'rm -rf "$SELFTEST_TMP"' EXIT
  python3 - "$SELFTEST_TMP" <<'PY'
import json
import sys

root = sys.argv[1]
workload = {
    "workload": "write-read", "rows": 1000000, "max_key": 1000000,
    "batch_size": 100, "read_per_part": 4, "partitions": 1,
    "kv_size": 128, "key_interval": 4, "del_ratio": 4,
    "test_secs": 30, "write_batchs": 10000, "scan_bytes": 0,
}
cpp = {
    "put_kv_s": 100000.0, "put_p99_us": 500.0,
    "get_kv_s": 200000.0, "get_p99_us": 400.0,
    "scan_kv_s": 300000.0, "scan_p99_us": 600.0,
}
rust = {
    "put_kv_s": 90000.0, "put_p99_us": 550.0,
    "get_kv_s": 190000.0, "get_p99_us": 440.0,
    "scan_kv_s": 280000.0, "scan_p99_us": 660.0,
    "rss_peak_kb": 100000.0,
}
ratios = {k: rust[k] / cpp[k] for k in ("put_kv_s", "get_kv_s", "scan_kv_s")}
base = {
    "schema": 1, "runner": "selftest", "page_size": 4096,
    "workload": workload, "cpp": {"metrics": cpp},
    "rust": {"metrics": rust}, "ratios": ratios,
}
with open(f"{root}/baseline.json", "w") as handle:
    json.dump(base, handle)

def history(metrics):
    runs = [{"impl": "rust", "metric": k, "value": v} for k, v in metrics.items()]
    data = dict(workload)
    data["runs"] = runs
    return data

with open(f"{root}/good.json", "w") as handle:
    json.dump(history(rust), handle)
slow = dict(rust)
slow["put_kv_s"] = rust["put_kv_s"] * 0.7
slow["get_kv_s"] = rust["get_kv_s"] * 0.7
slow["scan_kv_s"] = rust["scan_kv_s"] * 0.7
with open(f"{root}/slow.json", "w") as handle:
    json.dump(history(slow), handle)
PY
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/good.json" >/dev/null; then
    die "selftest: 未改动的 good case 被判红"
  fi
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/slow.json" >/dev/null 2>&1; then
    die "selftest: Rust 侧吞吐人为降 30% 仍然绿（门槛失效）"
  fi
  echo "selftest ok: 30% throughput regression is red; unchanged baseline is green"
}

case "$MODE" in
  gate) run_gate ;;
  compare) run_compare "$@" ;;
  produce) run_produce "$@" ;;
  selftest) run_selftest ;;
  -h|--help|help)
    sed -n '2,30p' "$0"
    ;;
  *)
    die "unknown mode $MODE (use gate|compare|produce|selftest)"
    ;;
esac
