# eng-92 剩余内联测试迁 tests/ 的路径与收尾

## 现状

本轮（对标 C++ 清理测试）已迁：fkv_btree `index_cache_tests`（9）→ `tests/index_cache.rs`、fkv_mapping `snapshot_tests`/`journal_tests`（3）→ `tests/snapshot.rs`、fkv_object_s3（4）→ `tests/error_classification.rs`（`#[doc(hidden)] pub` 支撑项：`map_err` / `status_error` / `http_status` / `RetryPolicy::from_lookup` / `to_retry_config`）。

保留内联的 5 处及原因：

| 位置 | 数量 | 阻塞点 |
|---|---|---|
| `fkv_task_write/src/lib.rs` `unit_tests` | 16 | 依赖 ~20 个私有项（`build_index_page_li` / `rebalance_*` / `Child` / `IndexLayout`…），迁出即必须 pub 化整条内部链，与「pub 接口收敛」目标相悖 |
| `fkv_shard/src/lib.rs` + `write_gate.rs` | 11 | 构造私有 `Envelope` / 按字段拼 `ShardSender`、`merge_put_batches`；`write_gate` 模块本身私有 |
| `fkv_runtime/src/lib.rs` prewarm/global tests | 8 | 改私有 `PrewarmState` 字段、调私有 `lock_prewarm_state` / `backend_of` |
| `fkv_io_compio/src/fixed.rs` tests | 3 | 依赖 `pub(crate)` 的 `FixedRead` / `FixedWrite` / registry lease；且是 Linux-only 单元 |
| `fkv_bench/src/main.rs` tests | 20 | bin 目标没有集成测试通道，需抽 lib 才能迁 |

## 方案

按收益排序：

1. `fkv_io_compio/fixed.rs`：3 个测试迁 `tests/fixed_unit.rs`，`FixedRead` / `FixedWrite` / `FixedBufferRegistry` 按 `#[doc(hidden)] pub` 暴露（与 s3 同法）；Linux-only 用 `#[cfg(target_os = "linux")]` 保留条件编译。
2. `fkv_bench`：`src/lib.rs` 已存在（另一会话引入）；把 bin 私有函数逐个上移 lib + `#[cfg(test)]` 测试改挂 lib，bin 只留 `main`。
3. `fkv_shard`：`Envelope` / `ShardSender` 若后续消息层重构（read_spawn / cancel 已在动），顺手把测试需要的构造面收敛成 `#[doc(hidden)] pub fn envelope_for_test` 类单一入口，避免逐字段拼装。
4. `fkv_task_write` / `fkv_runtime`：维持内联（算法/私有状态不变量测试，内联是 Rust 惯例位置）；仅当未来拆分 `src/lib.rs` 为模块文件夹时随迁。

## 验收

- 每迁一处：`cargo nextest run -p <crate>` 数量不降、`cargo clippy -D warnings` 干净；
- `#[doc(hidden)]` 项只增测试支撑、不增业务 API。

## 处置（2026-11-05）

P3 测试/组织性工作，不阻塞功能与正确性；当前测试覆盖已由 `fkv_store` 既有 file
size 断言与各 crate 内联/集成测试承担，移入 `issue/reviewed/` 作为后续可选清理项。
