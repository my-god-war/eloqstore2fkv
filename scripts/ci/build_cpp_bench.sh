#!/usr/bin/env bash
# 构建 C++ `simple_bench`（Release），供 perf 基线 / 本地对比使用。
#
# 幂等：`scripts/fetch_eloqstore.sh` 已把 eloqstore 固定到 COMPAT_BASELINE 的
# commit，重复执行只在 commit 不符时重建源码树；CMake 增量配置 / 编译。
#
# 用法：
#   scripts/ci/build_cpp_bench.sh
#   CPP_BENCH_BUILD_DIR=/tmp/eloq-build scripts/ci/build_cpp_bench.sh
#
# 输出：**最后一行 stdout 是 simple_bench 的绝对路径**，其余进度都走 stderr，
#       便于 `bin=$(scripts/ci/build_cpp_bench.sh)` 这样取路径。
#
# 环境变量：
#   CC / CXX                传给 CMake 的编译器（无空格时额外写成 -DCMAKE_*_COMPILER）
#   CMAKE_ARGS              追加给 configure 的参数（按 shell 空白切分）
#   CMAKE / NINJA           覆盖可执行文件（默认 cmake / ninja）
#   CPP_BENCH_GENERATOR     CMake generator，默认 Ninja
#   CPP_BENCH_BUILD_DIR     构建目录，默认
#                           ${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-build
#                           （源树之外，rsync / git checkout 不会清掉）
#   CPP_BENCH_SRC_DIR       eloqstore 源码目录，默认 <repo>/cpp/eloqstore
#   CPP_BENCH_SKIP_APT=1    不自动用 sudo apt-get 安装缺失的 dev 包
#   CPP_BENCH_FORCE_NIX=1   即使系统有 cmake 也走 nix shell（NixOS 上缺 boost 时用）
#   JOBS                    编译并发，默认 nproc
#   FETCHCONTENT_BASE_DIR   透传给 eloqstore 的 FetchContent 缓存（可选；
#                           不设则用 <build>/_deps，跨次构建仍可复用）
#
# 依赖解析策略：
#   1. Ubuntu/Debian：dpkg-query 检查缺失包，自动 sudo apt-get install
#      （CPP_BENCH_SKIP_APT=1 可关闭）。
#   2. 仍缺 cmake/ninja 且系统有 nix：用 `nix shell nixpkgs#... -c` 包一层
#      （NixOS c1 的默认路径）。
#   3. 都没有：给出可直接复制执行的安装命令后失败。
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SRC_DIR=${CPP_BENCH_SRC_DIR:-$ROOT/cpp/eloqstore}
BUILD_DIR=${CPP_BENCH_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-build}
GENERATOR=${CPP_BENCH_GENERATOR:-Ninja}
CMAKE=${CMAKE:-cmake}
NINJA=${NINJA:-ninja}
JOBS=${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)}

log() { echo "==> $*" >&2; }
die() {
  echo "build_cpp_bench: $*" >&2
  exit 1
}

APT_PACKAGES=(
  build-essential cmake ninja-build git curl ca-certificates pkg-config
  libboost-all-dev libjsoncpp-dev libzstd-dev libssl-dev libaio-dev
  liburing-dev libgoogle-glog-dev libgflags-dev libcurl4-openssl-dev
  libprotobuf-dev libprotoc-dev protobuf-compiler libleveldb-dev
  libzmq3-dev cppzmq-dev
)

have_cmake_stack() {
  command -v "$CMAKE" >/dev/null 2>&1 && command -v "$NINJA" >/dev/null 2>&1
}

install_apt_deps() {
  command -v dpkg-query >/dev/null 2>&1 || return 0
  command -v apt-get >/dev/null 2>&1 || return 0
  local pkg missing=()
  for pkg in "${APT_PACKAGES[@]}"; do
    if ! dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null |
      grep -q "install ok installed"; then
      missing+=("$pkg")
    fi
  done
  if [ "${#missing[@]}" -eq 0 ]; then
    return 0
  fi
  if [ "${CPP_BENCH_SKIP_APT:-0}" = "1" ]; then
    log "missing apt packages (CPP_BENCH_SKIP_APT=1, 不安装): ${missing[*]}"
    return 0
  fi
  log "安装缺失的 apt 包：${missing[*]}"
  local sudo=()
  if [ "$(id -u)" != "0" ]; then
    if ! command -v sudo >/dev/null 2>&1; then
      log "缺少 apt 包但无 sudo，跳过安装（可能由 cmake configure 报错）：${missing[*]}"
      return 0
    fi
    sudo=(sudo)
  fi
  "${sudo[@]}" apt-get update >&2
  "${sudo[@]}" apt-get install -y --no-install-recommends "${missing[@]}" >&2
}

NIX_PACKAGES=(
  nixpkgs#cmake
  nixpkgs#ninja
  nixpkgs#pkg-config
  nixpkgs#git
  nixpkgs#boost
  nixpkgs#jsoncpp
  nixpkgs#zstd
  nixpkgs#zlib
  nixpkgs#openssl
  nixpkgs#liburing
  nixpkgs#libaio
  nixpkgs#glog
  nixpkgs#gflags
  nixpkgs#curl
  nixpkgs#protobuf
  nixpkgs#zeromq
  nixpkgs#cppzmq
  nixpkgs#abseil-cpp
  nixpkgs#libsodium
  nixpkgs#leveldb
  nixpkgs#gcc
)

run_tool() {
  if [ "$USE_NIX" = "1" ]; then
    nix shell "${NIX_PACKAGES[@]}" -c "$@"
  else
    "$@"
  fi
}

# 1) Ubuntu/Debian 依赖（有 dpkg/apt 时）。NixOS 没有这些命令，会直接跳过。
#    每次都做一遍包状态检查：便宜，且能覆盖「cmake 在但 boost/jsoncpp 缺失」。
install_apt_deps

# 2) 选 native 还是 nix shell。
USE_NIX=0
if [ "${CPP_BENCH_FORCE_NIX:-0}" = "1" ] && command -v nix >/dev/null 2>&1; then
  USE_NIX=1
  log "CPP_BENCH_FORCE_NIX=1：使用 nix shell"
elif have_cmake_stack; then
  log "toolchain: $(command -v "$CMAKE") / $(command -v "$NINJA")"
elif command -v nix >/dev/null 2>&1; then
  USE_NIX=1
  log "cmake/ninja 缺失，改用 nix shell（首次会下载 nixpkgs 依赖，属正常）"
  log "nix packages: ${NIX_PACKAGES[*]}"
else
  die "找不到 cmake/ninja。Ubuntu: sudo apt-get install -y build-essential cmake ninja-build libboost-all-dev libjsoncpp-dev libzstd-dev libssl-dev libaio-dev liburing-dev libgoogle-glog-dev libgflags-dev libcurl4-openssl-dev libprotobuf-dev libprotoc-dev protobuf-compiler libleveldb-dev libzmq3-dev cppzmq-dev；NixOS: 安装 nix 或先 nix shell nixpkgs#cmake nixpkgs#ninja"
fi

# `nix shell` 只把 bin/ 放进 PATH，CMake 不会自己去 /nix/store 找 boost /
# jsoncpp 等库。这里用 `nix build --print-out-paths` 既 **realize** 了 out/dev
# 输出，又把路径注入 CMAKE_PREFIX_PATH / PKG_CONFIG_PATH。只 `nix eval` 拿到
# 的 dev path 可能还没下载到 store，find_package 会直接失败。
if [ "$USE_NIX" = "1" ]; then
  nix_prefixes=()
  nix_pc_paths=()
  nix_includes=()
  # 很多包把 headers / CMake config 放在 split 的 `dev` 输出里（jsoncpp、
  # boost、openssl、curl、zlib 都是），只加 default output 会让 find_package 失败。
  for pkg in boost jsoncpp zstd zlib openssl curl protobuf zeromq cppzmq abseil-cpp libsodium leveldb glog gflags libaio liburing; do
    # openssl 的 default outputs 是 bin/man/dev，`libcrypto.so` 在单独的 out
    # 输出里；必须逐个属性取，缺一个属性不能让整组 dev 输出一起丢失。
    for attr in "$pkg" "$pkg.out" "$pkg.lib" "$pkg.dev"; do
      outs=$(nix build --no-link --print-out-paths "nixpkgs#${attr}" 2>/dev/null || true)
      while IFS= read -r out; do
        [ -n "$out" ] || continue
        nix_prefixes+=("$out")
        [ -d "$out/include" ] && nix_includes+=("-I$out/include")
        for sub in lib/pkgconfig share/pkgconfig; do
          [ -d "$out/$sub" ] && nix_pc_paths+=("$out/$sub")
        done
      done <<<"$outs"
    done
  done
  if [ "${#nix_prefixes[@]}" -gt 0 ]; then
    _ifs=$IFS
    IFS=:
    CMAKE_PREFIX_PATH="${nix_prefixes[*]}${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"
    if [ "${#nix_pc_paths[@]}" -gt 0 ]; then
      PKG_CONFIG_PATH="${nix_pc_paths[*]}${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    fi
    IFS=$_ifs
    export CMAKE_PREFIX_PATH PKG_CONFIG_PATH
    log "nix prefixes: ${#nix_prefixes[@]} outPaths -> CMAKE_PREFIX_PATH"
  fi
  # eloqstore 只 `find_library(ZSTD_LIBRARY)`，不找 zstd.h；Nix 的 headers 在
  # 独立 dev 输出里，必须显式加 include 路径。
  zstd_inc=$(nix eval --raw nixpkgs#zstd.dev.outPath 2>/dev/null || true)/include
  if [ -d "$zstd_inc" ]; then
    NIX_CMAKE_FLAGS="-I$zstd_inc"
  fi
  # eloqstore 用 `${Boost_INCLUDE_DIRS}` 但没 find_package(Boost)；jsoncpp
  # 按 Ubuntu 布局 include `<jsoncpp/json/json.h>`，Nix dev 只有 `include/json`。
  boost_inc=$(nix eval --raw nixpkgs#boost.dev.outPath 2>/dev/null || true)/include
  json_dev=$(nix eval --raw nixpkgs#jsoncpp.dev.outPath 2>/dev/null || true)
  if [ -d "$boost_inc" ]; then
    NIX_CMAKE_FLAGS="${NIX_CMAKE_FLAGS:+$NIX_CMAKE_FLAGS }-I$boost_inc"
  fi
  if [ -d "$json_dev/include/json" ]; then
    mkdir -p "$BUILD_DIR/nix-include/jsoncpp"
    ln -sfn "$json_dev/include/json" "$BUILD_DIR/nix-include/jsoncpp/json"
    NIX_CMAKE_FLAGS="${NIX_CMAKE_FLAGS:+$NIX_CMAKE_FLAGS }-I$BUILD_DIR/nix-include"
  fi
  cppzmq_inc=$(nix eval --raw nixpkgs#cppzmq.outPath 2>/dev/null || true)/include
  if [ -d "$cppzmq_inc" ]; then
    NIX_CMAKE_FLAGS="${NIX_CMAKE_FLAGS:+$NIX_CMAKE_FLAGS }-I$cppzmq_inc"
  fi
  # 有些头（absl、protobuf 的依赖）只通过 CMAKE_PREFIX_PATH 不会自动进
  # compile line；把所有 nix 输出的 include 都显式加上，避免逐个补。
  if [ "${#nix_includes[@]}" -gt 0 ]; then
    NIX_CMAKE_FLAGS="${NIX_CMAKE_FLAGS:+$NIX_CMAKE_FLAGS }${nix_includes[*]}"
    # brpc / awssdk 的子 target 不继承顶层 CMAKE_CXX_FLAGS；用 GCC 的
    # CPLUS_INCLUDE_PATH 兜底，保证生成代码也能找到 absl / protobuf 依赖头。
    _inc=""
    for flag in "${nix_includes[@]}"; do
      _inc="${_inc:+$_inc:}${flag#-I}"
    done
    export CPLUS_INCLUDE_PATH="${_inc}${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}"
    export C_INCLUDE_PATH="${_inc}${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
  fi
  # page.h 直接用 sysconf/_SC_PAGESIZE，但没 include <unistd.h>；Ubuntu 的
  # libstdc++ 头会间接带入，Nix GCC 的包含顺序不保证，显式强制包含。
  NIX_CMAKE_FLAGS="${NIX_CMAKE_FLAGS:+$NIX_CMAKE_FLAGS }-include unistd.h -D_GNU_SOURCE"
fi

# 3) pinned eloqstore。
[ -f "$ROOT/scripts/fetch_eloqstore.sh" ] ||
  die "缺少 $ROOT/scripts/fetch_eloqstore.sh（仓库不完整）；从完整 checkout 运行本脚本"
if [ ! -f "$SRC_DIR/include/kv_options.h" ] || [ "${CPP_BENCH_REFETCH:-0}" = "1" ]; then
  log "fetch pinned eloqstore -> $SRC_DIR"
  bash "$ROOT/scripts/fetch_eloqstore.sh" "$SRC_DIR" >&2
else
  log "eloqstore already present: $SRC_DIR"
fi
[ -f "$SRC_DIR/CMakeLists.txt" ] ||
  die "eloqstore 源码不完整（缺 $SRC_DIR/CMakeLists.txt）；检查网络后重跑，或设 CPP_BENCH_REFETCH=1"

# pinned simple_bench 没有 redb 风格的 delete/compact phase。补丁在 fetch
# 之后幂等应用；已应用时 `--reverse --check` 通过，直接跳过。
CPP_BENCH_PATCH=${CPP_BENCH_PATCH:-$ROOT/cpp/patches/simple_bench_phases.patch}
if [ -f "$CPP_BENCH_PATCH" ]; then
  if git -C "$SRC_DIR" apply --check "$CPP_BENCH_PATCH" 2>/dev/null; then
    log "apply C++ bench phase patch: $CPP_BENCH_PATCH"
    git -C "$SRC_DIR" apply "$CPP_BENCH_PATCH"
  elif git -C "$SRC_DIR" apply --reverse --check "$CPP_BENCH_PATCH" 2>/dev/null; then
    log "C++ bench phase patch already applied"
  else
    die "C++ bench patch 无法应用（源码 commit 不符？）：$CPP_BENCH_PATCH"
  fi
fi

# 4) configure。
cmake_args=(
  -B "$BUILD_DIR"
  -S "$SRC_DIR"
  -G "$GENERATOR"
  -DCMAKE_BUILD_TYPE=Release
  -DSKIP_CREATE_BUCKET=ON
  -DWITH_BENCHMARK=ON
  # CMake >= 4 移除了 <3.5 兼容；aws-sdk-cpp 的 s2n 子模块仍声明旧版本。
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
)
if [ -n "${CMAKE_ARGS:-}" ]; then
  read -r -a _cmake_extra <<<"$CMAKE_ARGS"
  cmake_args+=("${_cmake_extra[@]}")
fi
# 环境变量只在首次 configure 初始化 cache；重复 configure 会沿用旧值。
# 显式 -D 才能让 nix dev/out 前缀在重试时生效。
if [ -n "${CMAKE_PREFIX_PATH:-}" ]; then
  cmake_args+=(-DCMAKE_PREFIX_PATH="$CMAKE_PREFIX_PATH")
fi
if [ -n "${NIX_CMAKE_FLAGS:-}" ]; then
  cmake_args+=(
    -DCMAKE_C_FLAGS="${CMAKE_C_FLAGS:+$CMAKE_C_FLAGS }$NIX_CMAKE_FLAGS"
    -DCMAKE_CXX_FLAGS="${CMAKE_CXX_FLAGS:+$CMAKE_CXX_FLAGS }$NIX_CMAKE_FLAGS"
  )
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
if ! run_tool "$CMAKE" "${cmake_args[@]}" >&2; then
  echo "build_cpp_bench: cmake configure 失败。常见原因：" >&2
  echo "  - FetchContent 需要网络（awssdk / glog / brpc / liburing）" >&2
  echo "  - 缺 libzmq / jsoncpp / curl / OpenSSL / boost_context 开发包" >&2
  echo "  - NixOS 上先进入 nix shell；Ubuntu 上按上面 install_apt_deps 的包名安装" >&2
  exit 1
fi

log "build simple_bench (-j$JOBS)"
if ! run_tool "$CMAKE" --build "$BUILD_DIR" --target simple_bench -j "$JOBS" >&2; then
  echo "build_cpp_bench: 编译 simple_bench 失败。完整日志见 $BUILD_DIR" >&2
  exit 1
fi

# 可选：在同一个已解析好的 nix / apt 依赖环境里顺带构建 FFI shim。
# 必须复用本进程的 CMAKE_PREFIX_PATH / PKG_CONFIG_PATH，否则 Nix 下
# 找不到 zstd / jsoncpp / boost 头。
if [ "${CPP_BENCH_WITH_FFI:-0}" = "1" ]; then
  FFI_BUILD_DIR=${CPP_FFI_BUILD_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/cpp-ffi-build}
  log "configure fkv_bench_ffi: $FFI_BUILD_DIR"
  ffi_cmake_args=(
    -B "$FFI_BUILD_DIR"
    -S "$ROOT/cpp/bench_ffi"
    -G "$GENERATOR"
    -DCMAKE_BUILD_TYPE=Release
    -DELOQSTORE_SRC="$SRC_DIR"
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  )
  if [ -n "${NIX_CMAKE_FLAGS:-}" ]; then
    ffi_cmake_args+=("-DCMAKE_CXX_FLAGS=$NIX_CMAKE_FLAGS")
  fi
  if ! run_tool "$CMAKE" "${ffi_cmake_args[@]}" >&2; then
    echo "build_cpp_bench: FFI configure 失败（CPP_BENCH_WITH_FFI=1）" >&2
    exit 1
  fi
  # RocksDB 参考引擎是可选目标：依赖系统 librocksdb，找不到时 CMake 会跳过，
  # 矩阵退化成两列而不是整轮失败。
  if ! run_tool "$CMAKE" --build "$FFI_BUILD_DIR" --target fkv_bench_ffi rocksdb_bench_ffi -j "$JOBS" >&2; then
    echo "build_cpp_bench: 编译 FFI 失败。完整日志见 $FFI_BUILD_DIR" >&2
    exit 1
  fi
fi

# 5) 输出路径（stdout 只有这一行）。
BIN=""
while IFS= read -r candidate; do
  if [ -x "$candidate" ]; then
    BIN=$candidate
    break
  fi
done < <(find "$BUILD_DIR" -type f -name simple_bench 2>/dev/null)
[ -n "$BIN" ] ||
  die "编译完成但 $BUILD_DIR 下找不到可执行的 simple_bench"
printf '%s\n' "$BIN"
