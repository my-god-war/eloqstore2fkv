# მიგრაცია წიგნში

`tool/fkv_ledger` გამოიყენეთ ხე-მჯდომარე eloqstore-ის სკანირებისთვის, რუკაზე და გადაამოწმეთ C++ სიმბოლოები და fkv
მტკიცებულებები იწერება motedb-ზე.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ledger მონაცემთა ბაზა არის `.data/motedb.mote` და არ ინახება მონაცემთა ბაზაში.

`verified` უნდა ჰქონდეს გამშვები მტკიცებულება; `doc/progress.md` გენერირებულია `report`-ის მიერ.