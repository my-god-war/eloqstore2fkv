---
date: 2026-11-06
summary: Hasierako orriaren errendimenduaren alderaketa, ebaluazio-adierazleen hedapena, GitHub orriak eta hizkuntza anitzeko blogak sarean
---

# 2026-11-06: Hasierako orriaren errendimendu-taula, adierazleen hedapena eta Orrialdeak

Gaurko eguneraketa errendimenduaren berrikuspenetan eta guneen abiarazteetan zentratzen da.

## Hasierako orriaren errendimenduaren alderaketa

- Hasierako orria errendimendu lehentasunera aldatu da, eta C++/Rust konparazio-taula zuzenean bistaratzen da lehen pantailan.
- Hiru adierazle multzo berri gehitzen dira: banda-zabalera logikoa, P99.9 buztana atzerapena eta anplifikazio espaziala.
- Rust exekutatzeko xehetasunak gehitu dira: eskaneatu, eskaneatu osoa, idazteko anplifikazioa, idazteko sistema dei kopurua eta lote bakoitzeko datuen sinkronizazioa.
- Markatu `write-read` espresuki kalibre misto gisa eta estekatu errendimenduaren dokumentaziora.

Makina bereko egungo oinarri-lerroa:

| aurkibidea | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS gailurra | 175.6 MiB | 145.5 MiB | 0.83x |
| Biltegiratze erabilera | 269.0 MiB | 101.4 MiB | 0.38x |

## Ebaluazio-adierazleak

RocksDB, LevelDB, YCSB, WiredTiger eta beste erreferentzia batzuen irteera komunari erreferentzia eginez, gehitu nuen:

- Irakurri eta idazteko banda-zabalera logikoa MiB/s;
- P99.9 buztana atzerapena;
- Espazioaren anplifikazioa, idazkeraren anplifikazioa;
- Lote bakoitzean idazteko sistema-deien eta datuen sinkronizazioaren kopurua;
- Eskaneatu / eskaneatu osoa;
- CPU, orrialde akatsak, testuinguru-aldaketak eta prozesuko IO-ak biltzen jarraitzen dute baliabideen sidecar-ek.

Aldi berean erretiratua`gxhash_4096B_gib_s`. 4 KiB bloke bakarreko hash mikrobenchmark bat da, KV motorraren bidean ez dagoena eta C++ parekorik ez duena.

## Zuzentasuna eta ingeniaritza konponketak

- Konpondu `--rng cpp` irakurketa/eskaneaketaren gako-espazioa: irakurketa detekzioa jada ez da `key_interval` biderkatzen, irakurketen % 75 idatzi gabeko gakoetan eror ez dadin.
- Eduki bereko manifestuak macOS `openrsync` gaizki baloratu izanak eragindako egonean egonean berriro saiakerak konpondu dira.
- QEMU disko birtualak SSD lokal gisa gaizki baloratu ziren hardware-atearen arazo bat konpondu da.

## Guneak eta Argitalpena

- GitHub orrialdeak gaituta: <https://my-god-war.github.io/eloqstore2fkv/>
- Gunearen bideratzeak `/eloqstore2fkv/` azpibideak onartzen ditu.
- Horren ordez, dokumentua `raw.githubusercontent.com`tik irakurtzen da, eta jsDelivr bidean `web/` falta izanak eragindako 404 konpondu da.
- Blogen nabigazioa eta `bun run tran` hizkuntza anitzeko itzulpena integratuta daude; artikulu hau Txinako iturritik beste hizkuntza batzuetara itzultzen da automatikoki.

Ondorengo aldaketen erregistroek bateragarritasuna, errendimendua eta tresna-katearen aldaketak dokumentatzen jarraituko dute.