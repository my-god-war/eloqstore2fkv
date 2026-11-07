---
date: 2026-11-06
summary: Confronto delle prestazioni della home page, espansione dell'indicatore di valutazione, pagine GitHub e blog multilingue online
---

# 06-11-2026: Tabella delle prestazioni della home page, espansione degli indicatori e Pagine

L'aggiornamento di oggi si concentra sulle revisioni delle prestazioni e sul lancio del sito.

## Confronto delle prestazioni della home page

- La home page viene modificata in priorità prestazionale e la tabella comparativa C++/Rust viene visualizzata direttamente nella prima schermata.
- Vengono aggiunti tre nuovi set di indicatori: larghezza di banda logica, ritardo di coda P99.9 e amplificazione spaziale.
- Aggiunti dettagli sull'esecuzione di Rust: scansione, scansione completa, amplificazione di scrittura, numero di chiamate di sistema di scrittura e sincronizzazione dei dati per batch.
- Contrassegna esplicitamente `write-read` come calibro misto e collega alla documentazione sulle prestazioni.

Riferimento attuale per la stessa macchina:

| indice | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Picco RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilizzo dello spazio di archiviazione | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicatori di valutazione

Facendo riferimento all'output comune di RocksDB, LevelDB, YCSB, WiredTiger e altri benchmark, ho aggiunto:

- Larghezza di banda logica di lettura e scrittura MiB/s;
- P99.9 ritardo coda;
- Amplificazione dello spazio, amplificazione della scrittura;
- Numero di chiamate di sistema in scrittura e sincronizzazione dei dati in ciascun batch;
- Velocità effettiva di scansione/scansione completa;
- CPU, errori di pagina, cambi di contesto e IO di processo continuano a essere raccolti dal sidecar delle risorse.

In pensione contemporaneamente`gxhash_4096B_gib_s`. Si tratta di un microbenchmark di hash a blocco singolo da 4 KiB che non si trova sul percorso del motore KV e non ha una controparte C++.

## Correttezza e correzioni tecniche

- Correggi lo spazio chiave di `--rng cpp` lettura/scansione: il rilevamento della lettura non viene più moltiplicato per `key_interval` per evitare che il 75% delle letture cada in chiavi non scritte.
- Risolti i tentativi in ​​standby causati da `openrsync` macOS valutazione errata dei manifest con lo stesso contenuto.
- Risolto un problema relativo al gate hardware per cui i dischi virtuali QEMU venivano erroneamente valutati come SSD locali.

## Siti ed editoria

- Pagine GitHub abilitate: <https://my-god-war.github.io/eloqstore2fkv/>
- Il routing del sito supporta `/eloqstore2fkv/` sottopercorsi.
- Il documento viene invece letto da `raw.githubusercontent.com` e il problema 404 causato dalla mancanza di `web/` nel percorso jsDelivr è stato corretto.
- La navigazione del blog e la `bun run tran` traduzione multilingue sono integrate; questo articolo è tradotto automaticamente dalla fonte cinese in altre lingue.

I successivi log delle modifiche continueranno a documentare la compatibilità, le prestazioni e le modifiche alla toolchain.