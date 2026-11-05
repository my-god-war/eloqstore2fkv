#!/usr/bin/env bash
# 可重复生成 fixture 语料（C++ oracle 写 + Rust 反向写）并跑双向校验。
#
# 用法（eloqstore 源码树已在本地时）：
#   ELOQSTORE_SRC=/path/to/eloqstore bash scripts/gen_fixtures.sh
# 缺省用仓库根的 cpp/eloqstore（scripts/fetch_eloqstore.sh 可拉取）。
#
# 产物（入库，版本见各 meta.txt 的 baseline 字段 + 仓库 COMPAT_BASELINE）：
#   fixtures/cpp/<kind>/<case>/   C++ oracle 写 → Rust 读
#   fixtures/rust/<kind>/<case>/  Rust builder 写 → C++ oracle 读
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SRC=${ELOQSTORE_SRC:-$ROOT/cpp/eloqstore}
ORACLE=${FKV_ORACLE:-/tmp/fkv_oracle}
cd "$ROOT"

if [ ! -f "$SRC/include/kv_options.h" ]; then
  echo "eloqstore source not found: $SRC（用 ELOQSTORE_SRC 指定，或先跑 scripts/fetch_eloqstore.sh）" >&2
  exit 1
fi

echo "== baseline =="
cat COMPAT_BASELINE

echo "== build oracle =="
ELOQSTORE_SRC="$SRC" bash cpp/oracle/build.sh "$ORACLE"

echo "== C++ 写 fixtures/cpp（矩阵全量）=="
cargo run -q -p fkv_ledger -- fixture --matrix --oracle "$ORACLE" --out fixtures/cpp

echo "== Rust 写 fixtures/rust（同一语料的 Rust 产物）=="
cargo run -q -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust

echo "== 双向校验：Rust 解码断言 + C++ oracle 交叉读 =="
cargo run -q -p fkv_ledger -- verify --fixtures fixtures/cpp --oracle "$ORACLE" --report /tmp/fixture-cpp-report.md
cargo run -q -p fkv_ledger -- verify --fixtures fixtures/rust --oracle "$ORACLE" --report /tmp/fixture-rust-report.md

echo "== 完成 =="
echo "报告：/tmp/fixture-cpp-report.md /tmp/fixture-rust-report.md"
