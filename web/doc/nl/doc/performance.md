# Prestaties

Download `perf-latest` van `fkv_bench` uit de fkv-repository en voer dezelfde werklast uit op een vaste machine.
En vergelijk de doorvoer, latentie-kwantielen en piek-RSS met een C++-basislijn.

De parameterbron komt overeen met fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

De C++-basislijn wordt uitgevoerd door de Oracle-omgeving `simple_bench` en vervolgens uitgevoerd volgens hetzelfde schema en gepubliceerd naar
`perf-baseline` item vrijgeven. De vergelijkingsresultaten worden teruggeschreven naar het magazijndocument en de site geeft Markdown direct weer.
Er wordt geen JSON-gegevenslaag onderhouden.

## Huidige resultaten

schema 2 basislijn voor 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4 KiB-pagina,
`data_append_mode=false`, `direct_io=true`. Vaste werklast:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`, `kv_size=128`, `rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| project | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Zet doorvoer | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Krijg doorvoer | 5,526 QPS | 7,293 QPS | 1.32x |
| Zet logische bandbreedte | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Zorg voor logische bandbreedte | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Piek RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Opslaggebruik | 269.0 MiB | 101.4 MiB | 0.38x |
| Opslag / nominale gegevens | 2.20x | 0.83x | 0.38x |

Het schrijven van 1 miljoen inzendingen (10.000 batches) komt overeen met
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, dat wil zeggen dat gegevens één keer per batch worden gesynchroniseerd.
Rust aanvullende statistieken: Scan `141,166 kv/s`, Volledige scan `108,274 kv/s`, Schrijfversterking `2.57x`
(`proc_io_write` 329 MB / 128 MB logische schrijfbewerkingen), `4.86` schrijf systeemaanroepen per batch.
De opslagverhouding wordt berekend op basis van 1M × 128B = 128 MB nominale gegevens; C++ activeert deze keer niet GC/compact,
Het bevat dus meer pagina's geschiedenis. Het is niet hetzelfde kalibernummer als "versterking van de overlevingsdataruimte".
De P99.9-staartlatentie van Rust is nog steeds slechter dan die van C++ (zet 223,3 ms versus 86,0 ms, krijg 355,5 ms versus
48,2 ms), fluctueert de enkele basislijn sterk. Zie het interne prestatierapport voor details.

**Opmerking**: C++'s `write-read` betekent dat de schrijfthread en de leesthread tegelijkertijd worden uitgevoerd, en Rust momenteel
"Schrijven voltooien, opnieuw openen en opnieuw lezen", er is geen schrijfwedstrijd in de leesfase. Daarom kan de vermenigvuldiger van `write-read` niet worden behandeld als
Puur lezen / puur schrijven conclusie; strikte vergelijking zou `scripts/perf_local.sh --symmetric` moeten zien
Er worden twee paar resultaten uitgevoerd: alleen-schrijven en alleen-lezen. Enkele gedeelde runnernummers worden alleen gebruikt voor regressie
Trends en releases worden beoordeeld op basis van de mediaan van `--runs`.