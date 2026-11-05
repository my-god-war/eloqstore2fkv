# 검증과정

## Rust 호환성 테스트

```bash
cargo nextest run -p fkv_compat
```

`fkv_compat`은 상대 경로를 통해 `../fkv` 형식 상자에 의존하고 입력 자료를 디코딩하며 다음을 수행합니다.
바이트별 표준 어설션.

## C++ oracle

```bash
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
```

Oracle은 형식 레이어 TU만 컴파일하며 io_uring/S3/전체 라이브러리 구성이 필요하지 않습니다.

## 신체

```bash
cargo run -p fkv_ledger -- fixture --matrix \
  --oracle /tmp/fkv_oracle --out fixtures/cpp
cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp \
  --oracle /tmp/fkv_oracle
cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust
```

CI가 `fixtures/cpp`을 재생성한 후 `git diff --exit-code fixtures/cpp`를 실행하여 드리프트를 확인합니다.

## 기준선

`COMPAT_BASELINE` eloqstore 커밋 및 호환성 수준을 잠급니다. 업그레이드된 기준선을 다시 검색하고 다시 확인해야 합니다.