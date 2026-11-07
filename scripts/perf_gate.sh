#!/usr/bin/env bash
# V6 性能门槛：同 runner 上跑 fkv_bench，与 perf-baseline 里的 Rust/C++ 基线比较。
#
# 用法：
#   scripts/perf_gate.sh gate            # CI 默认；用 PERF_BENCH_BIN 或 perf-latest，再比 perf-baseline
#   scripts/perf_gate.sh compare BASE CUR # 只比较两份 JSON（BASE=baseline，CUR=fkv history）
#     指标方向由 compare 内的 classify_metric 显式决定（吞吐 higher / 延迟与 RSS
#     lower / 资源、计数、窗口与配置回显 info 只审计 / 未登记 info+warning）；
#     不再使用「非延迟即吞吐」的默认。方向是 gate 侧策略，不写进 baseline schema。
#   scripts/perf_gate.sh produce --cpp-log LOG --rust-history JSON --out FILE
#     [--resources JSON] [--bench-commit SHA] [--data-append-mode BOOL] [--direct-io BOOL]
#     --resources 把侧车 JSON（scripts/perf_local.sh 产出的 store 体积 / 进程
#     RSS / CPU / 采样）原样并入 baseline JSON 的 "resources" 字段
#   scripts/perf_gate.sh selftest        # 不依赖硬件，反向验证「慢 30% 必须红」
#
# 门槛二进制必须与被审查的 fkv SHA 对齐：
#   1) 首选 PERF_BENCH_BIN（由 scripts/ci/build_rust_bench.sh 从当前 fkv 源码构建）；
#   2) 未提供时才下载 perf-latest，并读取同 release 的 commit.txt；
#   3) 设置 FKV_COMMIT 时断言 bench commit 与之一致，不一致直接红；未设置时
#      只告警——滚动二进制无法证明与当前源码一致。
# baseline schema 2 记录 bench_commit / data_append_mode / direct_io；schema 1
# 旧基线仍可读（跳过模式校验并打印 warning），但不再由 produce 生成。
#
# 缺 perf-baseline 时默认 **失败**（exit 78）。只有显式设置
# `PERF_GATE_REQUIRE_BASELINE=0` 才降级为告警并绿；该开关仅用于临时排障。
# 硬件只要求 Linux + 4 KiB 页 + PERF_NVME_ROOT 存在；普通 SSD、网络盘、
# 虚拟盘、容器 overlay 都可以跑。数字按同 runner 趋势读，不在脚本层拒绝。
#
# 固定 workload（两侧必须一致，写进 perf-baseline.json 的 workload 字段）：
#   workload=write-read rows=max_key=1000000 batch_size=100 partitions=1
#   read_per_part=4 kv_size=128 test_secs=30
#   write_batchs=rows/batch_size=10000 scan_bytes=0
# fkv_bench 的 --rng 缺省是 cpp（逐位复刻 C++ simple_bench 的 mt19937 随机
# 游走 + 每 4 条一次 delete）。此时实际生效的 key_interval / del_ratio 来自
# C++ 兼容生成器的 CPP_KEY_INTERVAL / CPP_DEL_RATIO（当前 4 / 4），不是
# PERF_KEY_INTERVAL / PERF_DEL_RATIO；后两者只在 fkv_bench 显式使用
# --rng grid 时生效（gate 固定走缺省 cpp，因此这两个环境变量不改变写负载）。
# 以 fkv_bench 写出的 history / baseline 的 workload 字段为准；gate 运行后
# 会回显实际值（rng=cpp, key_interval=4, del_ratio=4）。
# C++ 侧对应：
#   simple_bench --workload write-read --kv_size 128 --batch_size 100
#     --write_batchs 10000 --partitions 1 --max_key 1000000 --read_per_part 4
#     --read_thds 1 --test_secs 30 --scan_bytes 0 --show_write_perf=true
#     --kvoptions <default-options.ini>
# ini 只写 store_path，其余字段取 C++ KvOptions 默认值（num_threads=1、
# buffer_pool_size=128MB、fd_limit=10000、skip_verify_checksum=false、
# data_append_mode=false = PooledFilePages）。Rust fkv_bench 同样从
# KvOption::default() 取这些参数。
#
# 已知口径差异（不隐藏）：C++ `simple_bench` 的 write-read 是写/读线程并发；
# fkv_bench 的 write-read 是「先写阶段、再读阶段」的顺序执行。因此
# 「current rust / baseline rust」是同形状回归门槛（主判据），
# 「current rust / baseline cpp」只作同 runner 趋势比值，不承诺绝对值等价。
#
# 诊断 lane（Rust-only，**不进入**上面的固定门槛与 cpp 比值；默认不启用）：
#   PERF_WORKLOAD=ttl-write PERF_TTL_RATIO=4   输出 put_ttl_* / ttl_clean_*，
#     测带 expire_ts 的双树写与到期回收；PERF_TTL_MS 默认 3600000。
#   PERF_WORKLOAD=compact                     输出 compact_* 与并发 get_*，
#     测碎片 store 的搬运吞吐与 compact 期间前台读分位。
# 这两条 lane 需要与新 workload 字段匹配的 perf-baseline；默认
# PERF_TTL_RATIO=0 时命令行与历史逐字节一致。compact lane 在 fkv_bench 内
# 强制 Append，跑它的 baseline 时须显式设 PERF_DATA_APPEND_MODE=1，否则
# baseline / current 会把口径错记成 Pooled。

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MODE=${1:-gate}
if [ "$#" -gt 0 ]; then shift; fi

FKV_REPO=${FKV_REPO:-webc-site/fkv}
PERF_ARCH=${PERF_ARCH:-$(uname -m)}
PERF_LATEST_TAG=${PERF_LATEST_TAG:-perf-latest}
PERF_BASELINE_TAG=${PERF_BASELINE_TAG:-perf-baseline}
PERF_BASELINE_REPO=${PERF_BASELINE_REPO:-${GITHUB_REPOSITORY:-$FKV_REPO}}
PERF_LATEST_ASSET=${PERF_LATEST_ASSET:-fkv_bench-linux-$PERF_ARCH}
PERF_BASELINE_ASSET=${PERF_BASELINE_ASSET:-perf-baseline.json}
PERF_RUNNER_ID=${PERF_RUNNER_ID:-linux-$PERF_ARCH}
PERF_NVME_ROOT=${PERF_NVME_ROOT:-}
PERF_GATE_REQUIRE_BASELINE=${PERF_GATE_REQUIRE_BASELINE:-1}

# 门槛二进制来源与 commit pin：
#   PERF_BENCH_BIN      已构建的 fkv_bench（优先；CI 从被审查的 fkv SHA 构建）
#   FKV_COMMIT          被审查的 fkv SHA；设置时断言 bench commit 与之一致
#   PERF_BENCH_COMMIT   源码构建二进制的实际 SHA（缺省回落到 FKV_COMMIT / git HEAD）
FKV_COMMIT=${FKV_COMMIT:-}
PERF_BENCH_BIN=${PERF_BENCH_BIN:-}
PERF_BENCH_COMMIT=${PERF_BENCH_COMMIT:-}
FKV_REPO_PATH=${FKV_REPO_PATH:-$ROOT/../fkv}

# baseline / current 记录的分配与 IO 口径（compare 会校验一致性）。
# 当前 fkv_bench：KvOption::default().data_append_mode=false（PooledFilePages）
# + CompioStore::with_direct_io(true)。fkv 默认变化时必须同步更新本值并重建
# perf-baseline，否则门槛会拿旧口径基线比新口径实现。
PERF_DATA_APPEND_MODE=${PERF_DATA_APPEND_MODE:-0}
PERF_DIRECT_IO=${PERF_DIRECT_IO:-1}

PERF_WORKLOAD=${PERF_WORKLOAD:-write-read}
# 诊断 lane 专用（默认关闭）：ttl-write 需要 ratio > 0 才会带 expire_ts。
PERF_TTL_RATIO=${PERF_TTL_RATIO:-0}
PERF_TTL_MS=${PERF_TTL_MS:-3600000}
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

# 托管 NVMe runner 的 run-to-run 噪声明显高于裸金属：吞吐 20%、延迟 100%
# 的回归窗口仍能拦住 30% 级别的真实退化（selftest 覆盖），又不会因一次
# 调度抖动把 p99 判红。单次 max / p999 / p9999 是离群点，不参与门槛。
PERF_MAX_REGRESSION_PCT=${PERF_MAX_REGRESSION_PCT:-20}
PERF_MAX_LATENCY_REGRESSION_PCT=${PERF_MAX_LATENCY_REGRESSION_PCT:-100}
PERF_MAX_RSS_REGRESSION_PCT=${PERF_MAX_RSS_REGRESSION_PCT:-20}

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

# 统一布尔词表，写进 JSON 前转成 true / false。
bool_value() {
  case "${1:-}" in
    1|true|True|TRUE|yes|Yes|YES|on|On|ON) printf 'true' ;;
    0|false|False|FALSE|no|No|NO|off|Off|OFF) printf 'false' ;;
    *) die "非法布尔值 '$1'（用 1/0/true/false/yes/no/on/off）" ;;
  esac
}

# 源码构建二进制优先；未给则回落到 FKV_COMMIT；仍为空时尝试兄弟仓 git HEAD。
# PERF_BENCH_COMMIT 与 FKV_COMMIT 同时存在时必须一致，否则拒绝给结论。
resolve_bench_commit() {
  local commit=""
  if [ -n "$PERF_BENCH_COMMIT" ]; then
    commit=$PERF_BENCH_COMMIT
  fi
  if [ -n "$FKV_COMMIT" ]; then
    if [ -n "$commit" ] && [ "$commit" != "$FKV_COMMIT" ]; then
      die "PERF_BENCH_COMMIT=$commit 与 FKV_COMMIT=$FKV_COMMIT 不一致"
    fi
    commit=$FKV_COMMIT
  fi
  if [ -z "$commit" ] && [ -d "$FKV_REPO_PATH" ]; then
    commit=$(git -C "$FKV_REPO_PATH" rev-parse HEAD 2>/dev/null || true)
  fi
  printf '%s' "$commit"
}

require_linux() {
  if [ "$(uname -s)" != "Linux" ]; then
    skip_with_error "perf gate 只能在 Linux 上跑（当前 $(uname -s)）"
  fi
}

require_4k_nvme() {
  local page
  page=$(getconf PAGESIZE 2>/dev/null || echo 0)
  if [ "$page" != "4096" ]; then
    skip_with_error "需要 4 KiB OS 页，当前 PAGESIZE=$page；跨页大小的 manifest framing 不可互读"
  fi
  if [ -z "$PERF_NVME_ROOT" ]; then
    skip_with_error "PERF_NVME_ROOT 未设置"
  fi
  if [ ! -d "$PERF_NVME_ROOT" ]; then
    skip_with_error "PERF_NVME_ROOT 不存在: $PERF_NVME_ROOT"
  fi
  # 不再限制 NVMe / 本地盘 / rotational / 传输层。普通 SSD、网络盘、容器
  # overlay 都可以跑性能对比；硬件信息只打印，不再阻断。
  local fstype source tran
  source=$(findmnt -T "$PERF_NVME_ROOT" -no SOURCE 2>/dev/null || true)
  tran=$(findmnt -T "$PERF_NVME_ROOT" -no TRAN 2>/dev/null || true)
  fstype=$(findmnt -T "$PERF_NVME_ROOT" -no FSTYPE 2>/dev/null || true)
  echo "hardware: pagesize=$page fstype=${fstype:-unknown} source=${source:-unknown} tran=${tran:-unknown}"
}

gh_download() {
  local tag=$1 asset=$2 dest=$3 repo=${4:-$FKV_REPO}
  if command -v gh >/dev/null 2>&1; then
    if gh release download "$tag" -R "$repo" -p "$asset" -D "$dest" --clobber 2>/dev/null; then
      return 0
    fi
  fi
  local url="https://github.com/${repo}/releases/download/${tag}/${asset}"
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
  if ! gh_download "$PERF_BASELINE_TAG" "$PERF_BASELINE_ASSET" "$GATE_TMP" "$PERF_BASELINE_REPO"; then
    if [ "$optional" = "1" ]; then
      echo "::warning title=perf-gate:: perf-baseline 缺失；PERF_GATE_REQUIRE_BASELINE=0 显式降级，跳过比较" >&2
      echo "PERF GATE DEGRADED: baseline missing, explicitly allowed by PERF_GATE_REQUIRE_BASELINE=0"
      return 0
    fi
    skip_with_error "缺少 ${PERF_BASELINE_TAG}/${PERF_BASELINE_ASSET}（${PERF_BASELINE_REPO}）。默认必须阻断；仅排障时可显式设 PERF_GATE_REQUIRE_BASELINE=0 降级"
  fi

  local bench=""
  local bench_commit=""
  local append_mode direct_io
  append_mode=$(bool_value "$PERF_DATA_APPEND_MODE")
  direct_io=$(bool_value "$PERF_DIRECT_IO")

  if [ -n "$PERF_BENCH_BIN" ]; then
    [ -x "$PERF_BENCH_BIN" ] || die "PERF_BENCH_BIN 不可执行: $PERF_BENCH_BIN"
    bench=$PERF_BENCH_BIN
    bench_commit=$(resolve_bench_commit)
    echo "== bench: $bench（源码构建，commit=${bench_commit:-unknown}） =="
  else
    if ! gh_download "$PERF_LATEST_TAG" "$PERF_LATEST_ASSET" "$GATE_TMP"; then
      die "缺少 ${PERF_LATEST_TAG}/${PERF_LATEST_ASSET}（${FKV_REPO}）；改用 PERF_BENCH_BIN 指定从当前源码构建的二进制"
    fi
    bench="${GATE_TMP}/${PERF_LATEST_ASSET}"
    chmod +x "$bench"
    if gh_download "$PERF_LATEST_TAG" "commit.txt" "$GATE_TMP"; then
      bench_commit=$(tr -d '[:space:]' < "${GATE_TMP}/commit.txt")
    fi
    echo "== bench: ${PERF_LATEST_ASSET}（perf-latest，commit=${bench_commit:-unknown}） =="
  fi
  if [ -z "$bench_commit" ]; then
    bench_commit=unknown
  fi

  if [ -n "$FKV_COMMIT" ]; then
    [ "$bench_commit" != "unknown" ] ||
      die "无法确定 bench commit（缺少 commit.txt）；设置 FKV_COMMIT 时必须能断言，拒绝给结论"
    [ "$bench_commit" = "$FKV_COMMIT" ] ||
      die "bench commit=$bench_commit != FKV_COMMIT=$FKV_COMMIT；拒绝用旧二进制测当前源码"
    echo "bench commit asserted: $bench_commit == FKV_COMMIT"
  else
    echo "::warning title=perf-gate:: 未设置 FKV_COMMIT；bench commit=${bench_commit}，无法断言与当前源码一致" >&2
  fi
  echo "bench mode: data_append_mode=$append_mode direct_io=$direct_io"

  local current="${GATE_TMP}/current.json"
  echo "== run fkv_bench =="
  # TTL 旋钮只在显式开启时追加：默认命令行与历史版本 / 旧二进制逐字节一致。
  local bench_arg_li=(
    --impl rust
    --root "$GATE_BENCH_DIR/run"
    --workload "$PERF_WORKLOAD"
    --rows "$PERF_ROWS"
    --max-key "$PERF_MAX_KEY"
    --batch-size "$PERF_BATCH_SIZE"
    --read-per-part "$PERF_READ_PER_PART"
    --partitions "$PERF_PARTITIONS"
    --kv-size "$PERF_KV_SIZE"
    --key-interval "$PERF_KEY_INTERVAL"
    --del-ratio "$PERF_DEL_RATIO"
    --write-batchs "$PERF_WRITE_BATCHS"
    --scan-bytes "$PERF_SCAN_BYTES"
    --test-secs "$PERF_TEST_SECS"
    --history "$current"
  )
  if [ "$bench_commit" != "unknown" ]; then
    # 显式写 commit：CI 的 cwd 是本仓，bench 自己 `git rev-parse` 会得到
    # eloqstore2fkv 的 SHA，而不是被审查的 fkv SHA。
    bench_arg_li+=(--commit "$bench_commit")
  fi
  if [ "$PERF_TTL_RATIO" -gt 0 ]; then
    bench_arg_li+=(--ttl-ratio "$PERF_TTL_RATIO" --ttl-ms "$PERF_TTL_MS")
  fi
  "$bench" "${bench_arg_li[@]}"

  # current.json 由 fkv_bench 写出，本身不带口径字段（不能改 fkv 源码）；
  # 在 gate 侧补齐，compare 才能拒绝跨模式比较。
  python3 - "$current" "$bench_commit" "$append_mode" "$direct_io" <<'PY'
import json
import sys

path, commit, append_mode, direct_io = sys.argv[1:5]
with open(path, encoding="utf-8") as handle:
    data = json.load(handle)
data["bench_commit"] = commit
data["data_append_mode"] = append_mode == "true"
data["direct_io"] = direct_io == "true"
with open(path, "w", encoding="utf-8") as handle:
    json.dump(data, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
PY

  # 回显实际 workload 口径：--rng cpp 下 key_interval / del_ratio 由 C++ 兼容
  # 生成器决定，不能按 PERF_KEY_INTERVAL / PERF_DEL_RATIO 的入参假定。以
  # fkv_bench 写出的 history 为准，排障时与 baseline.workload 对照。
  python3 - "$current" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)
keys = (
    "workload", "rng", "key_interval", "del_ratio", "rows", "max_key",
    "batch_size", "read_per_part", "partitions", "kv_size", "test_secs",
    "write_batchs", "scan_bytes",
)
print(
    "workload (effective, from fkv_bench history): "
    + " ".join(f"{key}={data[key]}" for key in keys if key in data)
)
PY

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

run_hardware() {
  require_linux
  require_4k_nvme
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

schema = base.get("schema")
if schema not in (1, 2):
    raise SystemExit(f"baseline schema 不支持: {schema!r}（支持 1 / 2）")

def fail(msg):
    print(f"PERF GATE FAILED: {msg}", file=sys.stderr)
    raise SystemExit(1)

base_commit = base.get("bench_commit") or "<unset>"
cur_commit = cur_data.get("bench_commit") or "<unset>"
print(f"bench_commit: baseline={base_commit} current={cur_commit}")
print(
    "bench mode: baseline="
    f"data_append_mode={base.get('data_append_mode', '<unset>')} "
    f"direct_io={base.get('direct_io', '<unset>')}; current="
    f"data_append_mode={cur_data.get('data_append_mode', '<unset>')} "
    f"direct_io={cur_data.get('direct_io', '<unset>')}"
)
if schema >= 2:
    if not base.get("bench_commit") or base.get("bench_commit") == "unknown":
        fail("baseline 缺少 bench_commit，无法审计基线来源；用 perf_gate.sh produce 重新生成")
    mode_mismatch = []
    for key in ("data_append_mode", "direct_io"):
        if key not in base:
            fail(f"baseline schema=2 缺少 {key}")
        if key not in cur_data:
            fail(
                f"current 缺少 {key}，无法确认与 baseline 同口径"
                "（gate 会自动注入；裸 compare 请提供该字段）"
            )
        if cur_data[key] != base[key]:
            mode_mismatch.append(
                f"{key}: baseline={base[key]!r} current={cur_data[key]!r}"
            )
    if mode_mismatch:
        fail("分配 / IO 模式不一致（跨模式比值不可比）：" + "; ".join(mode_mismatch))
else:
    print(
        "legacy baseline schema=1：未记录 data_append_mode / direct_io / bench_commit，"
        "跳过模式一致性检查",
        file=sys.stderr,
    )

expected_workload = base.get("workload") or {}
workload_keys = [
    "workload", "rows", "max_key", "batch_size", "read_per_part",
    "partitions", "kv_size", "key_interval", "del_ratio", "rng", "test_secs",
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

# 指标方向表（gate 侧策略，不写进 baseline schema；旧基线无需重发即可生效）。
#
#   higher  ：吞吐 / 速率，低于基线超过 tput_pct 判红；
#   lower   ：延迟 / RSS，高于基线超过 lat_pct / rss_pct 判红；
#   info    ：资源 / 计数 / 窗口 / 配置回显，只打印审计，不参与 pass/fail；
#   None    ：已知噪声指标，不打印（沿用原有跳过语义）；
#   unknown ：方向表里没有的名字，只打印 + warning，绝不默认按吞吐判。
#
# 资源 / 计数指标为什么是 info（而不是简单换成「越低越好」）：
#   - store_bytes 取决于数据集合、文件布局与后台 GC 时机，单次点值方差大；
#     反过来设「不得变大」阈值，会把真实的回收 / 压缩改善判红，重演本 issue
#     的假红。它只适合看趋势，不适合做单次硬门槛。
#   - io_*_num / proc_io_* 是路径相关计数（fd 复用、sync 合并、进程累计）：
#     变少不一定更好（例如 data_sync 变少可能意味着持久化语义变化），
#     且 --repeat>1 时 proc_io_* 是累计值，不是单次运行口径。
#   - *_window_num 是 1 秒计量窗口数，是写 / 读阶段时长的代理，吞吐已由
#     *_kv_s / *_qps 覆盖，单独设门槛只会重复判红。
# 这些指标只进输出表，由人工 / 看板看趋势；要升级成硬门槛必须先定义明确的
# 上/下界与采样口径，不能直接套用吞吐的「允许下降 X%」。
HIGHER_SUFFIXES = ("_kv_s", "_mib_s", "_batch_s", "_gib_s", "_qps", "_page_s", "_file_s")
# 已退役指标：旧基线里可能还有，当前 history 不再输出。遇到「基线有、当前
# 没有」时跳过，避免为删除一个非引擎微基准而重发基线。
RETIRED_METRICS = frozenset({"gxhash_4096B_gib_s"})
INFO_EXACT = frozenset({
    "store_bytes", "file_num",
    "write_amp", "space_amp_nominal", "write_at_per_batch", "sync_per_batch",
    "cache_len", "cache_capacity",
    "compact_store_bytes_before", "compact_store_bytes_after",
    "store_bytes_before_compact", "store_bytes_after_compact",
    # 配置回显，不是性能结果
    "ttl_ratio", "ttl_ms", "compact_pages_per_file",
})
INFO_PREFIXES = ("proc_io_", "store_bytes_", "store_file_num_")
INFO_SUFFIXES = ("_num", "_cnt", "_count", "_bytes", "_deleted", "_ratio")
LATENCY_STAT_TOKENS = ("avg", "p50", "p90", "p95", "p99", "max")
SKIP_SUFFIXES = ("_max_us", "_p999_us", "_p9999_us")
REPEAT_VARIANT_SUFFIXES = ("_min", "_max")

def classify_metric(name):
    if name.endswith(SKIP_SUFFIXES):
        return None
    if name in RETIRED_METRICS:
        return "info"
    if name == "reopen_us":
        return "lower"
    if name == "compact_total_us":
        return "lower"
    if name == "fullscan_total_us":
        return "lower"
    if name == "rss_peak_kb":
        return "lower"
    if name.endswith("_us") and any(tok in name for tok in LATENCY_STAT_TOKENS):
        return "lower"
    if name.endswith(REPEAT_VARIANT_SUFFIXES):
        # repeat>1 的 <metric>_min/_max 聚合变体：中位数才是门槛值，变体只审计。
        return "info"
    if "_window_" in name:
        # 秒级窗口是阶段时长 / 抖动代理：单窗口可能赶上一次 fsync 长尾，
        # 即使总吞吐不变也会大幅波动。整段 `*_kv_s` / `*_qps` 已是门槛值，
        # 这里只审计，避免把窗口噪声判成回归。
        return "info"
    if name.endswith(HIGHER_SUFFIXES):
        return "higher"
    if name in INFO_EXACT or name.startswith(INFO_PREFIXES) or name.endswith(INFO_SUFFIXES):
        return "info"
    return "unknown"

errors = []
rows = []
unknown_metrics = []
info_count = 0
for metric in sorted(base_rust):
    if metric in RETIRED_METRICS and metric not in cur:
        print(f"retired metric skipped: {metric}", file=sys.stderr)
        continue
    direction = classify_metric(metric)
    if direction is None:
        continue
    if metric not in cur:
        errors.append(f"current 缺少指标 {metric}")
        continue
    base_value = base_rust[metric]
    value = cur[metric]
    if base_value <= 0:
        continue
    delta = (value - base_value) / base_value * 100.0
    if direction == "higher":
        limit = base_value * (1 - tput_pct / 100.0)
        ok = value >= limit
        bound = f">= {limit:.1f}"
    elif direction == "lower":
        pct = rss_pct if metric == "rss_peak_kb" else lat_pct
        limit = base_value * (1 + pct / 100.0)
        ok = value <= limit
        bound = f"<= {limit:.1f}"
    else:
        # info / unknown 只审计：永远 ok，且不进入 regressions。
        ok = True
        bound = "-"
        info_count += 1
        if direction == "unknown":
            unknown_metrics.append(metric)
    rows.append((metric, base_value, value, delta, bound, ok, direction))
    if not ok:
        errors.append(
            f"{metric}: baseline={base_value:.2f} current={value:.2f} "
            f"({delta:+.1f}%) 超过阈值 {bound}"
        )

# 同 runner 相对比值：current rust / baseline cpp 不得低于 baseline ratio 的容差。
# 只有 higher-is-better 的吞吐指标有「比值不得下降」语义；资源 / 延迟 / 未知指标
# 一律不放进比值门槛（否则会重演「方向反了」的假红）。
for metric, base_r in sorted(base_ratio.items()):
    if classify_metric(metric) != "higher":
        continue
    cpp_value = base_cpp.get(metric)
    if not cpp_value or cpp_value <= 0 or metric not in cur:
        continue
    cur_r = cur[metric] / cpp_value
    limit = base_r * (1 - tput_pct / 100.0)
    ok = cur_r >= limit
    delta = (cur_r - base_r) / base_r * 100.0 if base_r else 0.0
    rows.append((f"{metric}/cpp", base_r, cur_r, delta, f">= {limit:.4f}", ok, "higher"))
    if not ok:
        errors.append(
            f"{metric}/cpp: baseline={base_r:.4f} current={cur_r:.4f} "
            f"({delta:+.1f}%) 超过阈值 >= {limit:.4f}"
        )

print(f"{'metric':<28} {'baseline':>12} {'current':>12} {'delta%':>8}  {'limit':<20} {'dir':<6} status")
for metric, base_value, value, delta, bound, ok, direction in rows:
    kind = {
        "higher": "tput",
        "lower": "rss" if metric == "rss_peak_kb" else "lat",
        "info": "info",
        "unknown": "info",
    }[direction]
    status = "info" if direction in ("info", "unknown") else ("ok" if ok else "FAIL")
    print(
        f"{metric:<28} {base_value:>12.2f} {value:>12.2f} {delta:>+8.1f}  "
        f"{bound:<20} {kind:<6} {status}"
    )

if unknown_metrics:
    print(
        "\nWARNING: 方向表未登记的指标只记录、不参与 pass/fail: "
        + ", ".join(unknown_metrics)
        + "\n         请在 scripts/perf_gate.sh 的 classify_metric 中登记方向"
        "（吞吐 higher / 延迟或 RSS lower / 资源或计数 info）",
        file=sys.stderr,
    )

if errors:
    print("\nregressions:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    raise SystemExit(1)
print(
    f"\nperf gate ok: runner={base.get('runner')} "
    f"bench_commit={base_commit} current_commit={cur_commit} "
    f"tput_tol={tput_pct}% lat_tol={lat_pct}% metrics={len(rows)} "
    f"gated={len(rows) - info_count} informational={info_count - len(unknown_metrics)} "
    f"unknown={len(unknown_metrics)}"
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
  local cpp_log="" rust_history="" out="" resources=""
  local bench_commit="" data_append_mode="" direct_io=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --cpp-log) cpp_log=$2; shift 2 ;;
      --rust-history) rust_history=$2; shift 2 ;;
      --out) out=$2; shift 2 ;;
      --resources) resources=$2; shift 2 ;;
      --bench-commit) bench_commit=$2; shift 2 ;;
      --data-append-mode) data_append_mode=$2; shift 2 ;;
      --direct-io) direct_io=$2; shift 2 ;;
      *) die "produce: unknown argument $1" ;;
    esac
  done
  [ -n "$cpp_log" ] || die "produce: --cpp-log required"
  [ -n "$rust_history" ] || die "produce: --rust-history required"
  [ -n "$out" ] || die "produce: --out required"
  [ -f "$cpp_log" ] || die "produce: cpp log not found: $cpp_log"
  [ -f "$rust_history" ] || die "produce: rust history not found: $rust_history"
  if [ -n "$resources" ] && [ ! -f "$resources" ]; then
    die "produce: resources not found: $resources"
  fi

  # 基线必须能追溯到产出它的 fkv SHA；优先级：
  #   --bench-commit > PERF_BENCH_COMMIT > FKV_COMMIT > 兄弟仓 git HEAD。
  bench_commit=${bench_commit:-${PERF_BENCH_COMMIT:-${FKV_COMMIT:-}}}
  if [ -z "$bench_commit" ] && [ -d "$FKV_REPO_PATH" ]; then
    bench_commit=$(git -C "$FKV_REPO_PATH" rev-parse HEAD 2>/dev/null || true)
  fi
  if [ -z "$bench_commit" ] || [ "$bench_commit" = "unknown" ]; then
    if [ "${PERF_REQUIRE_BENCH_COMMIT:-0}" = "1" ]; then
      die "produce: 无法确定 bench_commit（设 FKV_COMMIT / PERF_BENCH_COMMIT，或提供 FKV_REPO_PATH 源码树）"
    fi
    echo "::warning title=perf-gate:: produce 未确定 bench_commit；基线将不可审计" >&2
    bench_commit=unknown
  fi
  data_append_mode=$(bool_value "${data_append_mode:-${PERF_DATA_APPEND_MODE:-0}}")
  direct_io=$(bool_value "${direct_io:-${PERF_DIRECT_IO:-1}}")

  PERF_RUNNER_ID="$PERF_RUNNER_ID" \
  PERF_BENCH_COMMIT="$bench_commit" \
  PERF_DATA_APPEND_MODE="$data_append_mode" \
  PERF_DIRECT_IO="$direct_io" \
  python3 - "$cpp_log" "$rust_history" "$out" "$resources" <<'PY'
import json
import os
import re
import sys
from datetime import datetime, timezone

cpp_log_path, rust_history_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
resources_path = sys.argv[4] if len(sys.argv) > 4 else ""
text = open(cpp_log_path, encoding="utf-8").read()

bench_commit = os.environ.get("PERF_BENCH_COMMIT", "unknown")
append_mode = os.environ.get("PERF_DATA_APPEND_MODE") == "true"
direct_io = os.environ.get("PERF_DIRECT_IO") == "true"

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

# history 若已带口径字段，必须与本次 produce 的声明一致，防止错标。
for key, expected in (("data_append_mode", append_mode), ("direct_io", direct_io)):
    if key in rust_data and bool(rust_data[key]) != expected:
        raise SystemExit(
            f"rust history {key}={rust_data[key]!r} 与 produce 记录 {expected!r} 不一致"
        )
history_commit = rust_data.get("bench_commit")
if (
    history_commit
    and history_commit != "unknown"
    and bench_commit != "unknown"
    and history_commit != bench_commit
):
    raise SystemExit(
        f"rust history bench_commit={history_commit!r} 与 produce 记录 {bench_commit!r} 不一致"
    )

workload_keys = [
    "workload", "rows", "max_key", "batch_size", "read_per_part",
    "partitions", "kv_size", "key_interval", "del_ratio", "rng", "test_secs",
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
    "schema": 2,
    "source": "fkv perf-baseline",
    "runner": os.environ.get("PERF_RUNNER_ID", "unknown"),
    "page_size": 4096,
    "bench_commit": bench_commit,
    "data_append_mode": append_mode,
    "direct_io": direct_io,
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "workload": workload,
    "cpp": {"metrics": cpp},
    "rust": {"metrics": rust},
    "ratios": ratios,
}
# 可选资源侧车（store 体积 / 进程 RSS / CPU / 采样）。只在提供时写入，
# 不改变 schema 语义：compare 只读已知字段，旧调用方不受影响。
if resources_path:
    with open(resources_path, encoding="utf-8") as handle:
        baseline["resources"] = json.load(handle)

with open(out_path, "w", encoding="utf-8") as handle:
    json.dump(baseline, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
print(
    f"wrote {out_path}: bench_commit={bench_commit} "
    f"data_append_mode={append_mode} direct_io={direct_io} "
    f"cpp={len(cpp)} rust={len(rust)} ratios={len(ratios)} "
    f"resources={'yes' if resources_path else 'no'}"
)
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
    # 方向表覆盖的其他吞吐 / 速率类：必须按 higher-is-better 判。
    "put_batch_s": 213.0, "gxhash_4096B_gib_s": 44.0, "scan_page_s": 550.0,
    # fc57926 新增的派生指标：逻辑带宽必须是 higher、reopen 延迟必须是 lower，
    # 四个放大 / 每批指标只审计。值不依赖平台（macOS 只省略 write_amp 的
    # 真实输出，合成 fixture 仍覆盖 INFO_EXACT 分支）。
    "put_mib_s": 45.57, "get_mib_s": 25.50,
    "scan_mib_s": 484.69, "fullscan_mib_s": 297.85,
    "reopen_us": 696.0,
    "put_p95_us": 450.0, "get_p95_us": 350.0, "scan_p95_us": 500.0,
    "write_amp": 1.27, "space_amp_nominal": 2.07,
    "write_at_per_batch": 4.77, "sync_per_batch": 1.0,
    # 资源 / 计数 / 窗口类：只审计，不参与 pass/fail。
    "store_bytes": 106352640.0,
    "store_bytes_data": 90000000.0, "store_bytes_segment": 16000000.0,
    "store_bytes_manifest": 352640.0, "store_file_num": 8.0,
    "cache_hit_num": 1000.0, "cache_miss_num": 100.0,
    "cache_evict_num": 10.0, "cache_bytes": 4096.0,
    "cache_len": 1.0, "cache_capacity": 65536.0, "cache_hit_ratio": 0.909,
    "compact_store_bytes_before": 106352640.0,
    "compact_store_bytes_after": 80000000.0,
    "compact_reclaim_ratio": 0.247,
    "delete_kv_s": 120000.0, "delete_batch_s": 1200.0,
    "delete_mib_s": 14.65, "delete_p99_us": 700.0,
    "compact_total_us": 5000.0,
    "store_bytes_before_compact": 106352640.0,
    "store_bytes_after_compact": 80000000.0,
    "io_fd_open_num": 8.0, "io_read_at_num": 833236.0,
    "io_data_sync_num": 10000.0,
    "proc_io_read_bytes": 3412946944.0, "proc_io_write_bytes": 329027584.0,
    "put_window_num": 47.0, "get_window_num": 31.0,
    "put_window_kv_s": 20400.0, "get_window_qps": 7260.0,
    "file_num": 8.0, "compact_run_num": 12.0,
}
# 当前 fkv_bench 不再输出 gxhash；基线里保留它，验证 retired 指标在
# 「基线有、当前没有」时被跳过，而不是报 current missing。
current = {k: v for k, v in rust.items() if k != "gxhash_4096B_gib_s"}
ratios = {k: rust[k] / cpp[k] for k in ("put_kv_s", "get_kv_s", "scan_kv_s")}
base = {
    "schema": 2, "runner": "selftest", "page_size": 4096,
    "bench_commit": "b" * 40,
    "data_append_mode": False, "direct_io": True,
    "workload": workload, "cpp": {"metrics": cpp},
    "rust": {"metrics": rust}, "ratios": ratios,
}
with open(f"{root}/baseline.json", "w") as handle:
    json.dump(base, handle)

# 旧 schema=1 基线没有模式 / commit 字段，读取时必须继续兼容。
legacy = dict(base)
legacy["schema"] = 1
for key in ("bench_commit", "data_append_mode", "direct_io"):
    legacy.pop(key)
with open(f"{root}/legacy-baseline.json", "w") as handle:
    json.dump(legacy, handle)

def history(metrics, *, append_mode=False, direct_io=True, commit="c" * 40):
    runs = [{"impl": "rust", "metric": k, "value": v} for k, v in metrics.items()]
    data = dict(workload)
    data["bench_commit"] = commit
    data["data_append_mode"] = append_mode
    data["direct_io"] = direct_io
    data["runs"] = runs
    return data

with open(f"{root}/good.json", "w") as handle:
    # current 与 baseline 的 commit 不同是正常的（基线可以来自更早的 SHA）。
    json.dump(history(current), handle)
slow = dict(current)
slow["put_kv_s"] = current["put_kv_s"] * 0.7
slow["get_kv_s"] = current["get_kv_s"] * 0.7
slow["scan_kv_s"] = current["scan_kv_s"] * 0.7
with open(f"{root}/slow.json", "w") as handle:
    json.dump(history(slow), handle)

# 其余吞吐 / 速率后缀（_batch_s / _gib_s / _page_s）也必须按 higher 判红。
rate_slow = dict(current)
rate_slow["put_batch_s"] = current["put_batch_s"] * 0.7
rate_slow["scan_page_s"] = current["scan_page_s"] * 0.7
with open(f"{root}/rate-slow.json", "w") as handle:
    json.dump(history(rate_slow), handle)

# fc57926 的逻辑带宽（_mib_s）降 25%：默认 20% 吞吐容差下必须判红。
mib_slow = dict(current)
mib_slow["put_mib_s"] = current["put_mib_s"] * 0.75
with open(f"{root}/mib-slow.json", "w") as handle:
    json.dump(history(mib_slow), handle)

# reopen_us 是显式 lower，不是靠 *_us 延迟后缀推断；+150% 超过默认 100%
# 延迟容差，必须判红。
reopen_slow = dict(current)
reopen_slow["reopen_us"] = current["reopen_us"] * 2.5
with open(f"{root}/reopen-slow.json", "w") as handle:
    json.dump(history(reopen_slow), handle)

# 四个放大 / 每批派生指标（write_amp / space_amp_nominal / write_at_per_batch
# / sync_per_batch，fc57926）：变差和改善都只审计。±30% 两个方向一起覆盖，
# 无论它们被误分类成 higher 还是 lower，都会有一侧被判红。
derived_worse = dict(current)
for key in ("write_amp", "space_amp_nominal", "write_at_per_batch", "sync_per_batch"):
    derived_worse[key] = current[key] * 1.3
with open(f"{root}/derived-worse.json", "w") as handle:
    json.dump(history(derived_worse), handle)

derived_better = dict(current)
for key in ("write_amp", "space_amp_nominal", "write_at_per_batch", "sync_per_batch"):
    derived_better[key] = current[key] * 0.7
with open(f"{root}/derived-better.json", "w") as handle:
    json.dump(history(derived_better), handle)

# 延迟 +30%：lower-is-better，必须判红（配合 20% 容差，证明方向而不是默认容差）。
latency_slow = dict(current)
latency_slow["put_p99_us"] = current["put_p99_us"] * 1.3
latency_slow["get_p99_us"] = current["get_p99_us"] * 1.3
latency_slow["scan_p99_us"] = current["scan_p99_us"] * 1.3
with open(f"{root}/latency-slow.json", "w") as handle:
    json.dump(history(latency_slow), handle)

# 资源 / 计数 / 窗口指标变差（更大 / 更多）：只审计，不得判红。
resource_worse = dict(current)
for key, factor in {
    "store_bytes": 1.3,
    "io_fd_open_num": 2.0,
    "io_read_at_num": 1.3,
    "io_data_sync_num": 1.3,
    "proc_io_read_bytes": 1.3,
    "proc_io_write_bytes": 1.3,
    "put_window_num": 1.25,
    "get_window_num": 1.25,
    # 窗口吞吐下降也只是 info：整段吞吐已由 put_kv_s 覆盖。
    "put_window_kv_s": 0.3,
    "get_window_qps": 0.3,
    "file_num": 2.0,
    "compact_run_num": 2.0,
}.items():
    resource_worse[key] = current[key] * factor
with open(f"{root}/resource-worse.json", "w") as handle:
    json.dump(history(resource_worse), handle)

# 同一批资源指标「改善」（更小 / 更少）：同样不得判红，避免方向反转后的假红。
resource_better = dict(current)
for key, factor in {
    "store_bytes": 0.7,
    "io_fd_open_num": 0.5,
    "io_read_at_num": 0.7,
    "io_data_sync_num": 0.7,
    "proc_io_read_bytes": 0.7,
    "proc_io_write_bytes": 0.7,
    "put_window_num": 0.75,
    "get_window_num": 0.75,
    "put_window_kv_s": 2.0,
    "get_window_qps": 2.0,
    "file_num": 0.5,
    "compact_run_num": 0.5,
}.items():
    resource_better[key] = current[key] * factor
with open(f"{root}/resource-better.json", "w") as handle:
    json.dump(history(resource_better), handle)

# 未知指标：foo_num 命中计数类（info），mystery_metric 完全未登记（info + warning）；
# 两者大幅下降都不得按吞吐判红。
rust_unknown = dict(current)
rust_unknown["foo_num"] = 100.0
rust_unknown["mystery_metric"] = 50.0
unknown_base = dict(base)
unknown_base["rust"] = {"metrics": rust_unknown}
with open(f"{root}/unknown-baseline.json", "w") as handle:
    json.dump(unknown_base, handle)
unknown_current = dict(rust_unknown)
unknown_current["foo_num"] = rust_unknown["foo_num"] * 0.2
unknown_current["mystery_metric"] = rust_unknown["mystery_metric"] * 0.2
with open(f"{root}/unknown.json", "w") as handle:
    json.dump(history(unknown_current), handle)

with open(f"{root}/mode-mismatch.json", "w") as handle:
    json.dump(history(rust, append_mode=True), handle)
missing_mode = history(rust)
missing_mode.pop("data_append_mode")
with open(f"{root}/missing-mode.json", "w") as handle:
    json.dump(missing_mode, handle)
legacy_history = history(rust)
for key in ("bench_commit", "data_append_mode", "direct_io"):
    legacy_history.pop(key)
with open(f"{root}/legacy-history.json", "w") as handle:
    json.dump(legacy_history, handle)

# produce 的 C++ 日志夹具：只包含脚本解析的三种 summary 行。
with open(f"{root}/cpp-bench.log", "w") as handle:
    handle.write(
        "write summary 100000 kvs/s average latency 500 microseconds "
        "p99 600 microseconds p99.9 700 microseconds\n"
        "read average 200000 QPS\n"
        "read speed 1 GB/s p99 400 microseconds\n"
        "scan throughput 100 MB/s 300000 kvs/s\n"
        "scan speed 1 GB/s p99 600 microseconds\n"
    )
PY
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/good.json" \
    >"$SELFTEST_TMP/good.log" 2>"$SELFTEST_TMP/good.err"; then
    die "selftest: 未改动的 good case 被判红"
  fi
  # 新指标必须真的被方向表认领：unknown=0 才能证明没有静默降级为 info；
  # 再断言 fc57926 的硬门槛各自落在 tput / lat 行，漏登记时直接红。
  if ! grep -q "unknown=0" "$SELFTEST_TMP/good.log"; then
    die "selftest: good case 出现未登记指标（unknown!=0，新硬门槛可能被静默降级为 info）"
  fi
  grep -q "put_mib_s.*tput" "$SELFTEST_TMP/good.log" ||
    die "selftest: put_mib_s 未按吞吐（higher）输出"
  grep -q "reopen_us.*lat" "$SELFTEST_TMP/good.log" ||
    die "selftest: reopen_us 未按延迟（lower）输出"
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/slow.json" >/dev/null 2>&1; then
    die "selftest: Rust 侧吞吐人为降 30% 仍然绿（门槛失效）"
  fi
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/rate-slow.json" >/dev/null 2>&1; then
    die "selftest: _batch_s / _page_s 速率降 30% 仍然绿（方向表漏了 higher 后缀）"
  fi
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/mib-slow.json" >/dev/null 2>&1; then
    die "selftest: _mib_s 逻辑带宽降 25% 仍然绿（方向表漏了 _mib_s higher 后缀）"
  fi
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/reopen-slow.json" >/dev/null 2>&1; then
    die "selftest: reopen_us +150% 仍然绿（漏了 reopen_us -> lower 分类）"
  fi
  # 延迟 +30%：用 20% 容差跑，验证 *_us 是 lower-is-better；默认容差属于默认值议题，
  # 这里不依赖它（默认 100% 下 +30% 本来就是绿）。
  if PERF_MAX_LATENCY_REGRESSION_PCT=20 \
    bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/latency-slow.json" >/dev/null 2>&1; then
    die "selftest: 延迟 +30%（20% 容差）仍然绿（lower-is-better 方向失效）"
  fi
  # 资源 / 计数 / 窗口指标无论变差还是改善都必须绿：它们只审计，不设门槛。
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/resource-worse.json" \
    >"$SELFTEST_TMP/resource-worse.log" 2>&1; then
    die "selftest: 资源 / 计数 / 窗口指标变差被判红（仍被当成吞吐门槛）"
  fi
  grep -q "store_bytes.*info" "$SELFTEST_TMP/resource-worse.log" ||
    die "selftest: 资源指标未以 info 行输出（审计信息丢失）"
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/resource-better.json" \
    >"$SELFTEST_TMP/resource-better.log" 2>&1; then
    die "selftest: 资源 / 计数 / 窗口指标改善被判红（方向反转后的假红）"
  fi
  # fc57926 的四个放大 / 每批派生指标：变差 / 改善都必须绿（只审计）。
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/derived-worse.json" \
    >"$SELFTEST_TMP/derived-worse.log" 2>&1; then
    die "selftest: 放大 / 每批派生指标变差被判红（仍被当成硬门槛）"
  fi
  grep -q "write_amp.*info" "$SELFTEST_TMP/derived-worse.log" ||
    die "selftest: write_amp 未按 info 行输出（审计信息丢失）"
  if ! bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/derived-better.json" \
    >"$SELFTEST_TMP/derived-better.log" 2>&1; then
    die "selftest: 放大 / 每批派生指标改善被判红（方向误判为 higher）"
  fi
  # 未知指标：不得默认按吞吐判红；完全未登记的名字必须 warn。
  if ! bash "$0" compare "$SELFTEST_TMP/unknown-baseline.json" "$SELFTEST_TMP/unknown.json" \
    >"$SELFTEST_TMP/unknown.log" 2>"$SELFTEST_TMP/unknown.err"; then
    die "selftest: 未登记方向的指标被按吞吐判红（未知指标应只审计）"
  fi
  grep -q "foo_num.*info" "$SELFTEST_TMP/unknown.log" ||
    die "selftest: foo_num 未按计数类 info 输出"
  grep -q "WARNING: 方向表未登记.*mystery_metric" "$SELFTEST_TMP/unknown.err" ||
    die "selftest: 完全未登记的指标没有 warning"
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/mode-mismatch.json" >/dev/null 2>&1; then
    die "selftest: data_append_mode 不一致仍然绿（跨模式比较未被拒绝）"
  fi
  if bash "$0" compare "$SELFTEST_TMP/baseline.json" "$SELFTEST_TMP/missing-mode.json" >/dev/null 2>&1; then
    die "selftest: current 缺少模式字段仍然绿（无法证明同口径）"
  fi
  if ! bash "$0" compare "$SELFTEST_TMP/legacy-baseline.json" "$SELFTEST_TMP/legacy-history.json" >/dev/null; then
    die "selftest: schema=1 旧基线不再兼容"
  fi

  # produce 不再做硬盘类型校验，任意盘型都能写出 baseline JSON。
  local selftest_sha
  selftest_sha=$(printf 'c%.0s' {1..40})
  if ! bash "$0" produce \
    --cpp-log "$SELFTEST_TMP/cpp-bench.log" \
    --rust-history "$SELFTEST_TMP/good.json" \
    --out "$SELFTEST_TMP/local-produce.json" \
    --bench-commit "$selftest_sha" >/dev/null; then
    die "selftest: produce 被误拒"
  fi
  [ -s "$SELFTEST_TMP/local-produce.json" ] ||
    die "selftest: produce 未写出 baseline JSON"

  echo "selftest ok: 30% throughput / rate regression and +30% latency (20% tol) are red; derived metrics cover _mib_s higher (25% drop red), reopen_us lower (+150% red), amplification / per-batch info (±30% green), and good case asserts unknown=0; retired gxhash is skipped when absent from current; good, resource worse/better (store_bytes/io/proc_io/window), unknown and legacy schema=1 are green; window throughput is audit-only; mode mismatch is rejected"
}

case "$MODE" in
  gate) run_gate ;;
  hardware) run_hardware ;;
  compare) run_compare "$@" ;;
  produce) run_produce "$@" ;;
  selftest) run_selftest ;;
  -h|--help|help)
    sed -n '2,64p' "$0"
    ;;
  *)
    die "unknown mode $MODE (use gate|compare|produce|selftest)"
    ;;
esac
