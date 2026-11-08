# Leistung

Laden Sie `perf-latest` von `fkv_bench` aus dem fkv-Repository herunter und führen Sie denselben Workload auf einem festen Computer aus.
Und vergleichen Sie Durchsatz, Latenzquantile und Spitzen-RSS mit einer C++-Basislinie.

Die Parameterquelle stimmt mit fkv überein:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Die C++-Baseline wird von der Oracle-Umgebung `simple_bench` ausgeführt und dann nach demselben Schema ausgegeben und veröffentlicht
`perf-baseline` Asset freigeben. Die Vergleichsergebnisse werden in das Lagerdokument zurückgeschrieben und die Site rendert Markdown direkt.
Es wird keine JSON-Datenschicht gepflegt.

## Aktuelle Ergebnisse

Schema 2-Basislinie für 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, 4-KiB-Seite,
`data_append_mode=false`, `direct_io=true`. Fester Arbeitsaufwand:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| Projekt | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Durchsatz setzen | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Erhalten Sie Durchsatz | 5,526 QPS | 7,293 QPS | 1.32x |
| Legen Sie die logische Bandbreite fest | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Erhalten Sie logische Bandbreite | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Spitzen-RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Speichernutzung | 269.0 MiB | 101.4 MiB | 0.38x |
| Lager-/Solldaten | 2.20x | 0.83x | 0.38x |

Das Schreiben von 1 Million Einträgen (10.000 Stapel) entspricht
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, d. h. die Daten werden einmal pro Stapel synchronisiert.
Zusätzliche Rust-Metriken: Scan `141,166 kv/s`, Fullscan `108,274 kv/s`, Write Amplification `2.57x`
(`proc_io_write` 329 MB / 128 MB logische Schreibvorgänge), `4.86` Systemaufrufe pro Stapel schreiben.
Das Speicherverhältnis wird basierend auf 1M × 128B = 128 MB Nenndaten berechnet; C++ löst diesmal GC/compact nicht aus,
Es enthält also mehr Seiten der Geschichte. Es ist nicht die gleiche Kalibernummer wie „Survival Data Space Amplification“.
Die P99.9-Tail-Latenz von Rust ist immer noch schlechter als die von C++ (setzen Sie 223,3 ms vs. 86,0 ms, erhalten Sie 355,5 ms vs
48,2 ms) schwankt die einzelne Grundlinie stark. Weitere Informationen finden Sie im internen Leistungsbericht.

**Hinweis**: `write-read` von C++ bedeutet, dass der Schreibthread und der Lesethread gleichzeitig ausgeführt werden und Rust derzeit ausgeführt wird
„Schreiben abschließen, erneut öffnen und erneut lesen“, in der Lesephase gibt es keinen Schreibwettbewerb. Daher kann der Multiplikator von `write-read` nicht als behandelt werden
Reine Lese-/reine Schreib-Schlussfolgerung; Beim strikten Vergleich sollte `scripts/perf_local.sh --symmetric` angezeigt werden
Es werden zwei Ergebnispaare ausgegeben: schreibgeschützt und schreibgeschützt. Einzelne gemeinsam genutzte Läufernummern werden nur für die Regression verwendet
Trends und Veröffentlichungen werden anhand des Medians von `--runs` beurteilt.