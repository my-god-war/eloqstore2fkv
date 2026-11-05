
## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:1319-1345` put_at：`begin_mapper_batch(tbl)` :1332（data+segment 同批）、`settle_mapper_batch(tbl, result.is_ok())` :1334、TTL 维护移到结算之后 :1336-1343（内联清理失败 warn 不吞）；失败注入测试 `tests/maintenance_batch.rs:136 put_over_large_value_manifest_failure_restores_segments` + `:200 segment_state`。
