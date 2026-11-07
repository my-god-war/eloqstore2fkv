#!/usr/bin/env bash
# 构建 C++ FFI shim（libfkv_bench_ffi.so / .dylib），供
# crate/perf/fkv_bench_matrix 通过 --cpp-lib dlopen 使用。
#
# 幂等：eloqstore 源码已在 `cpp/eloqstore`（固定 commit）时直接复用；
# 缺失时调用 scripts/fetch_eloqstore.sh。CMake 增量配置 / 编译。
#
# 用法：
#   scripts/ci/build_cpp_ffi.sh
#   CPP_FFI_BUILD_DIR=/tmp/fkv-ffi-build scripts/ci/build_cpp_ffi.sh
#
# 输出：**最后一行 stdout 是共享库的绝对路径**，其余进度都走 stderr，
#       便于 `lib=$(scripts/ci/build_cpp_ffi.sh)` 这样取路径。
#
# 依赖：
#   本脚本不负责安装 apt / nix 依赖。新机器请先跑
#   `scripts/ci/build_cpp_bench.sh`（它会按需安装 boost / jsoncpp / zstd /
#   openssl / liburing / glog / gflags / curl / protobuf 等）。本脚本假设
#   cmake、ninja 与 eloqstore 的系统依赖已经可用；缺失时直接报错并提示。
#
# 环境变量：
#   CC / CXX                传给 CMake 的编译器（无空格时额外写成 -DCMAKE_*_COMPILER）
#   CMAKE_ARGS              追加给 configure 的参数（按 shell 空白切分）
#   CMAKE / NINJA           覆盖可执行文件（默认 cmake / ninja）
#   CPP_FFI_GENERATOR       CMake generator，默认 Ninja
#   CPP_FFI_BUILD_DIR       构建目录，默认
#                           ${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-ffi-build
#   CPP_FFI_SRC_DIR         eloqstore 源码目录，默认 <repo>/cpp/eloqstore
#   CPP_FFI_REFETCH=1       强制重新 fetch pinned eloqstore
#   CPP_FFI_SKIP_PATCH=1    不尝试应用 simple_bench_phases.patch（FFI 不需要）
#   JOBS                    编译并发，默认 nproc / sysctl
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SRC_DIR=${CPP_FFI_SRC_DIR:-$ROOT/cpp/eloqstore}
BUILD_DIR=${CPP_FFI_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-ffi-build}
FFI_SRC_DIR=$ROOT/cpp/bench_ffi
GENERATOR=${CPP_FFI_GENERATOR:-Ninja}
CMAKE=${CMAKE:-cmake}
NINJA=${NINJA:-ninja}
JOBS=${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)}

log() { echo "==> $*" >&2; }
die() {
  echo "build_cpp_ffi: $*" >&2
  exit 1
}

# 1) toolchain。依赖安装由 build_cpp_bench.sh 负责，这里只做存在性检查。
if ! command -v "$CMAKE" >/dev/null 2>&1 ||
  ! command -v "$NINJA" >/dev/null 2>&1; then
  die "找不到 cmake/ninja。新机器先跑 scripts/ci/build_cpp_bench.sh（会安装 apt/nix 依赖），或手动安装 cmake + ninja。"
fi
log "toolchain: $(command -v "$CMAKE") / $(command -v "$NINJA")"

[ -f "$FFI_SRC_DIR/CMakeLists.txt" ] ||
  die "缺少 $FFI_SRC_DIR/CMakeLists.txt（仓库不完整）"

# 2) pinned eloqstore。
if [ -n "${CPP_FFI_SRC_DIR:-}" ]; then
  log "eloqstore source override: $SRC_DIR"
fi
if [ ! -f "$SRC_DIR/include/kv_options.h" ] ||
  [ ! -f "$SRC_DIR/CMakeLists.txt" ] || [ "${CPP_FFI_REFETCH:-0}" = "1" ]; then
  [ -f "$ROOT/scripts/fetch_eloqstore.sh" ] ||
    die "缺少 $ROOT/scripts/fetch_eloqstore.sh（仓库不完整）；从完整 checkout 运行本脚本"
  log "fetch pinned eloqstore -> $SRC_DIR"
  bash "$ROOT/scripts/fetch_eloqstore.sh" "$SRC_DIR" >&2
else
  log "eloqstore already present: $SRC_DIR"
fi
[ -f "$SRC_DIR/include/kv_options.h" ] ||
  die "eloqstore 源码不完整（缺 $SRC_DIR/include/kv_options.h）；检查网络后重跑，或设 CPP_FFI_REFETCH=1"

# 3) simple_bench phase patch。FFI shim 本身不需要它，但如果源码树与
#    build_cpp_bench.sh 共用，可能已经打过补丁 / 需要补上。幂等处理：
#    已应用 -> 跳过；未应用且能干净应用 -> 应用；否则只警告不失败。
CPP_FFI_PATCH=${CPP_FFI_PATCH:-$ROOT/cpp/patches/simple_bench_phases.patch}
if [ "${CPP_FFI_SKIP_PATCH:-0}" != "1" ] && [ -f "$CPP_FFI_PATCH" ]; then
  if git -C "$SRC_DIR" apply --reverse --check "$CPP_FFI_PATCH" 2>/dev/null; then
    log "C++ bench phase patch already applied (not needed by FFI)"
  elif git -C "$SRC_DIR" apply --check "$CPP_FFI_PATCH" 2>/dev/null; then
    log "apply C++ bench phase patch: $CPP_FFI_PATCH"
    git -C "$SRC_DIR" apply "$CPP_FFI_PATCH"
  else
    log "warning: C++ bench phase patch 无法应用；FFI 不需要该补丁，继续构建"
  fi
else
  log "skip C++ bench phase patch (FFI does not need it)"
fi

# 4) configure。
cmake_args=(
  -B "$BUILD_DIR"
  -S "$FFI_SRC_DIR"
  -G "$GENERATOR"
  -DCMAKE_BUILD_TYPE=Release
  -DELOQSTORE_SRC="$SRC_DIR"
  # CMake >= 4 移除了 <3.5 兼容；aws-sdk-cpp 的 s2n 子模块仍声明旧版本。
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
)
if [ -n "${CMAKE_ARGS:-}" ]; then
  read -r -a _cmake_extra <<<"$CMAKE_ARGS"
  cmake_args+=("${_cmake_extra[@]}")
fi
# CMake 会读环境变量 CC/CXX；只在值不含空格时显式写进 cache，避免覆盖
# "ccache gcc" 这类包装命令。
if [ -n "${CC:-}" ] && [[ "$CC" != *" "* ]]; then
  cmake_args+=(-DCMAKE_C_COMPILER="$CC")
fi
if [ -n "${CXX:-}" ] && [[ "$CXX" != *" "* ]]; then
  cmake_args+=(-DCMAKE_CXX_COMPILER="$CXX")
fi

mkdir -p "$BUILD_DIR"
log "configure: $BUILD_DIR"
if ! "$CMAKE" "${cmake_args[@]}" >&2; then
  echo "build_cpp_ffi: cmake configure 失败。常见原因：" >&2
  echo "  - 系统依赖缺失：先跑 scripts/ci/build_cpp_bench.sh" >&2
  echo "  - FetchContent 需要网络（awssdk / glog / brpc / liburing）" >&2
  exit 1
fi

log "build fkv_bench_ffi (-j$JOBS)"
if ! "$CMAKE" --build "$BUILD_DIR" --target fkv_bench_ffi -j "$JOBS" >&2; then
  echo "build_cpp_ffi: 编译 libfkv_bench_ffi 失败。完整日志见 $BUILD_DIR" >&2
  exit 1
fi

# 5) 输出路径（stdout 只有这一行）。Linux 是 .so，macOS 是 .dylib；
#    两者都算有效，脚本只负责打印 CMake 实际产物的绝对路径。
LIB=""
for candidate in \
  "$BUILD_DIR/libfkv_bench_ffi.so" \
  "$BUILD_DIR/libfkv_bench_ffi.dylib"; do
  if [ -f "$candidate" ]; then
    LIB=$candidate
    break
  fi
done
if [ -z "$LIB" ]; then
  while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    LIB=$candidate
    break
  done < <(find "$BUILD_DIR" -type f \
    \( -name 'libfkv_bench_ffi.so*' -o -name 'libfkv_bench_ffi.dylib' \) \
    2>/dev/null | sort)
fi
[ -n "$LIB" ] ||
  die "编译完成但 $BUILD_DIR 下找不到 libfkv_bench_ffi.so/.dylib"

printf '%s\n' "$LIB"
