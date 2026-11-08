#!/usr/bin/env bash
# Read-concurrency scaling harness: fkv (Rust) vs EloqStore C++ simple_bench.
#
# Measures random-point-read throughput while sweeping the number of concurrent
# readers (read_per_part * partitions).  Both implementations use the same
# dataset parameters and the same host.  Metrics are emitted as TSV/JSON with
# columns: impl, readers, qps, p99_us, speedup (qps / qps at readers=1).
#
# Flow (default, both sides "load-once"):
#   1. build (or reuse) fkv_bench and simple_bench
#   2. write the dataset once per implementation
#      - Rust: fkv_bench --load-only --store-dir <dir>
#      - C++ : simple_bench --workload write (single process, then exits)
#   3. for every reader count N:
#      - Rust: fkv_bench --read-only-existing --store-dir <dir> --read-per-part N
#      - C++ : simple_bench --workload read --read_per_part N --read_thds 1
#   Each read run is a fresh process that reopens the existing store, so the
#   dataset is identical across N on both sides.
#
# C++ has no single-process "load then read existing store" mode, but its
# `--workload read` does open the store_path from the INI without rewriting it
# (the same two-process sequence used by scripts/perf_local.sh).  If a local
# build cannot reopen, use --cpp-flow fresh: it rebuilds the C++ store for every
# reader count.  That mode is logged and reported as a different flow; it is
# never silently mixed with load-once numbers.
#
# Usage:
#   scripts/perf_scale_read.sh
#   scripts/perf_scale_read.sh --readers 1,2,4,8,16 --rows 2000000
#   scripts/perf_scale_read.sh --rust-bin /tmp/fkv_bench --cpp-bin /tmp/simple_bench \
#     --out /tmp/scale.tsv
#   scripts/perf_scale_read.sh --format json --out /tmp/scale.json
#   scripts/perf_scale_read.sh --cpp-flow fresh --readers 1,4,8
#
# Options:
#   --readers LIST      comma-separated reader counts, default 1,2,4,8
#   --rows N            dataset keys, default 1000000 (also C++ --max_key)
#   --batch-size N      keys per write batch, default 100
#   --kv-size BYTES     full KV width incl. 8B key, default 128
#   --partitions N      partitions, default 1 (strict parity needs 1)
#   --test-secs SECS    read window per reader count, default 10 (min 2)
#   --trials N          Rust-only: repeat each read phase N times on the same
#                        store and keep the median; C++ has no equivalent
#   --root DIR          work root, default /tmp/fkv-scale-read
#   --out FILE          write TSV/JSON here instead of stdout
#   --format tsv|json   output format, default tsv
#   --cpp-flow MODE     load-once (default) or fresh
#   --rust-bin PATH     prebuilt fkv_bench (else build)
#   --cpp-bin PATH      prebuilt simple_bench (else build)
#   --rust-only         skip the C++ side
#   --cpp-only          skip the Rust side
#   --rebuild           force rebuild of the selected sides
#   --clean             remove store/log directories when done
#   -h, --help          show this help
#
# Environment:
#   FKV_REPO_PATH        passed to scripts/ci/build_rust_bench.sh
#   CPP_BENCH_BUILD_DIR  passed to scripts/ci/build_cpp_bench.sh
#   CARGO_TARGET_DIR     passed to the Rust build
#   PYTHON               python3 executable, default python3
#
# Exit status: 0 on success, non-zero on validation / build / bench failure.
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON=${PYTHON:-python3}

READERS=1,2,4,8
ROWS=1000000
BATCH_SIZE=100
KV_SIZE=128
PARTITIONS=1
TEST_SECS=10
TRIALS=1
ROOT=/tmp/fkv-scale-read
OUT=""
FORMAT=tsv
CPP_FLOW=load-once
RUST_BIN=${PERF_SCALE_RUST_BIN:-}
CPP_BIN=${PERF_SCALE_CPP_BIN:-}
RUST_ONLY=0
CPP_ONLY=0
REBUILD=0
CLEAN=0

log() { echo "==> $*" >&2; }
warn() { echo "perf_scale_read: $*" >&2; }
die() {
  echo "perf_scale_read: $*" >&2
  exit 1
}

usage() {
  awk 'NR > 1 && /^set -euo pipefail$/ { exit } NR > 1 { print }' "$0"
}

need_uint() { # name value min
  local name=$1 value=$2 min=${3:-0}
  case "$value" in
    '' | *[!0-9]*) die "$name needs a non-negative integer, got '$value'" ;;
  esac
  [ "$value" -ge "$min" ] || die "$name must be >= $min, got $value"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --readers)
      [ "$#" -ge 2 ] || die "--readers needs a value"
      READERS=$2
      shift 2
      ;;
    --rows)
      [ "$#" -ge 2 ] || die "--rows needs a value"
      ROWS=$2
      shift 2
      ;;
    --batch-size)
      [ "$#" -ge 2 ] || die "--batch-size needs a value"
      BATCH_SIZE=$2
      shift 2
      ;;
    --kv-size)
      [ "$#" -ge 2 ] || die "--kv-size needs a value"
      KV_SIZE=$2
      shift 2
      ;;
    --partitions)
      [ "$#" -ge 2 ] || die "--partitions needs a value"
      PARTITIONS=$2
      shift 2
      ;;
    --test-secs)
      [ "$#" -ge 2 ] || die "--test-secs needs a value"
      TEST_SECS=$2
      shift 2
      ;;
    --trials)
      [ "$#" -ge 2 ] || die "--trials needs a value"
      TRIALS=$2
      shift 2
      ;;
    --root)
      [ "$#" -ge 2 ] || die "--root needs a directory"
      ROOT=$2
      shift 2
      ;;
    --out)
      [ "$#" -ge 2 ] || die "--out needs a file"
      OUT=$2
      shift 2
      ;;
    --format)
      [ "$#" -ge 2 ] || die "--format needs tsv or json"
      FORMAT=$2
      shift 2
      ;;
    --cpp-flow)
      [ "$#" -ge 2 ] || die "--cpp-flow needs load-once or fresh"
      CPP_FLOW=$2
      shift 2
      ;;
    --rust-bin)
      [ "$#" -ge 2 ] || die "--rust-bin needs a path"
      RUST_BIN=$2
      shift 2
      ;;
    --cpp-bin)
      [ "$#" -ge 2 ] || die "--cpp-bin needs a path"
      CPP_BIN=$2
      shift 2
      ;;
    --rust-only)
      RUST_ONLY=1
      shift
      ;;
    --cpp-only)
      CPP_ONLY=1
      shift
      ;;
    --rebuild)
      REBUILD=1
      shift
      ;;
    --clean)
      CLEAN=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument '$1' (see --help)"
      ;;
  esac
done

[ "$RUST_ONLY" = "1" ] && [ "$CPP_ONLY" = "1" ] &&
  die "--rust-only and --cpp-only are mutually exclusive"
case "$FORMAT" in
  tsv | json) ;;
  *) die "--format must be tsv or json, got '$FORMAT'" ;;
esac
case "$CPP_FLOW" in
  load-once | fresh) ;;
  *) die "--cpp-flow must be load-once or fresh, got '$CPP_FLOW'" ;;
esac
need_uint "--rows" "$ROWS" 1
need_uint "--batch-size" "$BATCH_SIZE" 1
need_uint "--kv-size" "$KV_SIZE" 9
need_uint "--partitions" "$PARTITIONS" 1
need_uint "--test-secs" "$TEST_SECS" 2
need_uint "--trials" "$TRIALS" 1

# Reader list: positive integers, no duplicates.  Preserve the user's order.
IFS=',' read -r -a READER_ARR <<<"$READERS"
[ "${#READER_ARR[@]}" -ge 1 ] || die "--readers must not be empty"
for value in "${READER_ARR[@]}"; do
  need_uint "--readers element" "$value" 1
done
# Detect duplicates without associative arrays (bash 3.2 on macOS).
dup=$(printf '%s\n' "${READER_ARR[@]}" | sort -n | uniq -d | head -n 1 || true)
[ -z "$dup" ] || die "duplicate reader count in --readers: $dup"

case "$ROOT" in
  *$'\n'*) die "--root must not contain a newline" ;;
  / | "" | . | ..) die "--root must not be root / cwd / empty: '$ROOT'" ;;
esac

[ "$(uname -s)" = "Linux" ] ||
  die "needs a Linux host; current $(uname -s). On macOS only 'bash -n' and '--help' are expected to work"

command -v python3 >/dev/null 2>&1 ||
  die "python3 is required for metric parsing/output"
[ -x "$PYTHON" ] || command -v "$PYTHON" >/dev/null 2>&1 ||
  die "PYTHON=$PYTHON is not executable"

# Derive the write batch count exactly like perf_local.sh / CI (0 = rows/batch).
WRITE_BATCHS=$((ROWS / BATCH_SIZE))
[ "$WRITE_BATCHS" -ge 1 ] || WRITE_BATCHS=1

RUST_STORE=$ROOT/rust-store
CPP_STORE=$ROOT/cpp-store
INI=$ROOT/cpp-bench.ini
LOGS=$ROOT/logs
RAW=$ROOT/results.raw

# ---------------------------------------------------------------------------
# Build / validate binaries
# ---------------------------------------------------------------------------
build_rust() {
  log "build Rust fkv_bench (scripts/ci/build_rust_bench.sh)"
  RUST_BIN=$(bash "$REPO/scripts/ci/build_rust_bench.sh")
}
build_cpp() {
  log "build C++ simple_bench (scripts/ci/build_cpp_bench.sh)"
  CPP_BIN=$(bash "$REPO/scripts/ci/build_cpp_bench.sh")
}

if [ "$RUST_ONLY" = "0" ] && { [ "$REBUILD" = "1" ] || [ -z "$CPP_BIN" ]; }; then
  build_cpp
fi
if [ "$CPP_ONLY" = "0" ] && { [ "$REBUILD" = "1" ] || [ -z "$RUST_BIN" ]; }; then
  build_rust
fi

if [ "$RUST_ONLY" = "0" ]; then
  [ -x "$CPP_BIN" ] || die "C++ binary not executable: $CPP_BIN"
  log "cpp bench: $CPP_BIN (flow=$CPP_FLOW)"
fi
if [ "$CPP_ONLY" = "0" ]; then
  [ -x "$RUST_BIN" ] || die "Rust binary not executable: $RUST_BIN"
  log "rust bench: $RUST_BIN (flow=load-once)"
fi

if [ "$CPP_ONLY" = "0" ]; then
  help_txt=$("$RUST_BIN" --help 2>&1 || true)
  for flag in --load-only --read-only-existing --store-dir; do
    printf '%s' "$help_txt" | grep -q -- "$flag" ||
      die "$RUST_BIN lacks $flag; rebuild the bench or pass a newer --rust-bin"
  done
  unset help_txt
fi

log "params: rows=$ROWS batch_size=$BATCH_SIZE write_batchs=$WRITE_BATCHS kv_size=$KV_SIZE partitions=$PARTITIONS test_secs=$TEST_SECS readers=$READERS"
if [ "$TRIALS" -gt 1 ] && [ "$RUST_ONLY" = "0" ]; then
  warn "trials=$TRIALS: Rust repeats each read phase on the same store; C++ has no --trials, so C++ numbers are single-pass and not directly comparable to Rust medians"
fi
log "work root: $ROOT"
mkdir -p "$ROOT" "$LOGS"
rm -rf "${RUST_STORE:?}" "${CPP_STORE:?}"
rm -f "$RAW"
: >"$RAW"

# ---------------------------------------------------------------------------
# Dataset preparation
# ---------------------------------------------------------------------------
RUST_COMMON=(
  --impl rust
  --rows "$ROWS"
  --max-key "$ROWS"
  --batch-size "$BATCH_SIZE"
  --kv-size "$KV_SIZE"
  --partitions "$PARTITIONS"
  --rng cpp
  --write-batchs "$WRITE_BATCHS"
)

write_ini() {
  cat >"$INI" <<EOF
[run]
num_threads = 1

[permanent]
store_path = ${CPP_STORE}
EOF
}

CPP_COMMON=(
  --kvoptions "$INI"
  --kv_size "$KV_SIZE"
  --batch_size "$BATCH_SIZE"
  --write_batchs "$WRITE_BATCHS"
  --partitions "$PARTITIONS"
  --max_key "$ROWS"
)

if [ "$CPP_ONLY" = "0" ]; then
  log "Rust load: --load-only -> $RUST_STORE"
  "$RUST_BIN" "${RUST_COMMON[@]}" --read-per-part 1 --test-secs 0 \
    --load-only --store-dir "$RUST_STORE" >"$LOGS/rust-load.log" 2>&1 ||
    die "Rust load failed; log: $LOGS/rust-load.log"
fi

if [ "$RUST_ONLY" = "0" ]; then
  write_ini
  if [ "$CPP_FLOW" = "load-once" ]; then
    log "C++ load: --workload write --> $CPP_STORE"
    "$CPP_BIN" "${CPP_COMMON[@]}" --workload write \
      >"$LOGS/cpp-load.log" 2>&1 ||
      die "C++ load failed; log: $LOGS/cpp-load.log"
  else
    log "C++ flow=fresh: store rebuilt for every reader count (not comparable to load-once qps)"
  fi
fi

# ---------------------------------------------------------------------------
# Metric parsing
# ---------------------------------------------------------------------------
parse_rust() { # history.json -> "qps p99"
  "$PYTHON" - "$1" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    data = json.load(handle)
metrics = {}
for run in data.get("runs", []):
    if run.get("impl") == "rust":
        metrics[run["metric"]] = float(run["value"])
for name in ("get_kv_s", "get_p99_us"):
    if name not in metrics:
        raise SystemExit(f"missing {name} in {path}")
print(f"{metrics['get_kv_s']:.6f} {metrics['get_p99_us']:.6f}")
PY
}

parse_cpp() { # log -> "qps p99"
  "$PYTHON" - "$1" <<'PY'
import re
import sys

path = sys.argv[1]
with open(path, encoding="utf-8", errors="replace") as handle:
    text = handle.read()
qps = [float(x) for x in re.findall(r"read average (\d+) QPS", text)]
p99 = [float(x) for x in re.findall(r"read speed .*?p99 (\d+) microseconds", text)]
if not qps:
    raise SystemExit(f"no 'read average ... QPS' in {path}")
if not p99:
    raise SystemExit(
        f"no 'read speed ... p99' in {path}; raise --test-secs (>= 2)"
    )
print(f"{sum(qps):.6f} {max(p99):.6f}")
PY
}

emit() { # raw out format
  "$PYTHON" - "$1" "$2" "$3" <<'PY'
import json
import sys

raw_path, out_path, fmt = sys.argv[1], sys.argv[2], sys.argv[3]
rows = []
with open(raw_path, encoding="utf-8") as handle:
    for line in handle:
        impl, readers, qps, p99 = line.split()
        rows.append(
            {
                "impl": impl,
                "readers": int(readers),
                "qps": float(qps),
                "p99_us": float(p99),
            }
        )
baseline = {row["impl"]: row["qps"] for row in rows if row["readers"] == 1}
for row in rows:
    base = baseline.get(row["impl"])
    row["speedup"] = None if not base else row["qps"] / base

if fmt == "tsv":
    lines = ["impl\treaders\tqps\tp99_us\tspeedup"]
    for row in rows:
        speedup = "-" if row["speedup"] is None else f"{row['speedup']:.3f}"
        lines.append(
            f"{row['impl']}\t{row['readers']}\t{row['qps']:.2f}"
            f"\t{row['p99_us']:.2f}\t{speedup}"
        )
    text = "\n".join(lines) + "\n"
else:
    text = json.dumps(rows, indent=2) + "\n"

if out_path == "-":
    sys.stdout.write(text)
else:
    with open(out_path, "w", encoding="utf-8") as handle:
        handle.write(text)
PY
}

# ---------------------------------------------------------------------------
# Sweep
# ---------------------------------------------------------------------------
for readers in "${READER_ARR[@]}"; do
  if [ "$CPP_ONLY" = "0" ]; then
    hist=$LOGS/rust-readers-$readers.json
    log "Rust read: readers=$readers"
    "$RUST_BIN" "${RUST_COMMON[@]}" \
      --read-per-part "$readers" \
      --test-secs "$TEST_SECS" \
      --trials "$TRIALS" \
      --workload read-only \
      --read-only-existing \
      --store-dir "$RUST_STORE" \
      --history "$hist" >"$LOGS/rust-readers-$readers.log" 2>&1 ||
      die "Rust read failed (readers=$readers); log: $LOGS/rust-readers-$readers.log"
    read -r qps p99 <<<"$(parse_rust "$hist")"
    printf 'rust\t%s\t%s\t%s\n' "$readers" "$qps" "$p99" >>"$RAW"
  fi

  if [ "$RUST_ONLY" = "0" ]; then
    cpp_log=$LOGS/cpp-readers-$readers.log
    if [ "$CPP_FLOW" = "fresh" ]; then
      rm -rf "${CPP_STORE:?}"
      write_ini
      log "C++ fresh load+read: readers=$readers"
      "$CPP_BIN" "${CPP_COMMON[@]}" --workload write \
        >"$LOGS/cpp-load-readers-$readers.log" 2>&1 ||
        die "C++ fresh load failed (readers=$readers)"
    else
      log "C++ read: readers=$readers"
    fi
    "$CPP_BIN" "${CPP_COMMON[@]}" \
      --workload read \
      --read_per_part "$readers" \
      --read_thds 1 \
      --test_secs "$TEST_SECS" >"$cpp_log" 2>&1 ||
      die "C++ read failed (readers=$readers); log: $cpp_log"
    read -r qps p99 <<<"$(parse_cpp "$cpp_log")"
    printf 'cpp\t%s\t%s\t%s\n' "$readers" "$qps" "$p99" >>"$RAW"
  fi
done

out_target=$OUT
[ -n "$out_target" ] || out_target=-
if [ "$out_target" != "-" ]; then
  mkdir -p "$(dirname "$out_target")"
fi
emit "$RAW" "$out_target" "$FORMAT"

if [ "$CLEAN" = "1" ]; then
  log "clean: removing $RUST_STORE and $CPP_STORE"
  rm -rf "${RUST_STORE:?}" "${CPP_STORE:?}"
else
  log "stores kept: $RUST_STORE $CPP_STORE (rerun without reloading not supported; use --clean)"
fi
