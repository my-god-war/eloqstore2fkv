# Μετανάστευση καθολικού

`tool/fkv_ledger` Χρησιμοποιήστε το tree-sitter για σάρωση eloqstore, χαρτογράφηση και επαλήθευση συμβόλων C++ και fkv
Τα αποδεικτικά στοιχεία είναι γραμμένα στο motedb.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

Η βάση δεδομένων του καθολικού βρίσκεται στο `.data/motedb.mote` και δεν αποθηκεύεται στη βάση δεδομένων.

Το `verified` πρέπει να έχει αποδεικτικά στοιχεία. Το `doc/progress.md` δημιουργείται από το `report`.