# 验证流程

## Rust 兼容测试

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat` 通过相对路径依赖 `../fkv` 的 format crate，解码入库语料并做
逐字节金标断言。

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

oracle 只编译格式层 TU，不需要 io_uring / S3 / 整库构建。

## 语料

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI 重新生成 `fixtures/cpp` 后执行 `git diff --exit-code fixtures/cpp` 检查漂移。

## 基线

`COMPAT_BASELINE` 锁定 eloqstore commit 与兼容等级。升级基线必须重扫、重验。
