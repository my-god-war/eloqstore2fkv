---
date: 2026-11-06
summary: Compararea performanței paginii de pornire, extinderea indicatorului de evaluare, Pagini GitHub și bloguri online în mai multe limbi
---

# 2026-11-06: Tabelul performanței paginii de pornire, extinderea indicatorului și Paginile

Actualizarea de astăzi se concentrează pe recenzii de performanță și lansări de site-uri.

## Comparația performanței paginii de pornire

- Pagina de pornire este schimbată la prioritate de performanță, iar tabelul de comparație C++/Rugină este afișat direct pe primul ecran.
- Se adaugă trei seturi noi de indicatori: lățimea de bandă logică, întârzierea cozii P99.9 și amplificarea spațială.
- S-au adăugat detalii de rulare Rust: scanare, scanare completă, amplificare de scriere, număr de apeluri de sistem de scriere și sincronizare a datelor per lot.
- Marcați explicit `write-read` ca calibru mixt și trimiteți la documentația de performanță.

Linia de referință actuală pentru aceeași mașină:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Vârf RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Utilizarea depozitării | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicatori de evaluare

Referindu-ne la ieșirea comună a RocksDB, LevelDB, YCSB, WiredTiger și alte repere, am adăugat:

- Lățimea de bandă de citire și scriere logică MiB/s;
- P99,9 întârziere coadă;
- Amplificare spațială, amplificare scris;
- Numărul de apeluri de sistem de scriere și sincronizarea datelor în fiecare lot;
- Debit de scanare / scanare completă;
- CPU, erorile de pagină, comutarile de context și IO procesele continuă să fie colectate de sidecar-ul de resurse.

Retras în același timp`gxhash_4096B_gib_s`. Este un microbenchmark hash cu un singur bloc de 4 KiB care nu se află pe calea motorului KV și nu are o contrapartidă C++.

## Corectitudine și remedieri de inginerie

- Remediați spațiul de taste al `--rng cpp` citire/scanare: detectarea citirii nu mai este multiplicată cu `key_interval` pentru a evita ca 75% din citiri să cadă în chei nescrise.
- S-au remediat încercările de așteptare cauzate de macOS `openrsync` judecată greșită a manifestelor cu același conținut.
- S-a remediat o problemă de poartă hardware în care discurile virtuale QEMU erau apreciate greșit ca SSD-uri locale.

## Site-uri și Publicare

- Pagini GitHub activate: <https://my-god-war.github.io/eloqstore2fkv/>
- Rutarea site-ului acceptă `/eloqstore2fkv/` căi secundare.
- În schimb, documentul este citit de la `raw.githubusercontent.com`, iar 404 cauzată de lipsa `web/` din calea jsDelivr este remediată.
- Navigarea pe blog și `bun run tran` traducerea în mai multe limbi sunt integrate; acest articol este tradus automat din sursa chineză în alte limbi.

Jurnalele de modificări ulterioare vor continua să documenteze compatibilitatea, performanța și modificările lanțului de instrumente.