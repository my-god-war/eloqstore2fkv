# खातेवही स्थलांतरित करा

`tool/fkv_ledger` eloqstore स्कॅन करण्यासाठी, C++ चिन्हे आणि fkv नकाशा आणि सत्यापित करण्यासाठी ट्री-सिटर वापरा
पुरावा motedb ला लिहिलेला आहे.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

खातेवही डेटाबेस `.data/motedb.mote` येथे आहे आणि डेटाबेसमध्ये संग्रहित नाही.

`verified` पास पुरावा असणे आवश्यक आहे; `doc/progress.md` `report` द्वारे व्युत्पन्न केले जाते.