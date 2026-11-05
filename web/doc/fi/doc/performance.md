# Suorituskyky

Lataa `perf-latest` / `fkv_bench` fkv-varastosta ja suorita sama työkuorma kiinteällä koneella.
Ja vertaa suorituskykyä, latenssikvantiileja ja huippu-RSS:ää C++-perustasoon.

Parametrilähde on yhdenmukainen fkv:n kanssa:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Oracle-ympäristö `simple_bench` suorittaa C++-perusviivan ja tulostetaan sitten saman skeeman mukaisesti ja julkaistaan
`perf-baseline` vapauta sisältö. Vertailutulokset kirjoitetaan takaisin varastoasiakirjaan, ja sivusto tekee suoraan Markdownin.
JSON-tietokerrosta ei ylläpidetä.