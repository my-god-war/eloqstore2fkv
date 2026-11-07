# Flytja höfuðbók

`tool/fkv_ledger` Notaðu tree-sitter til að skanna eloqstore, kortleggja og sannreyna C++ tákn og fkv
Sönnunargögn eru rituð til motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Fjárhagsgagnagrunnurinn er á `.data/motedb.mote` og er ekki geymdur í gagnagrunninum.

`verified` verður að hafa sönnunargögn; `doc/progress.md` er búið til af `report`.