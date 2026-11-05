# sto-62c Drop 与 DropTable 语义拆分

## 来源

`issue/new/sto-62-*` 的「Drop 语义合并」条目。

## 位置

- `crate/engine/fkv_api/src/lib.rs:72-74`：`Request::Drop { tbl }`（无文档注释，
  语义未写明）。
- `crate/engine/fkv_shard/src/lib.rs` `dispatch`：`Request::Drop` →
  `state.store.drop_table(tbl)`。
- `crate/engine/fkv_shard/src/lib.rs` `run_gated_write`：`Drop` 结束后
  `gate.remove_pending(&tbl)` 并把排队写回 `NotRunning`。
- `crate/engine/fkv_store/src/lib.rs:1235`：`delete(tbl, key, timestamp)`（单 key
  逻辑删除，已有实现但**无请求入口**）；`:2430` `drop_table(tbl)`。
- `crate/engine/fkv_runtime/src/lib.rs:718`：`drop_table` 便捷方法。

## 缺口

1. 单 key / 前缀删除没有请求类型：`Store::delete` 是孤儿 pub API（无 shard 调用）。
2. `Request::Drop` 的名字不表达「删整表数据」，与 C++ 的 `DropRequest` /
   `DropTableRequest` 对账时容易被误用成单 key 删除。
3. `cpp/eloqstore` 目录为空（submodule 未拉），C++ `DropRequest` 的精确定义
   （删表数据 vs 删表元数据）无法在本仓库核对，需先确认再改语义。

## 方案

1. 先补文档：`Request::Drop` 明确写「删整表（数据 + 元数据），等义
   `Store::drop_table`」，`Request::Truncate` 写「清空表数据但保留表」，避免误用。
2. C++ 头核对后二选一：
   - 若 C++ `DropRequest` 就是「删表数据」，Rust 保持 `Drop = drop_table`，
     ledger 标 ignored + reason；
   - 若 C++ `DropRequest` 是「删单 key/前缀」，新增 `Request::Delete { tbl, key,
     timestamp }` → `Store::delete`，`Response::Delete`，并在 `op_of` / `is_write_op`
     / 指标标签（sto-60）同步补齐；`Envelope` 取消与写队列语义自动复用。
3. 前缀删除不在本条做：当前 `Store` 无 `delete_range`，需要 `Scan` + 批量 tombstone
   的独立设计。

## 验收

- `Request::Drop` 文档注释写明 drop_table 语义；新增变体（若确认）后
  `match` 穷尽、`op_of` 无遗漏。
- 单 key 删除后 `Reopen` 读不到该 key，同表其它 key 不受影响。
- `Request::Drop` 现有行为不变（`fkv_runtime` 现有测试全绿）。
- `cargo clippy ... -D warnings` 零警告。

## 重叠

- 请求类型覆盖与指标标签：`issue/todo/12a-per-table-request-gaps.md`、
  `issue/todo/sto-60-metrics-surface-diverges-and-17-bucket-claim-unverifiable.md`。
- oneshot 完成契约：`issue/done/sto-62a-oneshot-cancel-token.md`。

## DONE（2026-11-04）

结论：`Drop` 保持 `drop_table` 语义，**不新增** `Request::Delete`。

### C++ 核对（`fkv/cpp/eloqstore` 为空；读仓库外 `/Users/z/git/db/kv/eloqstore`）

- `include/eloq_store.h:576` `DropRequest : WriteRequest { SetArgs(TableIdent) }`；
  handler `src/storage/shard.cpp:1122` → `BatchWriteTask::Drop()`（`src/tasks/batch_write_task.cpp:1893`）
  = `IoMgr()->DropManifest(tbl)` + 清内存 `RootMeta`（root_id/ttl_root_id/manifest_size/mapper…）
  + 触发 mode-aware GC。即**删该分区的表数据 + manifest**，不是单 key / 前缀删除。
- `include/eloq_store.h:586` `DropTableRequest` 是 store 级请求；
  `src/eloq_store.cpp:1527 HandleDropTableRequest` 只是 `CollectTablePartitions` 后把
  `DropRequest` 扇出到该表所有分区。故 Rust `Request::Drop { tbl }`（单个 `TableIdent`）
  语义 == C++ `DropRequest`；store 级 DropTable 的「全分区扇出」由上层对每个分区分别
  下发本请求实现。
- 单 key 删除在 C++ 由 `BatchWriteRequest` 携带 delete op 完成，`RequestType`（`eloq_store.h:39-65`）
  里没有独立 Delete 请求。Rust 对应 `Request::Put { batch }` 里的 `BatchEntry::delete`；
  `Store::delete` 是该路径的便捷封装，**不是孤儿**（`fkv_sdk::delete` 调用它）。

### 改动

- `crate/engine/fkv_api/src/lib.rs`：给 `Drop` / `Truncate` / `Reopen` 补文档注释，明确
  `Drop` = 按分区删整表（同义 `Store::drop_table` / C++ `DropRequest`）、
  `Truncate` = 清内容保留表、单 key 删除走 `Put`+delete 而非 `Drop`。纯注释，无行为变更。
- `crate/facade/fkv_sdk/tests/sdk.rs`：新增
  `sdk_delete_keeps_siblings_drop_and_truncate_semantics`——单 key 删除只影响目标 key
  且重启后 tombstone 仍生效；`drop_table` 只清目标分区、兄弟分区不动；`truncate`
  清空后同表仍可读写。

### 验收

- `cargo nextest run -p fkv_api -p fkv_sdk` 全绿（5 passed）。
- `cargo clippy -p fkv_api -p fkv_sdk --all-targets --no-deps -- -D warnings` 零警告。
- `cargo fmt -p fkv_api -p fkv_sdk -- --check` 通过。
- `Request::Drop` 无行为变更（仅注释）；未新增变体，`op_of` / `match` 无需补项。
- 注：`cargo nextest run -p fkv_runtime` 当前因并发会话在改 `fkv_runtime`
  （`PrewarmCrond` 的 `I: PageIo` 约束不满足，`fkv_runtime/src/lib.rs:336/460`）编译失败，
  与本条无关，未能运行；本条对 `fkv_api` 只增注释，不影响 fkv_runtime 行为。

### ledger

- 无需改：C++ `DropRequest` 与 Rust `Drop` 一一对应（Rust 额外覆盖 store 级 DropTable 的
  扇出职责）；`doc/metrics.md` 的 `Drop ↔ drop + drop_table` 映射维持。


## 复审（2026-11-05 r6）

**通过。** 核实：`Request::Drop`/`Truncate`/`Reopen` 文档注释已补（`fkv_api/src/lib.rs` Drop 臂注释指明 = `drop_table` = C++ DropRequest，单 key 删除走 Put+delete）；未新增变体，`op_of` 无需补项。测试 `fkv_sdk/tests/sdk.rs:202` `sdk_delete_keeps_siblings_drop_and_truncate_semantics` 在位。C++ 核对结论（DropRequest=分区级删表+manifest）引用路径合理。
