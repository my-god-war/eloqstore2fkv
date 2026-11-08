# eloqstore2fkv

Migration and compatibility verification warehouse from eloqstore to fkv.

The main fkv repository remains pure Rust and is built independently; this repository retains the C++ reference implementation oracle, bidirectional byte corpus,
Migrate ledgers, historical audit records and verify CI.

## Content

- C++ oracle: link eloqstore format layer and generate page/index/overflow/manifest/
  compress corpus.
- Bidirectional corpus: C++ write → Rust read, Rust write → C++ read.
- `fkv_compat`: Rust-side compatibility test, relying on the format crate of the fkv warehouse.
- `fkv_ledger`: Scan eloqstore, record migration ledger, generate and verify corpus.
- Performance comparison: Compare fkv and C++ baselines under the same workload.

## Entrance

- [Verification process](workflow.md)
- [C++ oracle](oracle.md)
- [Migrate ledger](ledger.md)
- [Performance](performance.md)
- [Review records](issues.md)