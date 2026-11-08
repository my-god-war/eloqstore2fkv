---
date: 2026-11-06
summary: Mājas lapas veiktspējas salīdzinājums, novērtējuma indikatora paplašināšana, GitHub lapas un vairāku valodu emuāri tiešsaistē
---

# 2026-11-06: Mājas lapas veiktspējas tabula, indikatora izvēršana un lapas

Šodienas atjauninājums ir vērsts uz veiktspējas pārskatiem un vietņu palaišanu.

## Mājas lapas veiktspējas salīdzinājums

- Mājas lapa tiek mainīta uz veiktspējas prioritāti, un C++/Rust salīdzināšanas tabula tiek tieši parādīta pirmajā ekrānā.
- Ir pievienoti trīs jauni indikatoru komplekti: loģiskais joslas platums, P99.9 aizkave un telpiskā pastiprināšana.
- Pievienota Rust darbības informācija: skenēšana, pilna skenēšana, rakstīšanas pastiprināšana, rakstīšanas sistēmas zvanu skaits un datu sinhronizācija vienā partijā.
- Skaidri atzīmējiet `write-read` kā jauktu kalibru un saiti uz veiktspējas dokumentāciju.

Pašreizējā tās pašas mašīnas bāzes līnija:

| rādītājs | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Maksimālais RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Krātuves izmantošana | 269.0 MiB | 101.4 MiB | 0.38x |

## Vērtēšanas rādītāji

Atsaucoties uz RocksDB, LevelDB, YCSB, WiredTiger un citu etalonu kopējo izvadi, es pievienoju:

- Loģiskā lasīšanas un rakstīšanas joslas platums MiB/s;
- P99.9 astes aizkave;
- Telpas pastiprināšana, rakstīšanas pastiprināšana;
- Rakstīšanas sistēmas zvanu un datu sinhronizācijas skaits katrā partijā;
- Scan / Fullscan caurlaidspēja;
- CPU, lapu kļūdas, konteksta slēdžus un procesa IO turpina apkopot resursa blakusvāģis.

Tajā pašā laikā aizgāja pensijā`gxhash_4096B_gib_s`. Tas ir 4 KiB viena bloka hash mikrobenchmark, kas neatrodas KV dzinēja ceļā un kuram nav C++ ekvivalenta.

## Pareizība un inženiertehniskie labojumi

- Labojiet `--rng cpp` lasīšanas/skenēšanas atslēgu vietu: lasīšanas noteikšana vairs netiek reizināta ar `key_interval`, lai izvairītos no 75% nolasījumu nonākšanas nerakstītās atslēgās.
- Izlaboti gaidstāves mēģinājumi, ko izraisīja macOS `openrsync` nepareizs manifestu novērtējums ar tādu pašu saturu.
- Novērsta aparatūras vārtu problēma, kuras dēļ QEMU virtuālie diski tika nepareizi novērtēti kā vietējie SSD.

## Vietnes un publicēšana

- GitHub lapas iespējotas: <https://my-god-war.github.io/eloqstore2fkv/>
- Vietnes maršrutēšana atbalsta `/eloqstore2fkv/` apakšceļus.
- Tā vietā dokuments tiek nolasīts no `raw.githubusercontent.com`, un 404. gadījums, ko izraisīja jsDelivr ceļā trūkstošais `web/`, tiek labots.
- Emuāra navigācija un `bun run tran` vairāku valodu tulkošana ir integrēta; šis raksts tiek automātiski tulkots no ķīniešu avota citās valodās.

Turpmākie izmaiņu žurnāli turpinās dokumentēt saderības, veiktspējas un rīku ķēdes izmaiņas.