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

## Dabartiniai rezultatai

2 schemos bazinė linija, skirta 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KB puslapis,
`data_append_mode=false`, `direct_io=true`. Fiksuotas darbo krūvis:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projektą | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Įdėkite pralaidumą | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Gaukite pralaidumą | 5,526 QPS | 7,293 QPS | 1.32x |
| Įdėkite loginį pralaidumą | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Gaukite loginį pralaidumą | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Peak RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Saugyklos naudojimas | 269.0 MiB | 101.4 MiB | 0.38x |
| Saugojimo / vardiniai duomenys | 2.20x | 0.83x | 0.38x |

1 milijono įrašų (10 000 partijų) rašymas atitinka
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, tai yra, duomenys sinchronizuojami vieną kartą per paketą.
Papildomos rūdžių metrikos: nuskaitymas `141,166 kv/s`, visiškas nuskaitymas `108,274 kv/s`, rašymo stiprinimas `2.57x`
(`proc_io_write` 329 MB / 128 MB loginių įrašų), `4.86` rašyti sistemos iškvietimus pakete.
Saugojimo koeficientas apskaičiuojamas remiantis 1M × 128B = 128 MB vardiniais duomenimis; C++ šį kartą nesuaktyvina GC/compact,
Taigi jame yra daugiau istorijos puslapių. Tai nėra tas pats kalibro skaičius, kaip „išgyvenimo duomenų erdvės stiprinimas“.
Rust P99.9 uodegos delsa vis dar yra blogesnė nei C++ (įdėkite 223,3 ms prieš 86,0 ms, gaukite 355,5 ms prieš
48,2 ms), viena bazinė linija labai svyruoja. Daugiau informacijos rasite vidinėje veiksmingumo ataskaitoje.

**Pastaba**: C++ `write-read` reiškia, kad rašymo ir skaitymo gijos veikia tuo pačiu metu, o Rust šiuo metu
„Užbaikite rašymą, atidarykite iš naujo ir skaitykite dar kartą“, skaitymo etape nėra rašymo konkurso. Todėl `write-read` daugiklis negali būti traktuojamas kaip
Gryno skaitymo / gryno rašymo išvada; griežtas palyginimas turėtų matyti `scripts/perf_local.sh --symmetric`
Išvedamos dvi rezultatų poros: tik rašoma ir tik skaitoma. Vieno bendrinamo bėgiko numeriai naudojami tik regresijai
Tendencijos, leidimai vertinami naudojant `--runs` medianą.