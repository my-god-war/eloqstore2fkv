# Afköst

Sæktu `perf-latest` af `fkv_bench` úr fkv geymslunni og keyrðu sama vinnuálag á fastri vél.
Og berðu saman afköst, leynd magn og hámarks RSS við C++ grunnlínu.

Færibreytugjafinn er í samræmi við fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ grunnlínan er keyrð af véfréttaumhverfinu `simple_bench` og síðan útgefin samkvæmt sama skema og birt á
`perf-baseline` losunareign. Samanburðarniðurstöðurnar eru skrifaðar aftur í vöruhúsaskjalið og vefsvæðið gefur Markdown beint.
Ekkert JSON gagnalag er viðhaldið.