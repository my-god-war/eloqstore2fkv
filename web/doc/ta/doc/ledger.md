# லெட்ஜரை நகர்த்தவும்

`tool/fkv_ledger` eloqstore, வரைபடம் மற்றும் C++ குறியீடுகள் மற்றும் fkv ஆகியவற்றை ஸ்கேன் செய்ய ட்ரீ-சிட்டரைப் பயன்படுத்தவும்
ஆதாரம் motedb க்கு எழுதப்பட்டுள்ளது.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

லெட்ஜர் தரவுத்தளம் `.data/motedb.mote` இல் உள்ளது மற்றும் தரவுத்தளத்தில் சேமிக்கப்படவில்லை.

`verified` பாஸ் சான்று இருக்க வேண்டும்; `doc/progress.md` ஆனது `report` ஆல் உருவாக்கப்படுகிறது.