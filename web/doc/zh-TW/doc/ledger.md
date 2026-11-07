# 遷移帳本

`tool/fkv_ledger` 用 tree-sitter 掃描 eloqstore，把 C++ 符號、fkv 映射與驗證
證據寫入 motedb。

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

帳本資料庫在 `.data/motedb.mote`，不入庫。

`verified` 必須有 pass evidence;`doc/progress.md` 由 `report` 產生。