# ផ្លាស់ទីសៀវភៅកត់ត្រា

`tool/fkv_ledger`ប្រើអ្នកមើលដើមឈើដើម្បីស្កេន eloqstore ផែនទី និងផ្ទៀងផ្ទាត់និមិត្តសញ្ញា C++ និង fkv
ភស្តុតាងត្រូវបានសរសេរទៅ motedb ។

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

មូលដ្ឋានទិន្នន័យបញ្ជីឈ្មោះគឺនៅ `.data/motedb.mote` ហើយមិនត្រូវបានរក្សាទុកក្នុងមូលដ្ឋានទិន្នន័យទេ។

`verified` ត្រូវមានភស្តុតាងឆ្លងកាត់ `doc/progress.md` ត្រូវបានបង្កើតដោយ `report`។