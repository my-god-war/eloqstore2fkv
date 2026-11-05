# R7b 云缓存只读审查 findings

范围：`crate/engine/fkv_cloud`、`crate/io/fkv_object`、`crate/io/fkv_object_s3`、`crate/engine/fkv_task_prewarm`、`crate/engine/fkv_shard/src/backend.rs`。
基线：`main @ 961f5be`（工作区改动）。审查方式：只读，未改代码。

验证：`cargo check`（3 crate）通过；`cargo nextest run -p fkv_cloud -p fkv_object_s3 -p fkv_task_prewarm` 93/93 PASS；clippy 同范围 `--all-targets --all-features --no-deps -D warnings` 0 告警。
结论：`VERDICT: FAIL`（P1 一条 durability 缺陷）。

---

## P1

### 1. `drop_manifest` 只删 identity term 名，换 term 后未写即 Drop 会复活

- 位置：`crate/engine/fkv_cloud/src/lib.rs:2288`（`drop_manifest`），调用点 `crate/engine/fkv_store/src/lib.rs:3539`（`drop_table`）、`:3803`（`cleanup_empty_partition`）。
- 现象：`manifest_key` 固定用 `identity().term`。attach 抬高 term（远端权威 manifest 仍在旧 term `T_prev < T`）且本 term 尚未写过该表时，`remote.delete("manifest_<branch>_T")` 是空操作（S3 NotFound → Ok）；本地 cache 被删但远端 `manifest_<branch>_T_prev` 仍在。之后 `read_manifest` 按 `latest_remote_manifest`（同 branch 最大 term）重新选中旧 manifest → 表复活。`prune_superseded_manifest_file_li` 要求“远端最高 term == 当前 term”才剪枝，此时无当前 term manifest，永远不会回收，所以不是“GC 后自愈”，是永久复活。
- 现有测试未覆盖：`crate/engine/fkv_cloud/tests/cloud.rs:1091 drop_manifest_removes_current_term_anchor` 在同一实例同一 term 写后 Drop，identity term 与 manifest term 相同。
- 修法：先 `latest_remote_manifest(tbl)` 取对象名，`file_term <= identity.term` 时删除该名（必要时再删 identity term 名）；高于 identity term 的 manifest 仍不动（不破坏新 owner 的 anchor）。补用例：term 3 写入 → 用 term 4 的实例 reopen → `drop_table` → 断言远端无同 branch 无 tag manifest，且 reopen 返 `ResourceMissing`。

## P2

### 2. `prewarm_manifest` 在远端无 manifest 时采纳本地旧 manifest

- 位置：`crate/engine/fkv_cloud/src/lib.rs:2741`（`prewarm_manifest`），分支 `(Some(local), None) => local`。
- 现象：`read_manifest` 同场景是 `DropLocalManifest` 并返回 `None`（`lib.rs:2250` 附近），预热却把本地旧 manifest 当权威，返回 `manifest_present=true`，随后 `prewarm_paged` 按远端残留 data/segment（Drop 后尚未 GC）把整表重新拉回本地 cache。与 finding 1 叠加，是“Drop 后复活”的第二条路径。
- 兼带死代码：`.filter(|(name, _)| matches!(parse_manifest_file_name(name), Some((b, _, None)) if b == branch))` 与 `latest_remote_manifest` 内部过滤（`lib.rs:2503`）完全重复，可删。
- 修法：`(Some(_), None) => return Ok(false)`（并考虑顺手 `local.drop_manifest` / `remove_manifest_file`），删冗余 filter。

### 3. S3 并发额度均分漏计运行时侧实例

- 位置：`crate/engine/fkv_shard/src/backend.rs:69-76`（`s3_config`），实例数来源 `crate/engine/fkv_runtime/src/lib.rs:1246`（`s3_instance_num = num_threads + 1`）。
- 现象：运行时侧实际可同时存在 3 个 `CloudStore<S3ObjectStore>`：prewarm crond（`lib.rs:1444`）、`cloud_enum_store` 长驻（`lib.rs:1535`）、archive crond 惰性实例（`lib.rs:483`）。每实例按 `n/(threads+1)` 放行，整机飞行中请求上限可达 `(threads+3) × quota > max_cloud_concurrency`，与注释“保证整机不超过配置值”不符。另外 `(n/instances).max(1)` 在 `n < instances` 时总和 = instances > n。
- 修法：实例数按真实构造点计数（shard 数 + 运行时侧实际启用数，随 `prewarm_cloud_cache` / archive 配置变化），或改成跨实例共享的全局信号量（如 `Arc<Semaphore>`）后各实例不再拆分额度。

### 4. `adopt_local_file` segment 分支逐文件全目录扫描，预热 O(N²)

- 位置：`crate/engine/fkv_cloud/src/lib.rs:1822`（`adopt_local_file`），调用点 `crate/engine/fkv_task_prewarm/src/lib.rs:117`。
- 现象：segment 分支每次未驻留都 `self.local.list_segment_file_id_li(tbl)`（每次 read_dir 全表扫描）。预热的 `buffer_unordered` 对 N 个 segment 文件各调一次 → O(N²) syscall，大批 segment 表预热延迟与 fd 压力显著。
- 修法：预热入口一次性取本地 segment id 集合传参；或直接用已有的 `segment_file_complete`（单文件读尾槽，O(1)）替代 `contains`，二者叠加即可去掉目录扫描。

## P3

### 5. `cache_local_file` 把“写后文件不存在”当作成功

- 位置：`crate/engine/fkv_cloud/src/lib.rs:1613`。
- 现象：`file_size(...).is_some_and(|size| size != bytes.len() as u64)` 在 `file_size` 返回 `None` 时为 `false`，直接放行并按 `bytes.len()` `upsert_resident`。后端若写成功但不落盘（或实现有 bug）会被登记为完整 resident，后续读写不再回源，直到进程重启。
- 修法：改成 `match self.local.file_size(..).await? { Some(size) if size == len => {}, _ => { 清理 + Err(KvError::IoFail) } }`。

### 6. `evict_table` 未清理该表的在飞下载

- 位置：`crate/engine/fkv_cloud/src/lib.rs:2157`（`evict_table`），`inflight_download` 定义 `:520`。
- 现象：驱逐（detach / drop_table）只清 `table_map`、`file_range_map`、`restored_table_set`，不动 `inflight_download`。驱逐后仍在飞的 owner 完成时走 `cache_local_file` → `upsert_resident`，把已 detach 的表重新落盘并计入共享预算；shard 与预热共用 `local_root` 时这份文件还会被预热实例看见。
- 修法：`evict_table` 中按 `tbl` 摘除 `DownloadKey` 并给等待者 `Err(KvError::TryAgain)`；owner 完成后的 `upsert_resident` 前复核表仍在当前世代。

### 7. `ensure_resident` 的“两轮单飞、第一轮回源”在 waiter 路径不成立

- 位置：`crate/engine/fkv_cloud/src/lib.rs:1555`（注释与实现）。
- 现象：`refetch = attempt == 0` 按轮次而不是“当前任务是否真的执行了 fetch”。round 0 若只是 waiter（等待 `fetch_data_file` 之类 owner），拿到的 `Cached` 可能来自“本地文件已存在即返回”的快路径；进入 round 1 后 `refetch=false` 直接按“扩展写”放行，全程没有任何一方做 `remove_file` + 回源，注释承诺的短文件修复可能一次都没发生。
- 修法：`single_flight` 回传“本任务是否 owner”，或用一个 `did_fetch` 标志；当 round 0 是 waiter 且 round 1 仍不覆盖 `start` 时，再强制一轮 `refetch=true`（仍保留“远端确实短”的最终放行）。

---

## 已核对无问题

- `S3ObjectStore::Drop`（`crate/io/fkv_object_s3/src/lib.rs:284`）非阻塞投递：`join.is_none()` 短路、`try_send` 满/断开不阻塞、靠 `tx` 释放让 worker 退出；`shutdown()` 显式等线程退出的分工正确。
- `single_flight` / `DownloadGuard`（`:398-420`）：owner 取消唤醒等待者、`remove` 与 `send` 之间无 await 窗口，无永久挂起。
- `evict_table` 清 `restored_table_set`（`:2164`）后按表重扫，预算与驱逐不再脱钩；`trim_cache` 待删队列上界 `EVICT_PENDING_FILE_MAX` 成立。
- `fkv_object` trait 的 `list_page` 契约（升序 / 严格大于 token / max_keys=0 不限）在 `MemObjectStore` 与 S3 侧实现一致。
- 范围内无 `#[allow]`；除 finding 2 的冗余 filter 外无新增死代码。


## 处置（2026-11-05）

本报告 findings 已在 r8/r9 与当前代码中修复；报告移入 `issue/reviewed/` 作为审查记录。
