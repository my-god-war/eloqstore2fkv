
## 复审（2026-11-05 r6）

通过。`fkv_runtime/src/lib.rs:616`（`PrewarmCrond`，:726 Drop 停止）、`:817`
（`Runtime.prewarm_crond` 字段）、`:1240`（`start_impl` 起线程）；`fkv_cloud/src/lib.rs:2186`
（`set_identity`，按 attach 的 (branch,term) 对齐远端对象名）、`:2198`
（`remote_partition_page` 跨页去重列举）；`fkv_task_prewarm/src/lib.rs:16`
（`PREWARM_TABLE_CONCURRENCY` 提 pub）。测试：`fkv_runtime/tests/prewarm.rs` 存在，
`fkv_task_prewarm/tests/prewarm.rs:171` 配套。遗留（预热线程与 shard 各持 CloudStore、
极端 2× 预算；S3 端到端需 EKV_S3_ENDPOINT）已在 DONE 记录，不重复立项。
