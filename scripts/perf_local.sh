#!/usr/bin/env bash
# 本地 / 自托管 Linux 主机（c1 x86_64、n1 aarch64）上的 perf 对比入口。
#
# 与 `.github/workflows/perf.yml` 使用同一套脚本：
#   - 构建：scripts/ci/build_cpp_bench.sh + scripts/ci/build_rust_bench.sh
#   - 运行：C++ simple_bench + Rust fkv_bench，固定 workload
#   - 汇总：scripts/perf_gate.sh produce
#
# 用法：
#   scripts/perf_local.sh
#   scripts/perf_local.sh --rebuild
#   scripts/perf_local.sh --runs 3 --trials 5 --symmetric
#   scripts/perf_local.sh --root /tmp/perf --out /tmp/perf/baseline.json \
#     --cpp-bin /tmp/build/simple_bench --rust-bin /tmp/fkv/target/release/fkv_bench
#
# 参数：
#   --root DIR       数据 / 默认输出根，默认 /tmp/fkv-perf-local
#                    （PERF_LOCAL_ROOT 同义）
#   --out FILE       产出 JSON，默认 <root>/perf-baseline.json；
#                    多轮 / symmetric 时这是第 1 对（write-read）的结果，
#                    全量统计见 <out>.summary.json
#   --summary FILE   汇总 JSON，默认 <out>.summary.json
#   --resources FILE 资源汇总 JSON，默认 <out>.resources.json
#   --runs N         每个 workload 跑 N 轮（默认 1）；每轮独立 store 目录 + 独立 JSON
#   --trials N       Rust fkv_bench 在同一个已建好的 store 上把读 / scan 阶段
#                    重复 N 次（默认 1），逐指标取中位数；写阶段仍只跑一次。
#                    只传给 Rust；C++ simple_bench 无对应 flag，>1 且非
#                    --rust-only 时会警告两侧口径不对称（不失败）。
#   --symmetric      跑三对独立子负载：write-only、read-only、write-read
#   --scan           只跑随机 scan：C++ 先 load 再 --workload scan，
#                    Rust 先 --load-only 再 --read-only-existing --workload scan
#   --read-only-inproc
#                    symmetric 的 Rust read-only 用单进程 `--workload read-only`
#                    （先静默 load 再计时读）；默认要求 `--load-only` /
#                    `--read-only-existing` 两个 flag，缺失直接报错
#   --sample-interval SECS
#                    Rust store 目录体积轮询间隔（默认 5，0＝关闭）。fkv_bench
#                    退出时删目录，只能边跑边采样；采样值是「删除前的最后/最大
#                    观测」，不是退出后 du
#   --time-bin PATH  /usr/bin/time 路径；auto（默认）自动探测 GNU time -v，
#                    传 none 关闭进程资源采集
#   --cpp-bin PATH   已构建的 simple_bench；未给或 --rebuild 时自动构建
#   --rust-bin PATH  已构建的 fkv_bench；未给或 --rebuild 时自动构建
#   --cpp-log FILE   C++ 日志输出，默认 <root>/cpp-bench.log（单轮单 workload 时）
#   --rust-history FILE  Rust 历史 JSON，默认 <root>/rust-bench.json
#   --rust-only      只跑 Rust（无可用 C++ 工具链的主机，如 c1 NixOS）
#   --rebuild        强制重新构建两侧（构建仍是增量）
#
# workload 参数（默认值与 CI / perf_gate.sh 文档逐字节一致）：
#   --rows 1000000 --batch-size 100 --test-secs 30 --write-batchs 0（0＝rows/batch）
#   --kv-size 128 --read-per-part 4 --partitions 1 --key-interval 4
#   --del-ratio 4 --scan-bytes 0
#   冒烟示例：--runs 1 --rows 20000 --batch-size 100 --test-secs 2
#
# 环境变量：
#   PERF_NVME_ROOT           由脚本固定为 --root（hardware 检查的对象就是真正
#                            写数据的目录，避免检查点和数据盘不一致）
#   PERF_RUNNER_ID           透传给 produce，默认 local-<uname -m>
#   FKV_REPO_PATH            透传给 build_rust_bench.sh（默认 <repo>/../fkv）
#   CPP_BENCH_BUILD_DIR      C++ 构建缓存，默认
#                            ${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-build
#                            （源树外，rsync / checkout 后仍可增量复用）
#   CC / CXX / CMAKE_ARGS / CPP_BENCH_* / JOBS  透传给 build_cpp_bench.sh
#   PERF_GATE_MAX_*          透传给 perf_gate.sh compare 的阈值（produce 不用）
#   PERF_RUNS / PERF_TRIALS / PERF_SYMMETRIC / PERF_READ_ONLY_INPROC / PERF_SAMPLE_INTERVAL /
#   PERF_TIME_BIN / PERF_ROWS / PERF_BATCH_SIZE / PERF_TEST_SECS /
#   PERF_WRITE_BATCHS / PERF_KV_SIZE / PERF_READ_PER_PART / PERF_PARTITIONS /
#   PERF_KEY_INTERVAL / PERF_DEL_RATIO / PERF_SCAN_BYTES
#                            与同名 CLI 参数等价，CLI 覆盖环境变量
#
# 固定 workload（与 perf_gate.sh 文档、CI 逐字节一致）：
#   write-read rows/max_key=1e6 batch=100 read_per_part=4 partitions=1
#   kv_size=128 write_batchs=10000 test_secs=30
#   fkv_bench 缺省 --rng cpp（复刻 C++ simple_bench 生成器）：实际生效的
#   key_interval / del_ratio 由 CPP_KEY_INTERVAL / CPP_DEL_RATIO 决定（当前
#   4 / 4），--key-interval / --del-ratio 只在 --rng grid 下生效。脚本仍把
#   两个 flag 传给 CLI，每轮 produced workload 会回显实际记录值。
#
# 不联网的前提：两侧二进制都已给出（--cpp-bin/--rust-bin 或环境变量），且
# eloqstore 源码已就位（C++ 构建本身需要 FetchContent；纯运行不需要网络）。
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

ROOT=${PERF_LOCAL_ROOT:-/tmp/fkv-perf-local}
OUT=""
CPP_BIN=${PERF_LOCAL_CPP_BIN:-}
RUST_BIN=${PERF_LOCAL_RUST_BIN:-}
CPP_LOG=""
RUST_HISTORY=""
REBUILD=0
RUST_ONLY=0

RUNS=${PERF_RUNS:-1}
TRIALS=${PERF_TRIALS:-1}
SYMMETRIC=${PERF_SYMMETRIC:-0}
SCAN=${PERF_SCAN:-0}
READ_ONLY_INPROC=${PERF_READ_ONLY_INPROC:-0}
SUMMARY=""
RESOURCES=""
SAMPLE_INTERVAL=${PERF_SAMPLE_INTERVAL:-5}
TIME_BIN=${PERF_TIME_BIN:-auto}

ROWS=${PERF_ROWS:-1000000}
BATCH_SIZE=${PERF_BATCH_SIZE:-100}
TEST_SECS=${PERF_TEST_SECS:-30}
WRITE_BATCHS=${PERF_WRITE_BATCHS:-0}
KV_SIZE=${PERF_KV_SIZE:-128}
READ_PER_PART=${PERF_READ_PER_PART:-4}
PARTITIONS=${PERF_PARTITIONS:-1}
KEY_INTERVAL=${PERF_KEY_INTERVAL:-4}
DEL_RATIO=${PERF_DEL_RATIO:-4}
SCAN_BYTES=${PERF_SCAN_BYTES:-0}

log() { echo "==> $*"; }
warn() { echo "perf_local: $*" >&2; }
die() {
  echo "perf_local: $*" >&2
  exit 1
}
usage() {
  # 打印文件头注释，到 `set -euo pipefail` 前为止；不要写死行号，头部
  # 注释增删后 / --help 仍要完整。
  awk 'NR > 1 && /^set -euo pipefail$/ { exit } NR > 1 { print }' "$0"
}

need_uint() { # name value [min]
  local name=$1 value=$2 min=${3:-0}
  case "$value" in
    '' | *[!0-9]*) die "$name 需要非负整数，得到 '$value'" ;;
  esac
  [ "$value" -ge "$min" ] || die "$name 必须 >= $min，得到 $value"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      [ "$#" -ge 2 ] || die "--root 需要目录"
      ROOT=$2
      shift 2
      ;;
    --out)
      [ "$#" -ge 2 ] || die "--out 需要文件"
      OUT=$2
      shift 2
      ;;
    --summary)
      [ "$#" -ge 2 ] || die "--summary 需要文件"
      SUMMARY=$2
      shift 2
      ;;
    --resources)
      [ "$#" -ge 2 ] || die "--resources 需要文件"
      RESOURCES=$2
      shift 2
      ;;
    --runs)
      [ "$#" -ge 2 ] || die "--runs 需要整数"
      RUNS=$2
      shift 2
      ;;
    --trials)
      [ "$#" -ge 2 ] || die "--trials 需要整数"
      TRIALS=$2
      shift 2
      ;;
    --symmetric)
      SYMMETRIC=1
      shift
      ;;
    --scan)
      SCAN=1
      shift
      ;;
    --read-only-inproc)
      READ_ONLY_INPROC=1
      shift
      ;;
    --sample-interval)
      [ "$#" -ge 2 ] || die "--sample-interval 需要秒数"
      SAMPLE_INTERVAL=$2
      shift 2
      ;;
    --time-bin)
      [ "$#" -ge 2 ] || die "--time-bin 需要路径或 none"
      TIME_BIN=$2
      shift 2
      ;;
    --rows)
      [ "$#" -ge 2 ] || die "--rows 需要整数"
      ROWS=$2
      shift 2
      ;;
    --batch-size)
      [ "$#" -ge 2 ] || die "--batch-size 需要整数"
      BATCH_SIZE=$2
      shift 2
      ;;
    --test-secs)
      [ "$#" -ge 2 ] || die "--test-secs 需要整数秒"
      TEST_SECS=$2
      shift 2
      ;;
    --write-batchs)
      [ "$#" -ge 2 ] || die "--write-batchs 需要整数"
      WRITE_BATCHS=$2
      shift 2
      ;;
    --kv-size)
      [ "$#" -ge 2 ] || die "--kv-size 需要字节数"
      KV_SIZE=$2
      shift 2
      ;;
    --read-per-part)
      [ "$#" -ge 2 ] || die "--read-per-part 需要整数"
      READ_PER_PART=$2
      shift 2
      ;;
    --partitions)
      [ "$#" -ge 2 ] || die "--partitions 需要整数"
      PARTITIONS=$2
      shift 2
      ;;
    --key-interval)
      [ "$#" -ge 2 ] || die "--key-interval 需要整数"
      KEY_INTERVAL=$2
      shift 2
      ;;
    --del-ratio)
      [ "$#" -ge 2 ] || die "--del-ratio 需要整数"
      DEL_RATIO=$2
      shift 2
      ;;
    --scan-bytes)
      [ "$#" -ge 2 ] || die "--scan-bytes 需要字节数"
      SCAN_BYTES=$2
      shift 2
      ;;
    --cpp-bin)
      [ "$#" -ge 2 ] || die "--cpp-bin 需要路径"
      CPP_BIN=$2
      shift 2
      ;;
    --rust-bin)
      [ "$#" -ge 2 ] || die "--rust-bin 需要路径"
      RUST_BIN=$2
      shift 2
      ;;
    --cpp-log)
      [ "$#" -ge 2 ] || die "--cpp-log 需要文件"
      CPP_LOG=$2
      shift 2
      ;;
    --rust-history)
      [ "$#" -ge 2 ] || die "--rust-history 需要文件"
      RUST_HISTORY=$2
      shift 2
      ;;
    --rebuild)
      REBUILD=1
      shift
      ;;
    --rust-only)
      RUST_ONLY=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      die "未知参数 $1（--help 查看用法）"
      ;;
  esac
done

need_uint "--runs" "$RUNS" 1
need_uint "--trials" "$TRIALS" 1
need_uint "--rows" "$ROWS" 1
need_uint "--batch-size" "$BATCH_SIZE" 1
need_uint "--test-secs" "$TEST_SECS" 0
need_uint "--write-batchs" "$WRITE_BATCHS" 0
need_uint "--kv-size" "$KV_SIZE" 9
need_uint "--read-per-part" "$READ_PER_PART" 1
need_uint "--partitions" "$PARTITIONS" 1
need_uint "--key-interval" "$KEY_INTERVAL" 1
need_uint "--del-ratio" "$DEL_RATIO" 0
need_uint "--scan-bytes" "$SCAN_BYTES" 0
case "$SAMPLE_INTERVAL" in
  '' | *[!0-9]*) die "--sample-interval 需要非负整数秒，得到 '$SAMPLE_INTERVAL'" ;;
esac

# 0＝按 rows/batch 推导（默认 1e6/100 = 10000，与历史命令行逐字节一致）。
if [ "$WRITE_BATCHS" -eq 0 ]; then
  WRITE_BATCHS=$((ROWS / BATCH_SIZE))
  [ "$WRITE_BATCHS" -ge 1 ] || WRITE_BATCHS=1
fi

case "$ROOT" in
  *$'\n'*) die "--root 不能包含换行（INI 无法表达）" ;;
  / | "" | . | ..) die "--root 不能是根 / 当前目录 / 空值: '$ROOT'" ;;
esac
OUT=${OUT:-$ROOT/perf-baseline.json}
SUMMARY=${SUMMARY:-$OUT.summary.json}
RESOURCES=${RESOURCES:-$OUT.resources.json}
CPP_LOG=${CPP_LOG:-$ROOT/cpp-bench.log}
RUST_HISTORY=${RUST_HISTORY:-$ROOT/rust-bench.json}
INI=$ROOT/perf-default.ini

[ "$(uname -s)" = "Linux" ] ||
  die "perf_local 需要在 Linux 4K 页主机（c1/n1）上运行；当前 $(uname -s)。macOS 只能做 bash -n 语法检查"

# GNU time -v：只在真正支持 -v 时启用（BSD / macOS 的 /usr/bin/time 不支持）。
if [ "$TIME_BIN" = "auto" ]; then
  TIME_BIN=""
  if [ -x /usr/bin/time ] && /usr/bin/time -v -o /dev/null true >/dev/null 2>&1; then
    TIME_BIN=/usr/bin/time
  fi
elif [ "$TIME_BIN" = "none" ] || [ -z "$TIME_BIN" ]; then
  TIME_BIN=""
elif [ -x "$TIME_BIN" ] && "$TIME_BIN" -v -o /dev/null true >/dev/null 2>&1; then
  :
else
  die "--time-bin '$TIME_BIN' 不存在或不支持 -v（GNU time）"
fi

mkdir -p "$ROOT" "$(dirname "$OUT")" "$(dirname "$CPP_LOG")" "$(dirname "$RUST_HISTORY")"

MODES="write-read"
if [ "$SCAN" = "1" ]; then
  MODES="scan-only"
elif [ "$SYMMETRIC" = "1" ]; then
  MODES="write-only read-only write-read"
fi

# ---------------------------------------------------------------------------
# 1) 磁盘 / 硬件信息（打印出来，便于区分权威与非权威结果）
# ---------------------------------------------------------------------------
print_disk_info() {
  log "perf root: $ROOT"
  if command -v df >/dev/null 2>&1; then
    df -h "$ROOT" 2>/dev/null | tail -n 2 || true
  fi
  if command -v findmnt >/dev/null 2>&1; then
    local target source fstype tran
    target=$(findmnt -T "$ROOT" -no TARGET 2>/dev/null || true)
    source=$(findmnt -T "$ROOT" -no SOURCE 2>/dev/null || true)
    fstype=$(findmnt -T "$ROOT" -no FSTYPE 2>/dev/null || true)
    tran=$(findmnt -T "$ROOT" -no TRAN 2>/dev/null || true)
    # btrfs 子卷：/dev/sda2[/@tmp] -> /dev/sda2
    if [ -n "$source" ]; then
      source=${source%%\[*}
    fi
    log "mount: target=${target:-?} source=${source:-?} fstype=${fstype:-?} tran=${tran:-?}"
    if [ -n "$source" ] && command -v lsblk >/dev/null 2>&1; then
      local base rota model
      base=$(basename "$(readlink -f "$source" 2>/dev/null || echo "$source")")
      if [ ! -r "/sys/block/$base/queue/rotational" ]; then
        base=$(lsblk -no PKNAME "$source" 2>/dev/null | head -n 1 || true)
      fi
      if [ -n "$base" ] && [ -r "/sys/block/$base/queue/rotational" ]; then
        rota=$(cat "/sys/block/$base/queue/rotational")
        model=$(cat "/sys/block/$base/device/model" 2>/dev/null || true)
        log "device: $base rotational=$rota model=${model:-unknown}"
      fi
    fi
  fi
  log "pagesize: $(getconf PAGESIZE 2>/dev/null || echo unknown)  arch: $(uname -m)"
  log "hardware: 4 KiB Linux root；任意盘型均可跑（趋势对比）"
}

print_disk_info
# hardware 检查必须针对真正写数据的目录；不接受外部指向别的挂载点。
export PERF_NVME_ROOT=$ROOT
bash "$REPO/scripts/perf_gate.sh" hardware

# ---------------------------------------------------------------------------
# 2) 二进制：给了就用，没给（或 --rebuild）就构建
# ---------------------------------------------------------------------------
if [ "$RUST_ONLY" = "0" ] && { [ "$REBUILD" = "1" ] || [ -z "$CPP_BIN" ]; }; then
  log "build C++ simple_bench"
  CPP_BIN=$(bash "$REPO/scripts/ci/build_cpp_bench.sh")
fi
if [ "$REBUILD" = "1" ] || [ -z "$RUST_BIN" ]; then
  log "build Rust fkv_bench"
  RUST_BIN=$(bash "$REPO/scripts/ci/build_rust_bench.sh")
fi

if [ "$RUST_ONLY" = "0" ]; then
  [ -x "$CPP_BIN" ] || die "C++ 二进制不可执行: $CPP_BIN"
  log "cpp bench: $CPP_BIN"
fi
[ -x "$RUST_BIN" ] || die "Rust 二进制不可执行: $RUST_BIN"
log "rust bench: $RUST_BIN"
log "runs=$RUNS trials=$TRIALS symmetric=$SYMMETRIC modes: $MODES"
log "workload args: rows=$ROWS batch_size=$BATCH_SIZE write_batchs=$WRITE_BATCHS test_secs=$TEST_SECS kv_size=$KV_SIZE read_per_part=$READ_PER_PART partitions=$PARTITIONS key_interval=$KEY_INTERVAL del_ratio=$DEL_RATIO scan_bytes=$SCAN_BYTES"
if [ "$TRIALS" -gt 1 ] && [ "$RUST_ONLY" = "0" ]; then
  warn "trials=$TRIALS：Rust fkv_bench 在同一 store 上把读/scan 阶段重复 $TRIALS 次并逐指标取中位数（写阶段只跑一次）；C++ simple_bench 无 --trials，只能整轮重复（如 --runs）。两侧口径不对称，C++ 数字不能直接与 Rust 的阶段中位数比较。"
fi
log "workload rng: fkv_bench 缺省 cpp；--rng cpp 下 key_interval/del_ratio 由 C++ 兼容生成器决定，实际记录值见每轮 produced workload"
if [ -n "$TIME_BIN" ]; then
  log "process resources: $TIME_BIN -v"
else
  log "process resources: procfs sampler（无 GNU time -v；RSS/CPU/io 为近似值）"
fi

# ---------------------------------------------------------------------------
# 3) symmetric 能力探测：Rust 侧 read-only 必须是「load 一次、计时读」
# ---------------------------------------------------------------------------
rust_help=$(mktemp "${TMPDIR:-/tmp}/fkv-perf-help.XXXXXX")
trap 'rm -f "$rust_help"' EXIT
"$RUST_BIN" --help >"$rust_help" 2>&1 || true

if [ "$SYMMETRIC" = "1" ] && [ "$READ_ONLY_INPROC" = "0" ]; then
  missing=""
  grep -q -- '--load-only' "$rust_help" || missing="$missing --load-only"
  grep -q -- '--read-only-existing' "$rust_help" || missing="$missing --read-only-existing"
  if [ -n "$missing" ]; then
    die "symmetric 的 read-only 子负载需要 fkv_bench 支持:${missing}（当前 $RUST_BIN 未实现）。
  期望命令：
    fkv_bench --impl rust --root <dir> --load-only <写入参数>
    fkv_bench --impl rust --root <dir> --read-only-existing <读取参数> --history <json>
  在 fkv 侧落地这两个 flag 之前，可加 --read-only-inproc 改用单进程
  \`--workload read-only\`（先静默 load、reopen 后计时读），口径差别见
  doc/perf-metrics.md §8。"
  fi
fi

# ---------------------------------------------------------------------------
# 4) 每轮 / 每 workload 的公共执行与采集
# ---------------------------------------------------------------------------
# Rust store 在 fkv_bench 退出时被 remove_dir_all，退出后 du 只能看到空目录。
# 采样线程在 bench 运行期间轮询（间隔 --sample-interval），取最大值作为
# 「删除前最后一次观测」，并在 JSON 里标 sampled=true。
store_sampler() {
  local watch=$1 log_file=$2 interval=$3 dir bytes files prev sample same
  same=0
  prev=""
  while :; do
    for dir in "$watch"/*/; do
      [ -d "$dir" ] || continue
      bytes=$(du -sb "$dir" 2>/dev/null | cut -f1 || true)
      case "$bytes" in
        '' | *[!0-9]*) continue ;;
      esac
      files=$(find "$dir" -type f 2>/dev/null | wc -l || true)
      sample="$bytes ${files:-0}"
      printf '%s\n' "$sample" >>"$log_file"
      # 写阶段结束后尺寸稳定；连续 3 次不变就停止采样，避免读阶段被 du 干扰。
      if [ "$sample" = "$prev" ]; then
        same=$((same + 1))
      else
        same=0
        prev=$sample
      fi
      if [ "$same" -ge 3 ]; then
        return 0
      fi
    done
    sleep "$interval"
  done
}

# 没有 GNU time -v 时的回退：直接采样 /proc/<pid>。
#   - /proc/<pid>/stat 的第 14/15 字段 utime/stime（单调累计，取最后一个样本）
#   - /proc/<pid>/status 的 VmHWM（单调高水位，取最大）
#   - /proc/<pid>/io 的 rchar/wchar/syscr/syscw/read_bytes/write_bytes（单调累计）
# 只读几个小文件，间隔 100ms；最后一个样本可能比真实终值少 ≤100ms 的尾部量，
# 因此 JSON 里标 approximate=true。
proc_sampler() { # pid out_file
  local pid=$1 out=$2 line rest key value
  local -a fields
  echo "# utime_ticks stime_ticks vmhwm_kb rchar wchar syscr syscw read_bytes write_bytes" >"$out"
  while kill -0 "$pid" 2>/dev/null; do
    local ut=0 st=0 hw=0 rchar=0 wchar=0 syscr=0 syscw=0 rbytes=0 wbytes=0
    if read -r line 2>/dev/null <"/proc/$pid/stat" && [ -n "$line" ]; then
      rest=${line##*)}
      # shellcheck disable=SC2206
      fields=($rest)
      ut=${fields[11]:-0}
      st=${fields[12]:-0}
    fi
    if [ -r "/proc/$pid/status" ]; then
      # 进程恰好在 -r 与 open 之间退出时不要往 stderr 喷错。
      { while read -r key value _; do
        [ "$key" = "VmHWM:" ] && hw=$value
      done <"/proc/$pid/status"; } 2>/dev/null
    fi
    if [ -r "/proc/$pid/io" ]; then
      { while read -r key value; do
        case "$key" in
          rchar:) rchar=$value ;;
          wchar:) wchar=$value ;;
          syscr:) syscr=$value ;;
          syscw:) syscw=$value ;;
          read_bytes:) rbytes=$value ;;
          write_bytes:) wbytes=$value ;;
        esac
      done <"/proc/$pid/io"; } 2>/dev/null
    fi
    printf '%s %s %s %s %s %s %s %s %s\n' \
      "$ut" "$st" "$hw" "$rchar" "$wchar" "$syscr" "$syscw" "$rbytes" "$wbytes" >>"$out"
    sleep 0.1
  done
}

# 运行 bench 命令并采集进程资源。
#   run_bench LOG TIMEFILE PROCFILE STREAM(0|1) CMD...
# STREAM=1 时同时把输出 tee 到终端（Rust 有阶段进度）。
run_bench() {
  local log=$1 tf=$2 pf=$3 stream=$4
  shift 4
  : >"$tf"
  : >"$pf"
  local rc=0 pid sampler
  if [ -n "$TIME_BIN" ]; then
    if [ "$stream" = "1" ]; then
      "$TIME_BIN" -v -o "$tf" "$@" 2>&1 | tee "$log" || rc=$?
    else
      "$TIME_BIN" -v -o "$tf" "$@" >"$log" 2>&1 || rc=$?
    fi
    return $rc
  fi
  if [ "$stream" = "1" ]; then
    "$@" > >(tee "$log") 2>&1 &
  else
    "$@" >"$log" 2>&1 &
  fi
  pid=$!
  proc_sampler "$pid" "$pf" &
  sampler=$!
  wait "$pid" || rc=$?
  kill "$sampler" 2>/dev/null || true
  wait "$sampler" 2>/dev/null || true
  return $rc
}

# 记录命令行（可复制复现），避免 JSON 里只剩「跑过」。
write_cmd() { # out_file cmd...
  local out_file=$1
  shift
  {
    local arg
    for arg in "$@"; do
      printf '%q ' "$arg"
    done
    printf '\n'
  } >"$out_file"
}

fail_with_log() { # log tail
  warn "$1 失败，日志尾部（$2）："
  tail -n 200 "$2" >&2 || true
  exit 1
}

mkdir -p "$(dirname "$SUMMARY")" "$(dirname "$RESOURCES")"
MANIFEST=$ROOT/.perf-local-manifest.tsv
: >"$MANIFEST"

params_json=$(printf '{"rows":%s,"batch_size":%s,"test_secs":%s,"write_batchs":%s,"kv_size":%s,"read_per_part":%s,"partitions":%s,"key_interval":%s,"del_ratio":%s,"scan_bytes":%s,"sample_interval_s":%s,"trials":%s}' \
  "$ROWS" "$BATCH_SIZE" "$TEST_SECS" "$WRITE_BATCHS" "$KV_SIZE" \
  "$READ_PER_PART" "$PARTITIONS" "$KEY_INTERVAL" "$DEL_RATIO" "$SCAN_BYTES" "$SAMPLE_INTERVAL" "$TRIALS")

for mode in $MODES; do
  idx=1
  while [ "$idx" -le "$RUNS" ]; do
    idx_tag=$(printf '%02d' "$idx")
    if [ "$RUNS" -eq 1 ] && [ "$SYMMETRIC" -eq 0 ]; then
      # 单轮单 workload：路径与历史默认完全一致。
      RUN_DIR=$ROOT
      cpp_store=$ROOT/cpp-store
      rust_store=$ROOT/rust-store
      cpp_log=$CPP_LOG
      rust_hist=$RUST_HISTORY
      produced=$OUT
    else
      RUN_DIR=$ROOT/$mode/run-$idx_tag
      cpp_store=$RUN_DIR/cpp-store
      rust_store=$RUN_DIR/rust-store
      cpp_log=$RUN_DIR/cpp-bench.log
      rust_hist=$RUN_DIR/rust-bench.json
      produced=$RUN_DIR/perf-baseline.json
    fi
    mkdir -p "$RUN_DIR"
    res_json=$RUN_DIR/resources.json

    log "[$mode $idx_tag/$RUNS] run dir: $RUN_DIR"
    rm -rf "${cpp_store:?}" "${rust_store:?}"
    rm -f "$rust_hist"
    : >"${rust_hist}.samples"
    sample_log=${rust_hist}.samples

    # C++ 的 ini 每轮重写：store_path 必须指向本轮目录。
    cat >"$INI" <<EOF
[run]
num_threads = 1

[permanent]
store_path = ${cpp_store}
EOF

    cpp_workload=""
    cpp_load_first=0
    rust_workload=""
    rust_load_first=0
    case "$mode" in
      write-only)
        cpp_workload=write
        rust_workload=write-only
        ;;
      read-only)
        # C++ 没有「load 到已有目录再纯读」的单进程模式：先 write 灌数据，
        # 再 read 只计时读阶段。
        cpp_load_first=1
        cpp_workload=read
        rust_workload=read-only
        if [ "$READ_ONLY_INPROC" != "1" ]; then
          rust_load_first=1
        fi
        ;;
      scan-only)
        # C++ 没有 scan 前的独立 load 子命令；先 write 灌数据，再 scan。
        cpp_load_first=1
        cpp_workload=scan
        rust_workload=scan
        rust_load_first=1
        ;;
      write-read)
        cpp_workload=write-read
        rust_workload=write-read
        ;;
      *)
        die "未知 mode $mode"
        ;;
    esac

    # 参数数组由函数按 workload 重建：不能用 `${arr[@]/read/write}` 之类
    # 的模式替换，那会把 `--read_per_part` / `--read_thds` 一并改掉。
    cpp_args_for() { # workload
      CPP_ARGS=(
        --kvoptions "$INI"
        --workload "$1"
        --kv_size "$KV_SIZE"
        --batch_size "$BATCH_SIZE"
        --write_batchs "$WRITE_BATCHS"
        --partitions "$PARTITIONS"
        --max_key "$ROWS"
        --read_per_part "$READ_PER_PART"
        --read_thds 1
        --test_secs "$TEST_SECS"
        --scan_bytes "$SCAN_BYTES"
        --show_write_perf=true
      )
    }
    cpp_args_for "$cpp_workload"
    cpp_args=("${CPP_ARGS[@]}")
    rust_args=(
      --impl rust
      --root "$rust_store"
      --rows "$ROWS"
      --max-key "$ROWS"
      --batch-size "$BATCH_SIZE"
      --read-per-part "$READ_PER_PART"
      --partitions "$PARTITIONS"
      --kv-size "$KV_SIZE"
      # C++ simple_bench 的 key/delete 生成器是固定 mt19937(0)；显式传 cpp
      # 避免旧二进制默认 grid 时静默产生另一套 workload。
      --rng cpp
      --key-interval "$KEY_INTERVAL"
      --del-ratio "$DEL_RATIO"
      --write-batchs "$WRITE_BATCHS"
      --scan-bytes "$SCAN_BYTES"
      --test-secs "$TEST_SECS"
      --trials "$TRIALS"
      --workload "$rust_workload"
    )

    cpp_time=$RUN_DIR/cpp-time.txt
    rust_time=$RUN_DIR/rust-time.txt
    cpp_proc=$RUN_DIR/cpp-proc.txt
    rust_proc=$RUN_DIR/rust-proc.txt
    cpp_cmd=$RUN_DIR/cpp-command.txt
    rust_cmd=$RUN_DIR/rust-command.txt
    : >"$cpp_time"
    : >"$rust_time"

    # --- C++ ---
    if [ "$RUST_ONLY" = "0" ]; then
      if [ "$cpp_load_first" = "1" ]; then
        log "[$mode $idx_tag] C++ load（--workload write，不计入指标）"
        cpp_args_for write
        write_cmd "$RUN_DIR/cpp-load-command.txt" "$CPP_BIN" "${CPP_ARGS[@]}"
        if ! "$CPP_BIN" "${CPP_ARGS[@]}" >"$RUN_DIR/cpp-load.log" 2>&1; then
          fail_with_log "C++ load" "$RUN_DIR/cpp-load.log"
        fi
        cpp_args_for "$cpp_workload"
        cpp_args=("${CPP_ARGS[@]}")
      fi
      log "[$mode $idx_tag] C++ simple_bench（--workload $cpp_workload）"
      write_cmd "$cpp_cmd" "$CPP_BIN" "${cpp_args[@]}"
      if ! run_bench "$cpp_log" "$cpp_time" "$cpp_proc" 0 \
        "$CPP_BIN" "${cpp_args[@]}"; then
        fail_with_log "C++ simple_bench" "$cpp_log"
      fi
      log "[$mode $idx_tag] cpp log: $cpp_log"
    fi

    # --- Rust ---
    SAMPLER_PID=""
    if [ "$SAMPLE_INTERVAL" -gt 0 ]; then
      store_sampler "$rust_store" "$sample_log" "$SAMPLE_INTERVAL" &
      SAMPLER_PID=$!
    fi
    if [ "$rust_load_first" = "1" ]; then
      log "[$mode $idx_tag] Rust load（--load-only，不计入指标）"
      rust_load_args=("${rust_args[@]}" --load-only)
      write_cmd "$RUN_DIR/rust-load-command.txt" "$RUST_BIN" "${rust_load_args[@]}"
      if ! "$RUST_BIN" "${rust_load_args[@]}" \
        >"$RUN_DIR/rust-load.log" 2>&1; then
        [ -n "$SAMPLER_PID" ] && kill "$SAMPLER_PID" 2>/dev/null || true
        fail_with_log "Rust --load-only" "$RUN_DIR/rust-load.log"
      fi
      rust_run_args=("${rust_args[@]}" --read-only-existing --history "$rust_hist")
    else
      rust_run_args=("${rust_args[@]}" --workload "$rust_workload" --history "$rust_hist")
    fi
    log "[$mode $idx_tag] Rust fkv_bench（${rust_workload:-read-only-existing}）"
    write_cmd "$rust_cmd" "$RUST_BIN" "${rust_run_args[@]}"
    if ! run_bench "$RUN_DIR/rust-bench.log" "$rust_time" "$rust_proc" 1 \
      "$RUST_BIN" "${rust_run_args[@]}"; then
      [ -n "$SAMPLER_PID" ] && kill "$SAMPLER_PID" 2>/dev/null || true
      fail_with_log "Rust fkv_bench" "$RUN_DIR/rust-bench.log"
    fi
    if [ -n "$SAMPLER_PID" ]; then
      kill "$SAMPLER_PID" 2>/dev/null || true
      wait "$SAMPLER_PID" 2>/dev/null || true
    fi
    log "[$mode $idx_tag] rust history: $rust_hist"

    # --- 资源采集（store 体积、进程时间、RSS、/proc/self/io）---
    python3 - "$res_json" "$mode" "$idx" "$ROOT" "$cpp_store" "$rust_store" \
      "$cpp_time" "$rust_time" "$cpp_proc" "$rust_proc" \
      "$cpp_cmd" "$rust_cmd" "$rust_hist" \
      "$sample_log" "$SAMPLE_INTERVAL" "${TIME_BIN:-none}" <<'PY'
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

(
    out_path,
    mode,
    run_index,
    root,
    cpp_store,
    rust_store,
    cpp_time,
    rust_time,
    cpp_proc,
    rust_proc,
    cpp_cmd,
    rust_cmd,
    rust_history,
    sample_log,
    sample_interval,
    time_tool,
) = sys.argv[1:17]

TIME_KEYS = {
    "User time (seconds)": ("user_s", float),
    "System time (seconds)": ("system_s", float),
    "Percent of CPU this job got": ("cpu_percent", lambda v: float(v.rstrip("%"))),
    "Elapsed (wall clock) time (h:mm:ss or m:ss)": ("elapsed_s", None),
    "Maximum resident set size (kbytes)": ("max_rss_kb", int),
    "Major (requiring I/O) page faults": ("major_page_faults", int),
    "Minor (reclaiming a frame) page faults": ("minor_page_faults", int),
    "Voluntary context switches": ("voluntary_ctxt_switches", int),
    "Involuntary context switches": ("involuntary_ctxt_switches", int),
    "File system inputs": ("fs_inputs", int),
    "File system outputs": ("fs_outputs", int),
    "Page size (bytes)": ("page_size", int),
    "Exit status": ("exit_status", int),
}
PROC_IO_KEYS = (
    "rchar",
    "wchar",
    "syscr",
    "syscw",
    "read_bytes",
    "write_bytes",
    "cancelled_write_bytes",
)
# fkv_bench 把 /proc/self/io 的 read_bytes / write_bytes 作为 runs 指标输出，
# 名字带 proc_io_ 前缀；映射回资源侧车沿用的字段名。
RUN_IO_KEYS = {
    "proc_io_read_bytes": "read_bytes",
    "proc_io_write_bytes": "write_bytes",
}


def parse_elapsed(value):
    parts = value.strip().split(":")
    try:
        numbers = [float(part) for part in parts]
    except ValueError:
        return None
    total = 0.0
    for number in numbers:
        total = total * 60.0 + number
    return total


def parse_time_v(path):
    """解析 `time -v`。按已知键前缀匹配，不能按第一个 ":" 切分：
    `Elapsed (wall clock) time (h:mm:ss or m:ss): 0:01.23` 的键里就有冒号。
    """
    if not path or not os.path.isfile(path):
        return None
    fields = {}
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            stripped = line.strip()
            for key, (name, caster) in TIME_KEYS.items():
                prefix = key + ":"
                if not stripped.startswith(prefix):
                    continue
                raw = stripped[len(prefix):].strip()
                try:
                    value = parse_elapsed(raw) if name == "elapsed_s" else caster(raw)
                except (TypeError, ValueError):
                    value = None
                if value is not None:
                    fields[name] = value
                break
    if not fields:
        return None
    fields["source"] = f"{os.path.basename(time_tool)} -v"
    return fields


PROC_SAMPLE_FIELDS = (
    "utime_ticks",
    "stime_ticks",
    "vmhwm_kb",
    "rchar",
    "wchar",
    "syscr",
    "syscw",
    "read_bytes",
    "write_bytes",
)


def parse_proc_samples(path):
    """解析 proc_sampler 的输出；无样本返回 None。

    utime/stime、VmHWM、/proc/<pid>/io 都是单调量，取样本序列里的最大值，
    等价于「最后一次读到的累计值」。
    """
    if not path or not os.path.isfile(path):
        return None
    best = dict.fromkeys(PROC_SAMPLE_FIELDS, 0)
    samples = 0
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) != len(PROC_SAMPLE_FIELDS):
                continue
            try:
                values = [int(part) for part in parts]
            except ValueError:
                continue
            samples += 1
            for key, value in zip(PROC_SAMPLE_FIELDS, values):
                if value > best[key]:
                    best[key] = value
    if samples == 0:
        return None
    try:
        ticks = os.sysconf("SC_CLK_TCK")
    except (ValueError, OSError):
        ticks = 100
    return {
        "source": "procfs sampler (/proc/<pid>)",
        "approximate": True,
        "sample_interval_s": 0.1,
        "samples": samples,
        "max_rss_kb": best["vmhwm_kb"],
        "user_s": best["utime_ticks"] / float(ticks),
        "system_s": best["stime_ticks"] / float(ticks),
        "io": {
            key: best[key]
            for key in ("rchar", "wchar", "syscr", "syscw", "read_bytes",
                        "write_bytes")
        },
    }


def du_bytes(path):
    if not os.path.isdir(path):
        return None
    try:
        result = subprocess.run(
            ["du", "-sb", path], capture_output=True, text=True, check=True
        )
        return int(result.stdout.split()[0])
    except (OSError, subprocess.CalledProcessError, ValueError, IndexError):
        total = 0
        for dir_path, _, file_names in os.walk(path):
            for name in file_names:
                try:
                    total += os.path.getsize(os.path.join(dir_path, name))
                except OSError:
                    pass
        return total


def store_footprint(path):
    info = {
        "path": path,
        "present": os.path.isdir(path),
        "bytes": du_bytes(path),
        "files": 0,
        "data_bytes": 0,
        "data_files": 0,
        "segment_bytes": 0,
        "segment_files": 0,
        "manifest_bytes": 0,
        "manifest_files": 0,
        "other_bytes": 0,
        "other_files": 0,
    }
    if not info["present"]:
        return info
    for dir_path, _, file_names in os.walk(path):
        for name in file_names:
            try:
                size = os.path.getsize(os.path.join(dir_path, name))
            except OSError:
                continue
            info["files"] += 1
            prefix = None
            for candidate in ("data", "segment", "manifest"):
                if name.startswith(candidate + "_"):
                    prefix = candidate
                    break
            if prefix is None:
                info["other_bytes"] += size
                info["other_files"] += 1
            else:
                info[prefix + "_bytes"] += size
                info[prefix + "_files"] += 1
    return info


def sampled_footprint(path, log_path, interval):
    try:
        has_data = os.path.isdir(path) and any(os.scandir(path))
    except OSError:
        has_data = False
    info = {
        "path": path,
        "present": has_data,
        "bytes": None,
        "files": None,
        "sampled": True,
        "sample_interval_s": interval,
        "samples": 0,
        "note": "fkv_bench 退出时 remove_dir_all；数值是删除前轮询到的最大值",
    }
    best_bytes = None
    best_files = None
    samples = 0
    if interval and os.path.isfile(log_path):
        with open(log_path, encoding="utf-8", errors="replace") as handle:
            for line in handle:
                parts = line.split()
                if len(parts) != 2:
                    continue
                try:
                    value = int(parts[0])
                    count = int(parts[1])
                except ValueError:
                    continue
                samples += 1
                if best_bytes is None or value > best_bytes:
                    best_bytes = value
                if best_files is None or count > best_files:
                    best_files = count
    info["samples"] = samples
    if best_bytes is not None:
        info["bytes"] = best_bytes
        info["files"] = best_files
    else:
        info["sampled"] = False
        info["note"] = (
            "未开启采样（--sample-interval 0）或采样窗口内没有观测到 store 目录"
        )
    return info


def proc_self_io(path):
    if not path or not os.path.isfile(path):
        return None, "rust history 不存在"
    try:
        with open(path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, ValueError) as err:
        return None, f"rust history 不可读: {err}"
    if isinstance(data.get("io"), dict):
        return data["io"], None
    found = {key: data[key] for key in PROC_IO_KEYS if key in data}
    if found:
        return found, None
    metrics = {
        run.get("metric"): run.get("value")
        for run in data.get("runs", [])
        if run.get("impl") == "rust"
    }
    found = {
        dst: metrics[src]
        for src, dst in RUN_IO_KEYS.items()
        if src in metrics
    }
    if found:
        return found, None
    return None, "fkv_bench 尚未输出 /proc/self/io（不臆造）"


def read_command(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return handle.read().strip()
    except OSError:
        return None


try:
    nproc = os.cpu_count()
except NotImplementedError:
    nproc = None

cpp = {
    "command": read_command(cpp_cmd),
    "process": parse_time_v(cpp_time) or parse_proc_samples(cpp_proc),
    "store": store_footprint(cpp_store),
}
rust_io, rust_io_note = proc_self_io(rust_history)
rust = {
    "command": read_command(rust_cmd),
    "process": parse_time_v(rust_time) or parse_proc_samples(rust_proc),
    "store": sampled_footprint(rust_store, sample_log, int(sample_interval)),
    "proc_self_io": rust_io,
}

absent = []
if cpp["process"] is None:
    absent.append("cpp.process（无 GNU time -v，且 /proc 采样为空）")
if rust["process"] is None:
    absent.append("rust.process（无 GNU time -v，且 /proc 采样为空）")
if rust["store"]["bytes"] is None:
    absent.append("rust.store.bytes")
if rust_io is None:
    absent.append(f"rust.proc_self_io（{rust_io_note}）")
for key in ("device_iops", "device_throughput", "fsync_num", "fsync_ms",
            "write_amp_external", "compaction_num", "cache_hit", "reopen_ms"):
    absent.append(key)

resources = {
    "schema": 1,
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "mode": mode,
    "run": int(run_index),
    "environment": {
        "hostname": os.uname().nodename,
        "kernel": os.uname().release,
        "arch": os.uname().machine,
        "nproc": nproc,
        "page_size": os.sysconf("SC_PAGE_SIZE"),
        "root": root,
    },
    "cpp": cpp,
    "rust": rust,
    "absent": absent,
}
with open(out_path, "w", encoding="utf-8") as handle:
    json.dump(resources, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
print(f"wrote {out_path}")
PY

    # --- produce / rust-only 伪 JSON ---
    if [ "$RUST_ONLY" = "1" ]; then
      python3 - "$rust_hist" "$produced" <<'PY'
import json
import sys

history_path, out_path = sys.argv[1], sys.argv[2]
with open(history_path, encoding="utf-8") as handle:
    data = json.load(handle)
metrics = {
    run["metric"]: float(run["value"])
    for run in data.get("runs", [])
    if run.get("impl") == "rust"
}
result = {
    "schema": 2,
    "source": "fkv perf-local rust-only",
    "runner": "local-rust-only",
    "page_size": 4096,
    "bench_commit": data.get("bench_commit", "unknown"),
    "data_append_mode": data.get("data_append_mode"),
    "direct_io": data.get("direct_io"),
    "workload": {k: v for k, v in data.items() if k not in ("runs",)},
    "cpp": {"metrics": {}},
    "rust": {"metrics": metrics},
    "ratios": {},
}
with open(out_path, "w", encoding="utf-8") as handle:
    json.dump(result, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
print(f"wrote {out_path} (rust-only, no cpp metrics)")
PY
    else
      export PERF_RUNNER_ID="${PERF_RUNNER_ID:-local-$(uname -m)}"
      log "[$mode $idx_tag] produce perf json (runner=$PERF_RUNNER_ID)"
      bench_commit=${PERF_BENCH_COMMIT:-}
      if [ -z "$bench_commit" ] && [ -d "${FKV_REPO_PATH:-}/.git" ]; then
        bench_commit=$(git -C "$FKV_REPO_PATH" rev-parse HEAD 2>/dev/null || true)
      fi
      bash "$REPO/scripts/perf_gate.sh" produce \
        --cpp-log "$cpp_log" \
        --rust-history "$rust_hist" \
        --resources "$res_json" \
        --bench-commit "$bench_commit" \
        --data-append-mode "${PERF_DATA_APPEND_MODE:-0}" \
        --direct-io "${PERF_DIRECT_IO:-1}" \
        --out "$produced"
    fi

    # 回显 fkv_bench 实际写进 history 的 workload 口径：--rng cpp 下
    # key_interval / del_ratio 由 C++ 兼容生成器决定，不能按 --key-interval /
    # --del-ratio 入参假定。
    effective_workload=$(python3 - "$produced" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)
workload = data.get("workload") or {}
keys = (
    "workload", "rng", "key_interval", "del_ratio", "rows", "batch_size",
    "read_per_part", "partitions", "kv_size", "test_secs", "write_batchs",
    "scan_bytes",
)
print(" ".join(f"{key}={workload[key]}" for key in keys if key in workload))
PY
)
    log "[$mode $idx_tag] workload (effective): ${effective_workload:-<unavailable>}"

    printf '%s\t%s\t%s\t%s\n' "$mode" "$idx" "$produced" "$res_json" >>"$MANIFEST"
    if [ -z "${FIRST_PRODUCED:-}" ]; then
      FIRST_PRODUCED=$produced
    fi
    idx=$((idx + 1))
  done
done

# 多轮 / symmetric 时 --out 仍指向「第 1 对」的结果，方便沿用既有读取路径；
# 全量统计在 <out>.summary.json 里。单轮单 workload 时两者本来就是同一个文件。
FIRST_PRODUCED=${FIRST_PRODUCED:-}
if [ -n "$FIRST_PRODUCED" ] && [ "$FIRST_PRODUCED" != "$OUT" ]; then
  cp "$FIRST_PRODUCED" "$OUT"
  log "out: $OUT（= 第 1 对 $FIRST_PRODUCED；多轮统计见 summary）"
fi

# ---------------------------------------------------------------------------
# 5) 汇总：median / min / max + 资源聚合
# ---------------------------------------------------------------------------
log "summarize: $SUMMARY"
log "resources: $RESOURCES"
python3 - "$MANIFEST" "$SUMMARY" "$RESOURCES" "$RUNS" "$SYMMETRIC" \
  "$ROOT" "$params_json" <<'PY'
import json
import statistics
import sys
from datetime import datetime, timezone

(
    manifest_path,
    summary_path,
    resources_path,
    runs_requested,
    symmetric,
    root,
    params_json,
) = sys.argv[1:8]

STAT_KEYS = [
    "put_kv_s",
    "get_kv_s",
    "put_p99_us",
    "get_p99_us",
    "rss_peak_kb",
    "cpp_rss_peak_kb",
    "cpp_store_bytes",
    "rust_store_bytes",
]

entries = []
with open(manifest_path, encoding="utf-8") as handle:
    for line in handle:
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 4:
            continue
        entries.append({"mode": parts[0], "run": int(parts[1]),
                        "produced": parts[2], "resources": parts[3]})

modes = {}
resource_runs = []
for entry in entries:
    mode = entry["mode"]
    slot = modes.setdefault(mode, {"runs": [], "stats": {}})
    metrics = {}
    workload = {}
    try:
        with open(entry["produced"], encoding="utf-8") as handle:
            data = json.load(handle)
        metrics.update(data.get("cpp", {}).get("metrics") or {})
        metrics.update(data.get("rust", {}).get("metrics") or {})
        workload = data.get("workload") or {}
    except (OSError, ValueError) as err:
        metrics = {}
        workload = {"error": str(err)}

    resources = {}
    try:
        with open(entry["resources"], encoding="utf-8") as handle:
            resources = json.load(handle)
    except (OSError, ValueError):
        resources = {}

    cpp_process = ((resources.get("cpp") or {}).get("process") or {})
    if "max_rss_kb" in cpp_process:
        metrics["cpp_rss_peak_kb"] = float(cpp_process["max_rss_kb"])
    cpp_store = ((resources.get("cpp") or {}).get("store") or {})
    if cpp_store.get("bytes") is not None:
        metrics["cpp_store_bytes"] = float(cpp_store["bytes"])
    rust_store = ((resources.get("rust") or {}).get("store") or {})
    if rust_store.get("bytes") is not None:
        metrics["rust_store_bytes"] = float(rust_store["bytes"])

    slot["runs"].append({
        "run": entry["run"],
        "produced": entry["produced"],
        "resources": entry["resources"],
        "workload": workload,
        "metrics": metrics,
    })
    if resources:
        resource_runs.append({
            "mode": mode,
            "run": entry["run"],
            "resources": resources,
        })

for mode, slot in modes.items():
    stats = {}
    for key in STAT_KEYS:
        values = [run["metrics"][key] for run in slot["runs"] if key in run["metrics"]]
        if not values:
            continue
        stats[key] = {
            "n": len(values),
            "min": min(values),
            "median": statistics.median(values),
            "max": max(values),
            "mean": statistics.fmean(values),
            "stdev": statistics.stdev(values) if len(values) > 1 else 0.0,
            "values": values,
        }
    slot["stats"] = stats

summary = {
    "schema": 1,
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "root": root,
    "runs_requested": int(runs_requested),
    "symmetric": symmetric == "1",
    "params": json.loads(params_json),
    "stat_keys": STAT_KEYS,
    "modes": modes,
    "notes": [
        "median/min/max 只在同一 runner、同一 workload、同一参数下可比",
        "put_kv_s / get_kv_s 取自 produce 后的 rust.metrics（缺省回落到 cpp.metrics）",
        "rss_peak_kb 是 fkv_bench 自报的 VmHWM；cpp_rss_peak_kb 来自 /usr/bin/time -v",
        "cpp_store_bytes 是退出后 du -sb；rust_store_bytes 是退出前轮询最大值",
    ],
}
with open(summary_path, "w", encoding="utf-8") as handle:
    json.dump(summary, handle, ensure_ascii=False, indent=2)
    handle.write("\n")

resources_doc = {
    "schema": 1,
    "generated_at": summary["generated_at"],
    "root": root,
    "runs_requested": int(runs_requested),
    "symmetric": summary["symmetric"],
    "params": summary["params"],
    "environment": (resource_runs[0]["resources"].get("environment")
                    if resource_runs else None),
    "runs": resource_runs,
}
with open(resources_path, "w", encoding="utf-8") as handle:
    json.dump(resources_doc, handle, ensure_ascii=False, indent=2)
    handle.write("\n")

lines = []
for mode, slot in modes.items():
    for key, stat in slot["stats"].items():
        lines.append(
            f"{mode:<12} {key:<18} n={stat['n']} "
            f"median={stat['median']:.1f} min={stat['min']:.1f} max={stat['max']:.1f}"
        )
print(f"wrote {summary_path}")
print(f"wrote {resources_path}")
for line in lines:
    print(line)
PY

if [ "$RUST_ONLY" = "1" ]; then
  log "done (rust-only): $SUMMARY"
else
  log "done: $OUT"
fi
log "汇总：$SUMMARY（median/min/max）；资源：$RESOURCES（store 体积 / RSS / CPU / 采样）"
