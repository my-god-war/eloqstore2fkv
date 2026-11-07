# Διαδικασία επαλήθευσης

## Δοκιμή συμβατότητας σκουριάς

```bash
cargo nextest run -p fkv_compat
```

Το `fkv_compat` βασίζεται στο πλαίσιο μορφής του `../fkv` μέσω σχετικών μονοπατιών, αποκωδικοποιεί το σώμα εισόδου και κάνει
Ισχυρισμοί χρυσού προτύπου byte-byte.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Η Oracle μεταγλωττίζει μόνο το επίπεδο μορφής TU και δεν απαιτεί io_uring / S3 / ολόκληρη την κατασκευή βιβλιοθήκης.

## σώμα

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

Εκτελέστε το `git diff --exit-code fixtures/cpp` μετά την αναγέννηση του CI `fixtures/cpp` για να ελέγξετε για drift.

## γραμμή βάσης

`COMPAT_BASELINE` Κλειδώνει τα επίπεδα δέσμευσης και συμβατότητας του eloqstore. Η αναβαθμισμένη γραμμή βάσης πρέπει να σαρωθεί και να επαληθευτεί εκ νέου.