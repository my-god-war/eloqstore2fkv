---
date: 2026-11-06
summary: Samanburður á frammistöðu heimasíðunnar, stækkun matsvísa, GitHub síður og fjöltungumál blogg á netinu
---

# 2026-11-06: Frammistöðutafla heimasíða, stækkun vísis og síður

Uppfærsla dagsins beinist að frammistöðurýni og kynningum á vefsvæðum.

## Samanburður á frammistöðu á heimasíðunni

- Heimasíðan er breytt í frammistöðuforgang og C++/Rust samanburðartaflan birtist beint á fyrsta skjánum.
- Þrjú ný sett af vísum er bætt við: rökræn bandbreidd, P99.9 hala seinkun og staðbundin mögnun.
- Bætt við ryðhlaupaupplýsingum: Skanna, fullskanna, skrifa mögnun, fjölda skrifakerfissímtöla og gagnasamstillingu í hverri lotu.
- Merktu `write-read` beinlínis sem blandað kaliber og tengil á frammistöðuskjöl.

Núverandi grunnlína sama vél:

| vísitölu | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Hámarks RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Geymslunotkun | 269.0 MiB | 101.4 MiB | 0.38x |

## Matsvísar

Með vísan til algengrar framleiðslu RocksDB, LevelDB, YCSB, WiredTiger og annarra viðmiða, bætti ég við:

- Rökrétt lesa og skrifa bandbreidd MiB/s;
- P99.9 hala seinkun;
- Geimmögnun, ritmögnun;
- Fjöldi skrifakerfissímtala og gagnasamstillingu í hverri lotu;
- Skanna / Fullscan afköst;
- Örgjörvi, síðuvillur, samhengisrofar og ferli IO halda áfram að safna af hliðarvagni auðlindarinnar.

Lét af störfum á sama tíma`gxhash_4096B_gib_s`. Það er 4 KiB stakur kjötkássa örviðmið sem er ekki á KV vélarbrautinni og hefur enga C++ hliðstæðu.

## Réttleiki og verkfræðilegar lagfæringar

- Lagaðu lyklarýmið á `--rng cpp` les/skanna: lesskynjun er ekki lengur margfölduð með `key_interval` til að koma í veg fyrir að 75% lestra falli í óskrifaða lykla.
- Lagaðar endurtilraunir í biðstöðu af völdum macOS `openrsync` rangt mat á upplýsingaskrá með sama efni.
- Lagaði vandamál með vélbúnaðarhlið þar sem QEMU sýndardiskar voru ranglega metnir sem staðbundnir SSD diskar.

## Síður og útgáfa

- GitHub síður virkar: <https://my-god-war.github.io/eloqstore2fkv/>
- Vefleiðing styður `/eloqstore2fkv/` undirslóðir.
- Skjalið er lesið úr `raw.githubusercontent.com` í staðinn og 404 sem stafar af því að `web/` vantar í jsDelivr slóðina er lagfærð.
- Bloggleiðsögn og `bun run tran` þýðing á mörgum tungumálum eru samþætt; þessi grein er sjálfkrafa þýdd úr kínverskri uppruna yfir á önnur tungumál.

Síðari breytingaskrár munu halda áfram að skjalfesta eindrægni, frammistöðu og breytingar á verkfærakeðju.