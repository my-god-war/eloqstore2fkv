---
date: 2026-11-06
summary: Ulinganisho wa utendaji wa ukurasa wa nyumbani, upanuzi wa kiashirio cha tathmini, Kurasa za GitHub na blogu za lugha nyingi mtandaoni.
---

# 2026-11-06: Jedwali la utendaji wa ukurasa wa nyumbani, upanuzi wa kiashirio na Kurasa

Sasisho la leo linaangazia ukaguzi wa utendaji na uzinduzi wa tovuti.

## Ulinganisho wa utendaji wa ukurasa wa nyumbani

- Ukurasa wa nyumbani hubadilishwa kuwa kipaumbele cha utendakazi, na jedwali la kulinganisha la C++/Rust linaonyeshwa moja kwa moja kwenye skrini ya kwanza.
- Seti tatu mpya za viashiria zinaongezwa: kipimo data cha kimantiki, kuchelewa kwa mkia wa P99.9, na ukuzaji wa anga.
- Maelezo ya uendeshaji ya Rust: Changanua, Changanua Kamili, ukuzaji wa maandishi, idadi ya simu za mfumo wa kuandika na maingiliano ya data kwa kila kundi.
- Weka alama `write-read` kwa uwazi kama kiwango kilichochanganywa na kiunganishi cha hati za utendakazi.

Msingi wa sasa wa mashine sawa:

| index | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Kilele cha RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Matumizi ya hifadhi | 269.0 MiB | 101.4 MiB | 0.38x |

## Viashiria vya tathmini

Nikirejelea matokeo ya kawaida ya RocksDB, LevelDB, YCSB, WiredTiger na alama zingine, niliongeza:

- Mantiki kusoma na kuandika Bandwidth MiB/s;
- Ucheleweshaji wa mkia wa P99.9;
- Ukuzaji wa nafasi, ukuzaji wa maandishi;
- Idadi ya simu za mfumo wa kuandika na maingiliano ya data katika kila kundi;
- Scan / Fullscan throughput;
- CPU, hitilafu za ukurasa, swichi za muktadha, na mchakato wa IO unaendelea kukusanywa na kando ya nyenzo.

Alistaafu kwa wakati mmoja`gxhash_4096B_gib_s`. Ni alama ndogo ya alama ya 4K ya block hash ambayo haiko kwenye njia ya injini ya KV na haina mlinganisho wa C++.

## Usahihi na marekebisho ya uhandisi

- Rekebisha nafasi muhimu ya `--rng cpp` read/scan: read discovery hairudishwi tena na `key_interval` ili kuepuka 75% ya usomaji kuangukia kwenye funguo ambazo hazijaandikwa.
- Majaribio yasiyobadilika ya hali ya kusubiri yanayosababishwa na macOS `openrsync` uamuzi usiofaa wa maonyesho yenye maudhui sawa.
- Ilirekebisha suala la lango la maunzi ambapo diski pepe za QEMU zilichukuliwa vibaya kama SSD za ndani.

## Maeneo na Uchapishaji

- Kurasa za GitHub zimewashwa: <https://my-god-war.github.io/eloqstore2fkv/>
- Uelekezaji wa tovuti unaauni `/eloqstore2fkv/` njia ndogo.
- Hati inasomwa kutoka `raw.githubusercontent.com` badala yake, na 404 iliyosababishwa na kukosa `web/` katika njia ya jsDelivr imerekebishwa.
- Urambazaji wa blogu na `bun run tran` tafsiri za lugha nyingi zimeunganishwa; makala hii inatafsiriwa kiotomatiki kutoka kwa chanzo cha Kichina hadi kwa lugha zingine.

Kumbukumbu za mabadiliko zinazofuata zitaendelea kuweka kumbukumbu za utangamano, utendakazi na mabadiliko ya mnyororo wa zana.