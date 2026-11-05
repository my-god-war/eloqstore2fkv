# eloqstore2fkv

eloqstore에서 fkv로의 마이그레이션 및 호환성 검증 웨어하우스.

기본 fkv 저장소는 순수한 Rust로 유지되며 독립적으로 구축됩니다. 이 저장소는 C++ 참조 구현 oracle, 양방향 바이트 코퍼스,
원장, 과거 감사 기록을 마이그레이션하고 CI를 확인합니다.

## 콘텐츠

- C++ oracle: eloqstore 형식 레이어를 연결하고 페이지/인덱스/오버플로/매니페스트/를 생성합니다.
  코퍼스를 압축합니다.
- 양방향 코퍼스: C++ 쓰기 → Rust 읽기, Rust 쓰기 → C++ 읽기.
- `fkv_compat`: fkv 웨어하우스의 형식 상자를 사용하는 Rust 측 호환성 테스트입니다.
- `fkv_ledger`: eloqstore를 스캔하고, 마이그레이션 원장을 기록하고, 말뭉치를 생성 및 확인합니다.
- 성능 비교: 동일한 워크로드에서 fkv 및 C++ 기준을 비교합니다.

## 입구

- [검증과정](workflow.md)
- [C++ oracle](oracle.md)
- [원장 마이그레이션](ledger.md)
- [성능](performance.md)
- [기록 검토](issues.md)