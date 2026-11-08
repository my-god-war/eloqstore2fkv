# eloqstore2fkv

Ghala la uhamaji na utangamano kutoka eloqstore hadi fkv.

Hifadhi kuu ya fkv inabakia Kutu safi na imejengwa kwa kujitegemea; hazina hii inahifadhi oracle ya utekelezaji wa marejeleo ya C++, ushirika wa baiti mbili,
Hamisha leja, rekodi za ukaguzi wa kihistoria na uthibitishe CI.

## maudhui

- C++ oracle: kiungo safu ya umbizo la eloqstore na toa ukurasa/index/furika/dhihirisha/
  compress corpus.
- Ushirika wa pande mbili: C++ andika → Kutu kusoma, Kutu kuandika → C++ soma.
- `fkv_compat`: Jaribio la uoanifu la upande wa kutu, linalotegemea kreti ya umbizo la ghala la fkv.
- `fkv_ledger`: Changanua duka la eloqstore, rekodi ya leja ya uhamiaji, tengeneza na uthibitishe shirika.
- Ulinganisho wa utendaji: Linganisha misingi ya fkv na C++ chini ya mzigo sawa wa kazi.

## Ingång

- [Mchakato wa uthibitishaji](workflow.md)
- [C++ oracle](oracle.md)
- [Hamisha leja](ledger.md)
- [utendaji](performance.md)
- [Kagua rekodi](issues.md)