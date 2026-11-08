# eloqstore2fkv

Flutningur og staðfestingarvöruhús frá eloqstore til fkv.

Aðalgeymslan fkv er áfram hrein Ryð og er byggð sjálfstætt; þessi geymsla heldur C++ tilvísunarútfærslu véfréttinni, tvíátta bæti corpus,
Flytja höfuðbækur, sögulegar endurskoðunarskrár og staðfesta CI.

## efni

- C++ véfrétt: tengja eloqstore sniðlag og búa til síðu/index/overflow/manifest/
  þjappa saman kroppi.
- Tvíátta corpus: C++ skrifa → Ryð lesið, Ryð skrifa → C++ lesið.
- `fkv_compat`: Ryðhliðarsamhæfispróf, sem byggir á sniði rimlakassi fkv vöruhússins.
- `fkv_ledger`: Skannaðu eloqstore, skráðu flutningsbók, búðu til og staðfestu samstæðu.
- Frammistöðusamanburður: Berðu saman fkv og C++ grunnlínur undir sama vinnuálagi.

## Inngangur

- [Staðfestingarferli](workflow.md)
- [C++ oracle](oracle.md)
- [Flytja höfuðbók](ledger.md)
- [frammistöðu](performance.md)
- [Farið yfir skrár](issues.md)