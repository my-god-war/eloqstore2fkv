# 원장 마이그레이션

`tool/fkv_ledger` tree-sitter를 사용하여 eloqstore 스캔, C++ 기호 및 fkv 매핑 및 확인
증거는 motedb에 기록됩니다.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

원장 데이터베이스는 `.data/motedb.mote`에 있으며 데이터베이스에 저장되지 않습니다.

`verified` 합격 증거가 있어야 합니다. `doc/progress.md`는 `report`에 의해 생성됩니다.