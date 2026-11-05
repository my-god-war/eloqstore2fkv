# 迁移账本

`tool/fkv_ledger` 用 tree-sitter 扫描 eloqstore，把 C++ 符号、fkv 映射与验证
证据写入 motedb。

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

账本数据库在 `.data/motedb.mote`，不入库。

`verified` 必须有 pass evidence；`doc/progress.md` 由 `report` 生成。
