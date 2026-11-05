# ලෙජරය සංක්‍රමණය කරන්න

`tool/fkv_ledger` Eloqstore ස්කෑන් කිරීමට, සිතියම්ගත කිරීමට සහ C++ සංකේත සහ fkv සත්‍යාපනය කිරීමට ගස්-සිටර් භාවිතා කරන්න
සාක්ෂි motedb වෙත ලියා ඇත.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ලෙජර් දත්ත සමුදාය `.data/motedb.mote` හි ඇති අතර දත්ත ගබඩාවේ ගබඩා නොවේ.

`verified` සමත් සාක්ෂි තිබිය යුතුය; `doc/progress.md` `report` මගින් ජනනය වේ.