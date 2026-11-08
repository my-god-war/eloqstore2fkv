# ଲିଜର୍ ସ୍ଥାନାନ୍ତର କରନ୍ତୁ |

`tool/fkv_ledger` ଏଲୋକ୍ଷ୍ଟୋର ସ୍କାନ୍ କରିବା, C ++ ସଙ୍କେତ ଏବଂ fkv ଯାଞ୍ଚ କରିବା ପାଇଁ ଗଛ-ସିଟର ବ୍ୟବହାର କରନ୍ତୁ |
ପ୍ରମାଣ motedb କୁ ଲେଖାଯାଇଛି |

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- next --limit 20
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
```

ଲିଜର୍ ଡାଟାବେସ୍ `.data/motedb.mote` ରେ ଅଛି ଏବଂ ଡାଟାବେସରେ ଗଚ୍ଛିତ ନୁହେଁ |

`verified` ନିଶ୍ଚିତ ଭାବରେ ପାସ୍ ପ୍ରମାଣ ଅଛି; `doc/progress.md` `report` ଦ୍ୱାରା ସୃଷ୍ଟି ହୋଇଥାଏ |