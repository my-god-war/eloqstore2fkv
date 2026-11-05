# 성능

fkv 저장소에서 `fkv_bench` 중 `perf-latest`을 다운로드하고 고정된 시스템에서 동일한 워크로드를 실행하세요.
그리고 처리량, 대기 시간 분위수, 최대 RSS를 C++ 기준과 비교합니다.

매개변수 소스는 fkv와 일치합니다.

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ 기준선은 oracle 환경 `simple_bench`에 의해 실행된 다음 동일한 스키마에 따라 출력되고 다음 위치에 게시됩니다.
`perf-baseline` 릴리스 자산. 비교 결과는 웨어하우스 문서에 다시 기록되고 사이트는 마크다운을 직접 렌더링합니다.
JSON 데이터 레이어는 유지되지 않습니다.