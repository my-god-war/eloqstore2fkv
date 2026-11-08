# 検証プロセス

## 錆びの適合性試験

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` は、相対パスを通じて `../fkv` のフォーマット クレートに依存し、入力コーパスをデコードして、
バイトごとのゴールドスタンダードアサーション。

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle はフォーマット層 TU のみをコンパイルし、io_uring / S3 / ライブラリ全体の構築を必要としません。

## コーパス

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI が再生成された後で `git diff --exit-code fixtures/cpp` を実行して`fixtures/cpp`ドリフトをチェックします。

## ベースライン

`COMPAT_BASELINE` eloqstore のコミットと互換性レベルをロックします。アップグレードされたベースラインは再スキャンして再検証する必要があります。