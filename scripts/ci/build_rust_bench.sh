#!/usr/bin/env bash
# 从兄弟仓 fkv 构建 `fkv_bench`（release）。
#
# 用法：
#   scripts/ci/build_rust_bench.sh
#   scripts/ci/build_rust_bench.sh --target aarch64-unknown-linux-gnu
#   FKV_REPO_PATH=/tmp/fkv scripts/ci/build_rust_bench.sh
#
# 输出：**最后一行 stdout 是 fkv_bench 的绝对路径**，进度走 stderr，
#       便于 `bin=$(scripts/ci/build_rust_bench.sh)` 取路径。
#
# 环境变量：
#   FKV_REPO_PATH           fkv 源码目录，默认 <repo>/../fkv
#   FKV_BENCH_TARGET        默认 target（--target 优先）
#   FKV_BENCH_RUSTFLAGS     显式覆盖 RUSTFLAGS（默认按架构自动加 target-feature）
#   CARGO_TARGET_DIR        透传 cargo
#
# n1（aarch64）且 rustc 不在 PATH 时，自动：
#   PATH=/opt/rust/toolchains/nightly-aarch64-unknown-linux-gnu/bin:/opt/rust/bin:$PATH
#   RUSTUP_HOME=/opt/rust（若存在）
#   RUSTFLAGS="-C target-feature=+aes,+neon"
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
FKV_REPO_PATH=${FKV_REPO_PATH:-$ROOT/../fkv}
TARGET=${FKV_BENCH_TARGET:-}

log() { echo "==> $*" >&2; }
die() {
  echo "build_rust_bench: $*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --target)
      [ "$#" -ge 2 ] || die "--target 需要参数"
      TARGET=$2
      shift 2
      ;;
    --target=*)
      TARGET=${1#--target=}
      shift
      ;;
    -h | --help)
      sed -n '2,21p' "$0"
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

ARCH=$(uname -m)
N1_TOOLCHAIN=/opt/rust/toolchains/nightly-aarch64-unknown-linux-gnu/bin
if { [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; } &&
  ! command -v rustc >/dev/null 2>&1; then
  log "rustc 不在 PATH；按 n1 约定注入 $N1_TOOLCHAIN"
  export PATH="$N1_TOOLCHAIN:/opt/rust/bin:$PATH"
  if [ -d /opt/rust ]; then
    export RUSTUP_HOME=${RUSTUP_HOME:-/opt/rust}
  fi
fi

if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
  if [ -n "${FKV_BENCH_RUSTFLAGS:-}" ]; then
    export RUSTFLAGS=$FKV_BENCH_RUSTFLAGS
  else
    # AES/NEON 是 aarch64 基线扩展，显式打开避免 CI 交叉编译器默认值差异。
    # n1 的登录 shell 会预设 RUSTFLAGS（-Z threads / mold linker），所以这里
    # 是**追加**而不是覆盖；已经带了 +aes / +neon 时不重复。
    need_features=0
    case "${RUSTFLAGS:-}" in *+aes*) ;; *) need_features=1 ;; esac
    case "${RUSTFLAGS:-}" in *+neon*) ;; *) need_features=1 ;; esac
    if [ "$need_features" = "1" ]; then
      export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-feature=+aes,+neon"
      log "RUSTFLAGS=$RUSTFLAGS"
    fi
  fi
fi

command -v cargo >/dev/null 2>&1 ||
  die "找不到 cargo。n1: 检查 $N1_TOOLCHAIN 是否存在；c1: 把 \$CARGO_HOME/bin 加进 PATH"
command -v rustc >/dev/null 2>&1 ||
  die "找不到 rustc。n1: 检查 $N1_TOOLCHAIN 是否存在；或安装 rustup nightly"

# 共享 target dir（c1 的 /opt/rust/config.toml 指向 /tmp/rust）会被多个
# checkout 复用；rsync 保留的旧 mtime 会让 cargo 误判源码未变，出现
# "trait 方法不存在" 之类的陈旧 rlib。默认放到源树外的独立缓存目录。
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/fkv-perf/rust-target"
fi

[ -f "$FKV_REPO_PATH/Cargo.toml" ] ||
  die "FKV_REPO_PATH=$FKV_REPO_PATH 不是 fkv 源码树（缺 Cargo.toml）；设置 FKV_REPO_PATH 或把 fkv 放到 $ROOT/../fkv"

# rust-toolchain.toml 是 nightly；rustup 可能尝试联网同步。若本地已装 nighty
# toolchain，直接 pin 住，避免离线主机卡住或把 toolchain 更新坏。
if [ -z "${RUSTUP_TOOLCHAIN:-}" ] && [ -d "${RUSTUP_HOME:-$HOME/.rustup}/toolchains" ]; then
  for tc in $(ls "${RUSTUP_HOME:-$HOME/.rustup}/toolchains" 2>/dev/null); do
    case "$tc" in
      nightly-*) RUSTUP_TOOLCHAIN=$tc ;;
    esac
  done
  if [ -n "${RUSTUP_TOOLCHAIN:-}" ]; then
    export RUSTUP_TOOLCHAIN
    log "RUSTUP_TOOLCHAIN=$RUSTUP_TOOLCHAIN (offline pin)"
  fi
fi

build_args=(build --release -p fkv_bench)
if [ -n "$TARGET" ]; then
  build_args+=(--target "$TARGET")
  log "target: $TARGET"
fi
log "cargo ${build_args[*]} (in $FKV_REPO_PATH)"
if ! (cd "$FKV_REPO_PATH" && cargo "${build_args[@]}"); then
  echo "build_rust_bench: cargo build 失败。检查：" >&2
  echo "  - FKV_REPO_PATH 指向 fkv 源码（当前 $FKV_REPO_PATH）" >&2
  echo "  - nightly toolchain 与 target std 已安装" >&2
  echo "  - 交叉 target 需要对应 linker（aarch64 用 aarch64-linux-gnu-gcc）" >&2
  exit 1
fi

# target 目录不一定在 <repo>/target：CARGO_TARGET_DIR 或 $CARGO_HOME/config.toml
# 的 build.target-dir 都可能改写（c1 的 /opt/rust/config.toml 就指向 /tmp/rust）。
# `cargo metadata --no-deps` 是唯一可靠且不需要编译的解析方式。
TARGET_DIR=${CARGO_TARGET_DIR:-}
if [ -z "$TARGET_DIR" ]; then
  meta=$(cd "$FKV_REPO_PATH" && cargo metadata --format-version 1 --no-deps 2>/dev/null || true)
  TARGET_DIR=$(printf '%s' "$meta" |
    sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
fi
[ -n "$TARGET_DIR" ] || TARGET_DIR=$FKV_REPO_PATH/target
if [ -n "$TARGET" ]; then
  BIN=$TARGET_DIR/$TARGET/release/fkv_bench
else
  BIN=$TARGET_DIR/release/fkv_bench
fi
[ -x "$BIN" ] || die "cargo 成功但找不到 $BIN"
printf '%s\n' "$BIN"
