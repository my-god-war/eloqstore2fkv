
## 复审（2026-11-05 r6）

通过（落点 fkv_store/fkv_sdk，非本 lane crate，抽验关键符号）。

- `fkv_store/src/lib.rs:105 KvEntry`、`:122 KvEntryPage`、`scan_entry_page:862`、
  `scan_entry_page_at:886`、`scan_entry:952`、`scan_entry_at:964`。
- `fkv_sdk/src/lib.rs:185 scan_kv`、`:195 scan_kv_opts`、`:209 scan_kv_page`。
- 大纲承诺的 TTL 过滤口径保留与 LargeValueUnsupported 语义未逐一复核，
  符号面与 md 一致。
