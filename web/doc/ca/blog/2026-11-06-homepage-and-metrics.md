---
date: 2026-11-06
summary: Comparació del rendiment de la pàgina d'inici, expansió d'indicadors d'avaluació, pàgines de GitHub i blocs multilingües en línia
---

# 06-11-2026: taula de rendiment de la pàgina d'inici, ampliació d'indicadors i pàgines

L'actualització d'avui se centra en les revisions de rendiment i els llançaments de llocs.

## Comparació del rendiment de la pàgina d'inici

- La pàgina d'inici es canvia a la prioritat de rendiment i la taula de comparació C++/Rust es mostra directament a la primera pantalla.
- S'afegeixen tres nous conjunts d'indicadors: amplada de banda lògica, retard de cua P99.9 i amplificació espacial.
- S'han afegit detalls d'execució de Rust: escaneig, escaneig complet, amplificació d'escriptura, nombre de trucades al sistema d'escriptura i sincronització de dades per lot.
- Marqueu explícitament `write-read` com a calibre mixt i enllaceu a la documentació de rendiment.

Línia de base actual de la mateixa màquina:

| índex | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS màxim | 175.6 MiB | 145.5 MiB | 0.83x |
| Ús d'emmagatzematge | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicadors d'avaluació

En referència a la sortida comuna de RocksDB, LevelDB, YCSB, WiredTiger i altres punts de referència, he afegit:

- Ample de banda de lectura i escriptura lògica MiB/s;
- P99,9 retard de cua;
- Amplificació espacial, amplificació d'escriptura;
- Nombre de trucades de sistema d'escriptura i sincronització de dades a cada lot;
- Rendiment d'escaneig/escaneig complet;
- La CPU, els errors de pàgina, els canvis de context i les E/S de procés continuen recopilant-se pel sidecar de recursos.

Jubilat al mateix temps`gxhash_4096B_gib_s`. Es tracta d'un microbenchmark hash de bloc únic de 4 KiB que no es troba a la ruta del motor KV i no té cap contrapart en C++.

## Correcció i correccions d'enginyeria

- Corregiu l'espai clau de `--rng cpp` lectura/escaneig: la detecció de lectura ja no es multiplica per `key_interval` per evitar que el 75% de les lectures caiguin en claus no escrites.
- S'han solucionat els reintents d'espera causats per la `openrsync` de macOS de judici errònia dels manifests amb el mateix contingut.
- S'ha solucionat un problema de la porta de maquinari en què els discs virtuals QEMU es consideraven malament com a SSD locals.

## Llocs i publicacions

- Pàgines de GitHub habilitades: <https://my-god-war.github.io/eloqstore2fkv/>
- L'encaminament del lloc admet `/eloqstore2fkv/` subcamins.
- El document es llegeix des de `raw.githubusercontent.com`, i es corregeix el 404 causat per la falta de `web/` al camí jsDelivr.
- La navegació per blocs i la `bun run tran` traducció en diversos idiomes estan integrades; aquest article es tradueix automàticament de la font xinesa a altres idiomes.

Els registres de canvis posteriors continuaran documentant els canvis de compatibilitat, rendiment i cadena d'eines.