#!/usr/bin/env bash
# 取 eloqstore 源码到 `cpp/eloqstore`，pin 在 `COMPAT_BASELINE` 的 commit。
#
# 用途：本地构建 C++ oracle（`cpp/oracle/build.sh`）时需要一个 eloqstore 源码树；
# CI（`.github/workflows/oracle.yml`）在 workflow 里直接 checkout 到 workspace。
# 已有同 commit 的本地树时是幂等的。
#
# 用法：
#   scripts/fetch_eloqstore.sh [目标目录，默认 <repo>/cpp/eloqstore]
#   ELOQSTORE_REPO=<git url> scripts/fetch_eloqstore.sh
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
DEST=${1:-$ROOT/cpp/eloqstore}
COMMIT=$(awk 'NR == 1 {print $3}' "$ROOT/COMPAT_BASELINE")
REPO=${ELOQSTORE_REPO:-https://github.com/eloqdata/eloqstore}

if [ -z "$COMMIT" ]; then
  echo "COMPAT_BASELINE 里读不到 commit" >&2
  exit 1
fi

if [ -f "$DEST/include/kv_options.h" ]; then
  have=$(git -C "$DEST" rev-parse HEAD 2>/dev/null || echo unknown)
  if [ "$have" = "$COMMIT" ]; then
    echo "eloqstore 已就绪：$DEST @ $COMMIT"
    exit 0
  fi
  echo "eloqstore 已存在但 commit 不符（有 $have，要 $COMMIT）：重建 $DEST" >&2
  rm -rf "$DEST"
fi

mkdir -p "$(dirname "$DEST")"
# blob-less clone 省带宽；submodules 只有头文件级依赖（inih / span 等在仓内）。
git clone --filter=blob:none "$REPO" "$DEST"
git -C "$DEST" checkout --detach "$COMMIT"
git -C "$DEST" submodule update --init --recursive --depth 1 || true
echo "eloqstore 就绪：$DEST @ $COMMIT"
