#!/usr/bin/env bash
# redb 风格矩阵入口：C++ FFI 与 Rust fkv_bench 走同一个 Rust trait。
#
# 用法：
#   scripts/perf_redb_matrix.sh --rows 1000000 --batch-size 1000
#   scripts/perf_redb_matrix.sh --rebuild --readers 1,4,8,16,32 --markdown /tmp/matrix.md
#
# 默认复用已构建二进制 / FFI 共享库；`--rebuild` 先跑构建脚本。C++ 源码在
# `cpp/eloqstore`（gitignored）；FFI 共享库由 `cpp/bench_ffi` 构建，
# `--cpp-lib` 通过 dlopen 加载。
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CPP_LIB=${CPP_LIB:-}
RUST_BIN=${RUST_BIN:-}
REBUILD=0
ARGS=()

log() { echo "==> $*" >&2; }

while [ "$#" -gt 0 ]; do
  case "$1" in
    --rebuild) REBUILD=1; shift ;;
    --cpp-lib) CPP_LIB=$2; shift 2 ;;
    --rust-bin) RUST_BIN=$2; shift 2 ;;
    -h | --help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *)
      ARGS+=("$1")
      shift
      ;;
  esac
done

if [ "$REBUILD" = "1" ] || [ -z "$CPP_LIB" ]; then
  if [ "$REBUILD" = "1" ]; then
    # build_cpp_bench.sh 负责 apt/nix 依赖安装与 pinned eloqstore fetch；
    # FFI 构建脚本只消费这些前置条件。
    log "prepare C++ toolchain / eloqstore deps"
    bash "$REPO/scripts/ci/build_cpp_bench.sh" >/dev/null
  fi
  log "build C++ FFI shared library"
  CPP_LIB=$(bash "$REPO/scripts/ci/build_cpp_ffi.sh")
fi
if [ "$REBUILD" = "1" ] || [ -z "$RUST_BIN" ]; then
  log "build Rust fkv_bench"
  RUST_BIN=$(bash "$REPO/scripts/ci/build_rust_bench.sh")
fi

[ -f "$CPP_LIB" ] || { echo "C++ FFI library not found: $CPP_LIB" >&2; exit 1; }
[ -x "$RUST_BIN" ] || { echo "Rust binary not executable: $RUST_BIN" >&2; exit 1; }

log "matrix: cpp-lib=$CPP_LIB rust=$RUST_BIN"
exec cargo run --release --manifest-path "$REPO/Cargo.toml" -p fkv_bench_matrix -- \
  --cpp-lib "$CPP_LIB" --rust-bin "$RUST_BIN" "${ARGS[@]}"
