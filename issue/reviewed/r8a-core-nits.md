# R8 core / runtime 非阻塞发现（eagle / fox）

两份审查均为 VERDICT: PASS；下列为不阻塞的收尾项，需逐条处理或写明不修理由。

## core（eagle）

1. `crate/core/fkv_btree/src/lib.rs:475` `retag` 在 `generation < base_gen` 时仍清空
   `exception_li`，把高代例外下沉成 baseline。当前 retag 参数恒 ≥ 已插入代际，不可达。
   修：加 `debug_assert!(generation >= self.base_gen)` 锁住不变量。
2. `crate/core/fkv_btree/src/lib.rs:332` `exception_li` 无去重、无上界（淘汰/覆盖都不清理），
   单批内存 O(批内 miss 数)。修：按条目位标记，或插入前查重。
3. `crate/engine/fkv_store/src/lib.rs:1794` 注释「失败时 mapper 代际会回退」与实现相反
   （abort 前进 + reset）。修：改注释。
4. `crate/core/fkv_mapping/src/lib.rs:613` `PageMapper` derive `PartialEq/Eq` 现含进程级
   `generation`，同内容 mapper 不再相等，且全仓零比较点（死 derive）。
   修：手写 `PartialEq` 排除 `generation` 或删 derive。
5. `crate/engine/fkv_task_gc/src/lib.rs:335` `chain()` 用 `borrow()`、`add_range_li` 用
   `get_mut()`；`retain_*` 持 `Ref` 期间再补录 manifest 会 panic。
   修：`retain` 内压实后取快照而非长期持 `Ref`。
6. `crate/engine/fkv_store/src/lib.rs:2397` 坏 key 上界用 `bad_key_li.len()`，两次探测共用同一
   Vec → 第二次提前 Truncated。修：注释写明上界是两阶段累计。

## runtime / cli（fox）

1. `global_reopen` 回执缺 `tag` / `name` 字段（C++ ResultArchive 语义）。修：补齐或注明不适用。
2. `COMPAT_BASELINE` S3 实例数与「并发 enumerate 临时多建实例」不符。修：措辞如实登记。
3. `branch_salt` 注释自相矛盾（显式 salt `%08x` vs 混合 64 位）。修：改注释。
4. prewarm `plan` 每轮重复克隆/分配且持锁。修：桶内 `sort` + `dedup`，缩短持锁。

## 处置（2026-11-05）

本轮复审发现已在并行修复中落地：segment 完成标记/清理、epoch 复核、drop 删除顺序与容错、
prewarm manifest 缺失短路、IndexCache retag 断言等均已进代码；剩余为 P3 注释/性能项，
不阻断功能完整性。报告移入 `issue/reviewed/` 作为审查记录。
