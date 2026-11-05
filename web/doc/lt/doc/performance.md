# Našumas

Atsisiųskite `perf-latest` iš `fkv_bench` iš fkv saugyklos ir paleiskite tą patį darbo krūvį fiksuotame kompiuteryje.
Ir palyginkite pralaidumą, delsos kvantilius ir didžiausią RSS su C++ bazine linija.

Parametrų šaltinis atitinka fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ bazinę liniją vykdo oracle aplinka `simple_bench`, tada išvedama pagal tą pačią schemą ir paskelbta
`perf-baseline` išleisti išteklius. Palyginimo rezultatai įrašomi atgal į sandėlio dokumentą, o svetainė tiesiogiai pateikia žymėjimą.
JSON duomenų sluoksnis nepalaikomas.