# మైగ్రేట్ లెడ్జర్

`tool/fkv_ledger` eloqstore, మ్యాప్ మరియు C++ చిహ్నాలు మరియు fkvని ధృవీకరించడానికి ట్రీ-సిట్టర్‌ని ఉపయోగించండి
సాక్ష్యం మోటెడ్బికి వ్రాయబడింది.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

లెడ్జర్ డేటాబేస్ `.data/motedb.mote` వద్ద ఉంది మరియు డేటాబేస్‌లో నిల్వ చేయబడదు.

`verified` తప్పనిసరిగా పాస్ సాక్ష్యం కలిగి ఉండాలి; `doc/progress.md` `report` ద్వారా ఉత్పత్తి చేయబడింది.