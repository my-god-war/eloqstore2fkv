# ដំណើរការផ្ទៀងផ្ទាត់

## ការធ្វើតេស្តភាពឆបគ្នានៃច្រែះ

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` ពឹងផ្អែក​លើ​ទម្រង់​នៃ `../fkv` តាមរយៈ​ផ្លូវ​ដែល​ទាក់ទង ឌិកូដ​អង្គធាតុ​បញ្ចូល និង​ធ្វើ
ការអះអាងស្តង់ដារមាសដោយបៃបៃបៃ។

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle ចងក្រងតែ format layer TU ប៉ុណ្ណោះ ហើយមិនត្រូវការ io_uring / S3 / បណ្ណាល័យទាំងមូលទេ។

## សាកសព

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ប្រតិបត្តិ `git diff --exit-code fixtures/cpp` បន្ទាប់ពី CI បង្កើតឡើងវិញ `fixtures/cpp` ដើម្បីពិនិត្យមើលការរសាត់។

## បន្ទាត់មូលដ្ឋាន

`COMPAT_BASELINE`ចាក់សោ eloqstore ប្រព្រឹត្ត និងកម្រិតភាពត្រូវគ្នា។ បន្ទាត់មូលដ្ឋានដែលបានធ្វើឱ្យប្រសើរឡើងត្រូវតែស្កេនឡើងវិញ និងផ្ទៀងផ្ទាត់ឡើងវិញ។