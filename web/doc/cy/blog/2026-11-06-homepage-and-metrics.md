---
date: 2026-11-06
summary: Cymhariaeth perfformiad tudalen gartref, ehangu dangosyddion gwerthuso, GitHub Pages a blogiau aml-iaith ar-lein
---

# 2026-11-06: Tabl perfformiad tudalen gartref, ehangu dangosyddion a Tudalennau

Mae diweddariad heddiw yn canolbwyntio ar adolygiadau perfformiad a lansio safleoedd.

## Cymhariaeth perfformiad tudalen gartref

- Mae'r dudalen hafan yn cael ei newid i flaenoriaeth perfformiad, ac mae'r tabl cymharu C ++/Rust yn cael ei arddangos yn uniongyrchol ar y sgrin gyntaf.
- Ychwanegir tair set newydd o ddangosyddion: lled band rhesymegol, oedi cynffon P99.9, ac ymhelaethu gofodol.
- Ychwanegwyd manylion rhedeg Rust: Sganio, Fullscan, ysgrifennu ymhelaethiad, nifer y galwadau system ysgrifennu a chydamseru data fesul swp.
- Marciwch `write-read` yn eglur fel safon gymysg a dolen i ddogfennaeth perfformiad.

Llinell sylfaen gyfredol yr un peiriant:

| mynegai | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS brig | 175.6 MiB | 145.5 MiB | 0.83x |
| Defnydd storio | 269.0 MiB | 101.4 MiB | 0.38x |

## Dangosyddion gwerthuso

Gan gyfeirio at allbwn cyffredin RocksDB, LevelDB, YCSB, WiredTiger a meincnodau eraill, ychwanegais:

- MiB/s lled band darllen ac ysgrifennu rhesymegol;
- P99.9 oedi cynffon;
- Ymhelaethu ar y gofod, ymhelaethu ar ysgrifennu;
- Nifer y galwadau system ysgrifennu a chydamseru data ym mhob swp;
- Trwybwn Scan / Fullscan;
- Mae CPU, diffygion tudalen, switshis cyd-destun, ac IO proses yn parhau i gael eu casglu gan y car ochr adnoddau.

Wedi ymddeol ar yr un pryd`gxhash_4096B_gib_s`. Mae'n feincnod micro hash bloc sengl 4 KiB nad yw ar lwybr yr injan KV ac nad oes ganddo gymar C ++.

## Cywirdeb a datrysiadau peirianyddol

- Trwsiwch ofod allweddol `--rng cpp` darllen/sgan: nid yw canfod darllen bellach yn cael ei luosi â `key_interval` er mwyn osgoi 75% o ddarlleniadau rhag syrthio i allweddi anysgrifenedig.
- Ailgeisiadau sefydlog sefydlog a achosir gan macOS `openrsync` camfarnu maniffestau gyda'r un cynnwys.
- Wedi trwsio mater giât caledwedd lle cafodd disgiau rhithwir QEMU eu camfarnu fel SSDs lleol.

## Gwefannau a Chyhoeddi

- Tudalennau GitHub wedi'u galluogi: <https://my-god-war.github.io/eloqstore2fkv/>
- Mae llwybro safle yn cefnogi `/eloqstore2fkv/` subpaths.
- Darllenir y ddogfen o `raw.githubusercontent.com` yn lle hynny, ac mae'r 404 a achosir gan `web/` ar goll yn y llwybr jsDelivr wedi'i osod.
- Mae llywio blog a `bun run tran` cyfieithu aml-iaith wedi'u hintegreiddio; mae'r erthygl hon yn cael ei chyfieithu'n awtomatig o'r ffynhonnell Tsieineaidd i ieithoedd eraill.

Bydd logiau newid dilynol yn parhau i ddogfennu newidiadau cydweddoldeb, perfformiad a chadwyn offer.