#!/usr/bin/env bash
# 构建 C++ oracle（`fixtures` 差分用的可执行文件）。
#
# 依赖：eloqstore 源码树 + zstd + glog/gflags（Linux 容器里还需要 jsoncpp /
# liburing 头；见 docker/Dockerfile.oracle）。用法：
#   ELOQSTORE_SRC=/src/eloqstore cpp/oracle/build.sh [输出路径]
#
# 只编格式层需要的 TU（page / index / manifest builder 与 parser、编码、压缩、
# comparator），不拉整库，避开真实 IO / S3 / 云服务依赖：oracle 只需要字节格式。
set -euo pipefail

DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SRC=${ELOQSTORE_SRC:-$DIR/../eloqstore}
OUT=${1:-$DIR/fkv_oracle}

if [ ! -f "$SRC/include/kv_options.h" ]; then
  echo "eloqstore source not found: $SRC（用 ELOQSTORE_SRC 指定，或先跑 scripts/fetch_eloqstore.sh）" >&2
  exit 1
fi

WORK=$(mktemp -d "${TMPDIR:-/tmp}/fkv_oracle_build.XXXXXX")
trap 'rm -rf "$WORK"' EXIT

FLAGS=(-std=c++20 -O2 -ffunction-sections -fdata-sections -pthread)
DEFS=(-DGLOG_USE_GLOG_EXPORT)
LIBS=(-lzstd -lglog -lgflags)
# 注意：eloqstore 根目录有 `version` 文件（内容如 `1.1.1`）。若用 `-I "$SRC"`，
# 会把 C++ 标准头 `<version>`（被 <limits>/<string> 等间接包含）解析到该文本文件，
# 直接编译失败。根目录只放兜底的 `-idirafter`（系统头优先），头文件一律走 include/。
INC=(-I "$SRC/include" -I "$SRC/external" -I "$SRC/external/abseil" -idirafter "$SRC")

if [ "$(uname -s)" = Darwin ]; then
  # macOS：jsoncpp 只装到 <prefix>/include/json，而 eloqstore 头里写的是
  # <jsoncpp/json/json.h>；liburing 不存在，但 page.cpp 仅经 shard.h 间接包含它，
  # oracle 不调用任何 io_uring 符号。这里生成最小 include shim，不改 eloqstore 源码。
  BREW_PREFIX=$(brew --prefix 2>/dev/null || echo /opt/homebrew)
  JSONCPP_PREFIX=$(brew --prefix jsoncpp 2>/dev/null || echo "$BREW_PREFIX/opt/jsoncpp")
  mkdir -p "$WORK/shim/jsoncpp"
  ln -sfn "$JSONCPP_PREFIX/include/json" "$WORK/shim/jsoncpp/json"
  cat > "$WORK/shim/liburing.h" <<'EOF'
#pragma once
// 仅供 macOS 本地编译 oracle：oracle 不调用任何 io_uring 符号。
struct io_uring
{
};
struct io_uring_sqe
{
};
struct io_uring_cqe
{
};
EOF
  # 注意顺序：source tree 的 absl 头必须优先于 brew 的同名头。brew abseil
  # 使用 inline namespace（absl::lts_...），与下面用 eloqstore 源码构建的
  # absl 静态库符号不一致；brew include 只用于 glog/gflags/zstd 等系统依赖。
  INC=(-I "$WORK/shim" "${INC[@]}" -I "$BREW_PREFIX/include")
  FLAGS+=(-include unistd.h -include fcntl.h)
  LIBS=(-L"$BREW_PREFIX/lib" "${LIBS[@]}")
  LDFLAGS=(-Wl,-dead_strip)
else
  # Linux：只保留实际可达的格式层函数，避免为了 ManifestBuilder 链接整个存储栈。
  LDFLAGS=(-Wl,--gc-sections)
fi

SRC_LI=(
  "$SRC/src/storage/data_page_builder.cpp"
  "$SRC/src/storage/data_page.cpp"
  "$SRC/src/storage/index_page_builder.cpp"
  "$SRC/src/storage/mem_cached_page.cpp"
  "$SRC/src/storage/page.cpp"
  "$SRC/src/storage/root_meta.cpp"
  "$SRC/src/storage/page_mapper.cpp"
  "$SRC/src/kv_options.cpp"
  "$SRC/src/coding.cpp"
  "$SRC/src/compression.cpp"
  "$SRC/src/comparator.cpp"
  "$SRC/external/inih/cpp/INIReader.cpp"
)

# `ManifestBuilder::Snapshot` 的入参是 `MappingSnapshot`，其 `MappingTbl`
# 内部用 `absl::flat_hash_map`。oracle 直接构造 snapshot 后会拉入
# raw_hash_set / hashtablez_sampler 符号；这里用 eloqstore pin 住的 abseil
# 源码构建静态库（不能用系统 absl：头文件版本必须与库一致）。
ABSL_SRC="$SRC/external/abseil"
ABSL_BUILD="$WORK/absl"
if [ ! -f "$ABSL_SRC/CMakeLists.txt" ]; then
  echo "abseil source not found: $ABSL_SRC（eloqstore submodule 未拉全）" >&2
  exit 1
fi
JOBS=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)
cmake -B "$ABSL_BUILD" -S "$ABSL_SRC" \
  -DCMAKE_BUILD_TYPE=Release \
  -DABSL_BUILD_TESTING=OFF \
  -DABSL_PROPAGATE_CXX_STD=ON \
  -DCMAKE_CXX_STANDARD=20 \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
  -DCMAKE_INSTALL_PREFIX="$ABSL_BUILD/install" \
  -DBUILD_SHARED_LIBS=OFF >/dev/null
cmake --build "$ABSL_BUILD" --target raw_hash_set hashtablez_sampler \
  --parallel "$JOBS" >/dev/null
ABSL_LIBS=()
while IFS= read -r absl_lib; do
  ABSL_LIBS+=("$absl_lib")
done < <(find "$ABSL_BUILD" -name 'libabsl_*.a' | sort)
if [ "${#ABSL_LIBS[@]}" -eq 0 ]; then
  echo "abseil static libs not built under $ABSL_BUILD" >&2
  exit 1
fi
if [ "$(uname -s)" = Darwin ]; then
  LIBS=("${ABSL_LIBS[@]}" "${LIBS[@]}")
else
  # GNU ld 不重扫静态库：absl 各库互相引用，必须放进 group 里解析。
  LIBS=(-Wl,--start-group "${ABSL_LIBS[@]}" -Wl,--end-group "${LIBS[@]}")
fi

# xxhash.c 是 C 源码；按 C 编译，避免 g++ 把它当 C++ 的 deprecated 警告。
cc -O2 -c "$SRC/external/xxhash.c" -o "$WORK/xxhash.o"

g++ "${FLAGS[@]}" "${DEFS[@]}" "${INC[@]}" -o "$OUT" \
  "$DIR/src/main.cpp" "$DIR/src/standalone.cpp" \
  "${SRC_LI[@]}" "$WORK/xxhash.o" \
  "${LDFLAGS[@]}" "${LIBS[@]}"
echo "oracle built: $OUT"
