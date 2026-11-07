---
date: 2026-11-06
summary: Primerjava uspešnosti domače strani, razširitev indikatorja vrednotenja, strani GitHub in večjezični spletni dnevniki
---

# 2026-11-06: tabela uspešnosti domače strani, razširitev indikatorja in strani

Današnja posodobitev se osredotoča na ocene uspešnosti in zagone spletnih mest.

## Primerjava uspešnosti domače strani

- Domača stran se spremeni v prioriteto zmogljivosti in primerjalna tabela C++/Rust je neposredno prikazana na prvem zaslonu.
- Dodani so trije novi nizi indikatorjev: logična pasovna širina, repna zakasnitev P99.9 in prostorsko ojačanje.
- Dodane podrobnosti o izvajanju Rust: skeniranje, popolno skeniranje, povečanje pisanja, število sistemskih klicev pisanja in sinhronizacija podatkov na paket.
- Izrecno označite `write-read` kot mešani kaliber in se povežite z dokumentacijo o zmogljivosti.

Trenutna osnovna linija za isti stroj:

| kazalo | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Vrhunec RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Poraba prostora za shranjevanje | 269.0 MiB | 101.4 MiB | 0.38x |

## Indikatorji vrednotenja

Sklicujoč se na skupne rezultate RocksDB, LevelDB, YCSB, WiredTiger in drugih meril uspešnosti sem dodal:

- Logična pasovna širina branja in pisanja MiB/s;
- P99.9 zakasnitev repa;
- Povečanje prostora, povečanje pisanja;
- Število zapisovalnih sistemskih klicev in sinhronizacija podatkov v vsakem paketu;
- Prepustnost skeniranja / polnega skeniranja;
- CPE, napake strani, kontekstna stikala in IO procesa še naprej zbira stranska prikolica vira.

Hkrati upokojen`gxhash_4096B_gib_s`. To je 4 KiB mikrobenchmark zgoščevanja z enim blokom, ki ni na poti motorja KV in nima protipostavke C++.

## Pravilnost in inženirski popravki

- Popravite prostor ključa `--rng cpp` branje/skeniranje: zaznavanje branja se ne množi več z `key_interval`, da preprečite, da bi 75 % branj padlo v nezapisane ključe.
- Popravljeni ponovni poskusi v stanju pripravljenosti, ki jih je povzročila napačna presoja manifestov z isto vsebino macOS `openrsync`.
- Odpravljena je težava z vrati strojne opreme, pri kateri so bili virtualni diski QEMU napačno ocenjeni kot lokalni diski SSD.

## Spletna mesta in založništvo

- Omogočene strani GitHub: <https://my-god-war.github.io/eloqstore2fkv/>
- Usmerjanje spletnega mesta podpira `/eloqstore2fkv/` podpoti.
- Dokument se namesto tega prebere iz `raw.githubusercontent.com` in napaka 404, ki jo povzroča manjkajoči `web/` v poti jsDelivr, je popravljena.
- Integrirana sta navigacija po spletnem dnevniku in `bun run tran` prevod v več jezikov; ta članek je samodejno preveden iz kitajskega vira v druge jezike.

Poznejši dnevniki sprememb bodo še naprej dokumentirali spremembe združljivosti, zmogljivosti in verige orodij.