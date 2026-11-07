# eloqstore2fkv

Migration at compatibility verification warehouse mula sa eloqstore hanggang fkv.

Ang pangunahing fkv repository ay nananatiling purong Rust at itinayo nang nakapag-iisa; pinapanatili ng repositoryong ito ang orakulo ng pagpapatupad ng sanggunian ng C++, bidirectional byte corpus,
I-migrate ang mga ledger, makasaysayang talaan ng pag-audit at i-verify ang CI.

## nilalaman

- C++ oracle: i-link ang layer ng format ng eloqstore at bumuo ng page/index/overflow/manifest/
  compress corpus.
- Bidirectional corpus: C++ write → Rust read, Rust write → C++ read.
- `fkv_compat`: Pagsusuri sa pagiging tugma sa gilid ng kalawang, umaasa sa format na crate ng fkv warehouse.
- `fkv_ledger`: I-scan ang eloqstore, itala ang migration ledger, bumuo at i-verify ang corpus.
- Paghahambing ng pagganap: Paghambingin ang mga baseline ng fkv at C++ sa ilalim ng parehong workload.

## Pagpasok

- [Proseso ng pagpapatunay](workflow.md)
- [C++ oracle](oracle.md)
- [Ilipat ang ledger](ledger.md)
- [pagganap](performance.md)
- [Suriin ang mga talaan](issues.md)