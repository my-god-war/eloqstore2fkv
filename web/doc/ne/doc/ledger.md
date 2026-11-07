# लेजर माइग्रेट गर्नुहोस्

`tool/fkv_ledger` eloqstore स्क्यान गर्न ट्री-सिटर प्रयोग गर्नुहोस्, नक्सा गर्नुहोस् र C++ प्रतीकहरू र fkv प्रमाणित गर्नुहोस्।
प्रमाण motedb मा लेखिएको छ।

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

लेजर डाटाबेस `.data/motedb.mote` मा छ र डाटाबेसमा भण्डार गरिएको छैन।

`verified` पास प्रमाण हुनुपर्छ; `doc/progress.md` `report` द्वारा उत्पन्न गरिएको हो।