# ಲೆಡ್ಜರ್ ಅನ್ನು ಸ್ಥಳಾಂತರಿಸಿ

`tool/fkv_ledger` eloqstore ಅನ್ನು ಸ್ಕ್ಯಾನ್ ಮಾಡಲು ಟ್ರೀ-ಸಿಟರ್ ಅನ್ನು ಬಳಸಿ, ನಕ್ಷೆ ಮತ್ತು C++ ಚಿಹ್ನೆಗಳು ಮತ್ತು fkv ಅನ್ನು ಪರಿಶೀಲಿಸಲು
ಸಾಕ್ಷ್ಯವನ್ನು motedb ಗೆ ಬರೆಯಲಾಗಿದೆ.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ಲೆಡ್ಜರ್ ಡೇಟಾಬೇಸ್ `.data/motedb.mote` ನಲ್ಲಿದೆ ಮತ್ತು ಡೇಟಾಬೇಸ್‌ನಲ್ಲಿ ಸಂಗ್ರಹಿಸಲಾಗಿಲ್ಲ.

`verified` ಪಾಸ್ ಪುರಾವೆಗಳನ್ನು ಹೊಂದಿರಬೇಕು; `doc/progress.md` ಅನ್ನು `report` ನಿಂದ ರಚಿಸಲಾಗಿದೆ.