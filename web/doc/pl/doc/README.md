# eloqstore2fkv

Migracja i weryfikacja kompatybilności magazynu z eloqstore do fkv.

Główne repozytorium fkv pozostaje czystym Rustem i jest budowane niezależnie; to repozytorium zachowuje wyrocznię implementacji referencyjnej C++, dwukierunkowy korpus bajtów,
Przeprowadź migrację ksiąg, historycznych zapisów audytu i zweryfikuj CI.

## treść

- C++ Oracle: połącz warstwę formatu eloqstore i wygeneruj stronę/indeks/przepełnienie/manifest/
  skompresować korpus.
- Korpus dwukierunkowy: zapis C++ → odczyt Rust, zapis Rust → odczyt C++.
- `fkv_compat`: Test kompatybilności strony rdzy, w oparciu o skrzynię formatową magazynu fkv.
- `fkv_ledger`: Skanuj eloqstore, rejestruj księgę migracji, generuj i weryfikuj korpus.
- Porównanie wydajności: porównaj linie bazowe fkv i C++ przy tym samym obciążeniu.

## Wejście

- [Proces weryfikacji](workflow.md)
- [C++ oracle](oracle.md)
- [Przeprowadź migrację księgi](ledger.md)
- [wydajność](performance.md)
- [Przejrzyj zapisy](issues.md)