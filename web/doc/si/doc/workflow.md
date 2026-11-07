# සත්යාපන ක්රියාවලිය

## මලකඩ අනුකූලතා පරීක්ෂණය

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` සාපේක්ෂ මාර්ග හරහා `../fkv` හි ආකෘති කූඩය මත රඳා පවතී, ආදාන සංග්‍රහය විකේතනය කර කරයි
බයිට් බයිට් රන් සම්මත ප්‍රකාශයන්.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle විසින් TU ආකෘතියේ ස්ථරය පමණක් සම්පාදනය කරන අතර io_uring / S3 / සම්පූර්ණ පුස්තකාල ඉදිකිරීම අවශ්‍ය නොවේ.

## corpus

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

ප්ලාවිතය සඳහා පරීක්ෂා කිරීමට CI විසින් `fixtures/cpp` නැවත උත්පාදනය කිරීමෙන් පසු `git diff --exit-code fixtures/cpp` ක්‍රියාත්මක කරන්න.

## මූලික රේඛාව

`COMPAT_BASELINE` eloqstore කැපවීම සහ අනුකූලතා මට්ටම් අගුළු දමයි. උත්ශ්‍රේණිගත කළ මූලික රේඛාව නැවත ස්කෑන් කර නැවත සත්‍යාපනය කළ යුතුය.