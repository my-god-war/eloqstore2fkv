# बही-खाता माइग्रेट करें

`tool/fkv_ledger` eloqstore को स्कैन करने, मैप करने और C++ प्रतीकों और fkv को सत्यापित करने के लिए ट्री-सिट्टर का उपयोग करें
साक्ष्य motedb को लिखा गया है।

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

लेज़र डेटाबेस `.data/motedb.mote` पर है और डेटाबेस में संग्रहीत नहीं है।

`verified` पास साक्ष्य होना चाहिए; `doc/progress.md` `report` द्वारा उत्पन्न होता है।