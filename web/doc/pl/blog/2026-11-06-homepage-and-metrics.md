---
date: 2026-11-06
summary: Porównanie wydajności strony głównej, rozszerzenie wskaźnika oceny, strony GitHub i wielojęzyczne blogi online
---

# 2026-11-06: Tabela wydajności strony głównej, rozwinięcie wskaźników i Strony

Dzisiejsza aktualizacja koncentruje się na przeglądach wydajności i uruchamianiu witryn.

## Porównanie wydajności strony głównej

- Strona główna zostanie zmieniona na priorytet wydajności, a tabela porównawcza C++/Rust zostanie wyświetlona bezpośrednio na pierwszym ekranie.
- Dodano trzy nowe zestawy wskaźników: szerokość pasma logicznego, opóźnienie końcowe P99.9 i wzmocnienie przestrzenne.
- Dodano szczegóły działania Rusta: skanowanie, pełne skanowanie, wzmocnienie zapisu, liczba wywołań systemowych zapisu i synchronizacja danych na partię.
- Wyraźnie oznacz `write-read` jako kaliber mieszany i podaj link do dokumentacji wydajności.

Bieżąca linia bazowa na tej samej maszynie:

| indeks | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Szczyt RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Wykorzystanie pamięci | 269.0 MiB | 101.4 MiB | 0.38x |

## Wskaźniki oceny

Odnosząc się do wspólnych wyników RocksDB, LevelDB, YCSB, WiredTiger i innych benchmarków, dodałem:

- Logiczna przepustowość odczytu i zapisu MiB/s;
- P99.9 opóźnienie ogona;
- Wzmocnienie przestrzeni, wzmocnienie zapisu;
- Liczba zapisów wywołań systemowych i synchronizacji danych w każdej partii;
- Przepustowość skanowania / pełnego skanowania;
- Procesor, błędy stron, przełączenia kontekstu i operacje we/wy procesu są w dalszym ciągu gromadzone przez wózek pomocniczy zasobów.

Jednocześnie przeszedł na emeryturę`gxhash_4096B_gib_s`. Jest to mikrobenchmark mieszający o rozmiarze 4 KiB, który nie znajduje się na ścieżce silnika KV i nie ma odpowiednika w C++.

## Poprawność i poprawki inżynieryjne

- Napraw odstęp kluczy w `--rng cpp` odczytu/skanowaniu: wykrywanie odczytu nie jest już mnożone przez `key_interval`, aby uniknąć wpadania 75% odczytów na niezapisane klucze.
- Naprawiono ponowne próby w trybie gotowości spowodowane `openrsync` błędną oceną manifestów zawierających tę samą treść w systemie macOS.
- Naprawiono problem z bramą sprzętową, w wyniku którego dyski wirtualne QEMU były błędnie oceniane jako lokalne dyski SSD.

## Witryny i publikacje

- Włączono strony GitHub: <https://my-god-war.github.io/eloqstore2fkv/>
- Routing lokacji obsługuje `/eloqstore2fkv/` podścieżek.
- Zamiast tego dokument jest odczytywany z `raw.githubusercontent.com` i naprawiany jest błąd 404 spowodowany brakiem `web/` w ścieżce jsDelivr.
- Nawigacja po blogu i `bun run tran` tłumaczenie na wiele języków są zintegrowane; ten artykuł jest automatycznie tłumaczony ze źródła chińskiego na inne języki.

Kolejne dzienniki zmian będą nadal dokumentować zmiany w zakresie kompatybilności, wydajności i łańcucha narzędzi.