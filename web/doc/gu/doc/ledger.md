# ખાતાવહી સ્થળાંતર કરો

`tool/fkv_ledger` eloqstore સ્કેન કરવા, નકશા કરવા અને C++ પ્રતીકો અને fkv ચકાસવા માટે ટ્રી-સિટરનો ઉપયોગ કરો
પુરાવા motedb ને લખવામાં આવ્યા છે.

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ખાતાવહી ડેટાબેઝ `.data/motedb.mote` પર છે અને ડેટાબેઝમાં સંગ્રહિત નથી.

`verified` પાસ પુરાવા હોવા આવશ્યક છે; `doc/progress.md` `report` દ્વારા જનરેટ થાય છે.