# 33a fkv_root 接线或删除：RootMeta/RootMetaMgr 与 Store 每表状态重复

> 由 `33-orphan-crates.md` 拆分。原 issue 的其余部分已落地，见其 `## 本轮落地`。

## 位置

- `crate/core/fkv_root/src/lib.rs`（全 239 行）：`RootMeta`、`RootMetaMgr`（LRU + pin/ref_cnt + `evict_if_needed`）。
- 无任何 crate 依赖：全仓 `Cargo.toml` 只有自身 `name = "fkv_root"`。
- `crate/engine/fkv_store/src/lib.rs` 自带每表状态：`branch_metadata_map`（:125）、`mapper`/`segment_mapper`（:934、:1527 等）、`snapshot_record`（:1596）。
- C++ 对照：`src/storage/root_meta.cpp`、`include/storage/root_meta.h`，由 `RootMetaMgr` 管每表 RootMeta。

## 现状

`fkv_root` 是 C++ `RootMeta` 的结构化搬运，但 Rust 的 `Store` 用散落字段 + 每表 map 承担了同样职责，
所以 `fkv_root` 从落地起就没有消费者，单测也只覆盖自己。
`eng-73-store-per-table-state-maps-consolidation.md` 正在收敛 Store 的每表状态，
两者若各自演进会形成第二套 RootMeta 语义。

LRU 实现细节（不是主因，但接线前必须改）：

- `touch`（`lib.rs:228-238`）非尾部命中时 `VecDeque::retain` + `push_back` → O(n)。
- `remove`（`lib.rs:180-185`）同样 `retain` → O(n)。
- `evict_if_needed`（`lib.rs:187-226`）入口全量重算 `used_bytes` 是刻意为之（mapper 会增长），保留；
  但 `used_bytes` 增量维护在 `get_or_create` 之外没有更新点，接线后需要每次 `memory_bytes()` 变化后校正。

## 方案（二选一，先决策再动手）

A. 删除（推荐，若无独立价值）：
   - 确认 `eng-73` 的每表状态结构能承载 `first_unflushed_fp_id` / `first_unflushed_seg_fp_id` /
     `compression` / `manifest_size` / `next_expire_ts`；
   - `git rm -r crate/core/fkv_root crate/core/fkv_root_cpp`，从 `CRATE` 映射（`tool/fkv_ledger/src/map.rs:29-30`）与
     `doc/architecture/01-overview.md`、`doc/architecture/06-gaps.md`、`todo.md` 清理引用；
   - 注意 `fkv_ledger` 的 unit 映射需同步 `ignore` 或改指 `fkv_store`。

B. 接线（若确实要独立缓存层）：
   - `Store` 的 `branch_metadata_map` + 每表 mapper 收敛为 `RootMetaMgr`，
     `Store::branch_metadata` / `install_replay_state` 改走 `get_or_create` / `pin` / `release`；
   - 先做 O(1)：`VecDeque` 换成侵入式双向链（`Entry` 存 `prev`/`next` 索引）
     或 `gxhash::HashMap<TableIdent, usize>` 记录位置 + 链表；
   - `first_unflushed_*` 的更新点必须与 flush 完成对齐，否则重复 `core-14` 的语义缺口。

## 验收

- `./tool/orphan.sh` 通过，且 `tool/orphan_allow.tsv` 中不再有 `fkv_root`（删除或接线都满足）。
- 若删除：`cargo check --workspace --all-targets --all-features` 无引用残留，文档无 `fkv_root`。
- 若接线：新增集成测试覆盖「同一表连续访问」「超过容量淘汰」「pin 后不淘汰」，并给出 LRU 操作 O(1) 的复杂度断言（可在测试里用计数钩子，不追求基准）。

## DONE（2026-11-04）

**决策：采纳方案 A（删除）。** 未采用的方案 B（接线）本轮不可行且无必要。

删除依据（源码实读，不只看 issue 断言）：

- 零消费者：`cargo metadata --no-deps | jq '.packages[] | .dependencies[]? |
  select(.name=="fkv_root") | .name'` 空；`rg -n fkv_root`（排除 target/.git/issue）
  只命中 `tool/fkv_ledger/src/map.rs` 与文档。
- 职责已被覆盖：`eng-73a`（done）把 Store 每表状态收敛为
  `TableState{mapper, segment_mapper, root, ttl_root, branch_metadata,
  manifest_started, manifest_size, next_expire, compression, index_cache,
  ttl_pending}`（`crate/engine/fkv_store/src/lib.rs`），覆盖 RootMeta 的
  root/ttl_root/mapper/manifest_size/next_expire/compression/branch_metadata；
  表级 LRU 由 `touch_table`/`evict_table` 承担。
- 唯一独有的 `first_unflushed_fp_id` / `first_unflushed_seg_fp_id` 语义
  **在 C++ 侧已被废弃**：`eloqstore/src/file_gc.cpp:145-155` 注释明写
  「Derive the in-flight boundary from the live allocators rather than the
  RootMeta::first_unflushed_*_fp_id_ snapshot fields ... a data-only flush
  leaves first_unflushed_seg_fp_id_ stale (0)」，改用 live mapper 的
  `FilePgAllocator()->MaxFilePageId()`（即 Rust `PageMapper` 的高水位）。
  Rust 侧该 in-flight guard 缺口由 `core-23-file-gc-missing-per-branch-
  inflight-guard.md` 归口，实现不依赖 `fkv_root`。
- 因此删除不丢语义；`fkv_root` 的 `ref_cnt/locked/evict_if_needed`
  在 Rust Store 中本就无对应调用点。

改动文件：

- 删除 `crate/core/fkv_root/`（Cargo.toml、src/lib.rs、tests/root.rs）与
  `crate/core/fkv_root_cpp/`（空壳对照，1 行 doc comment）。
- `tool/orphan_allow.tsv`：删 `fkv_root` 行（现 7 项）。
- `tool/fkv_ledger/src/map.rs`：`include/storage/root_meta.h`、
  `src/storage/root_meta.cpp` 的映射 `fkv_root(core)` →
  `fkv_store(engine)`（RootMeta 职责现落在 `fkv_store::TableState`）。
- `doc/adopt.tsv`：删 `fkv_root done crate/core/fkv_root/tests` 行
  （目录已不存在，留着会让 `adopt` 指向空路径）。
- `doc/architecture/01-overview.md`：L2 列表去掉 `fkv_root`。
- `doc/architecture/06-gaps.md`：孤儿行去掉 `fkv_root` / `33`。
- `todo.md`：C++→Rust 映射行改指 `fkv_store`；删 crate 清单行；
  `fkv_task_read` / `fkv_task_write` / `fkv_recovery` / `fkv_compaction`
  依赖列去掉 `fkv_root`；M2 清单去掉 `fkv_root`。
- `doc/progress.md`：重新生成（`map --seed --force` + `report --out`）。
  除 `fkv_root` 行消失外，`fkv_store` 198→241（正好吸收原 47 个 unit），
  并同步吸收了其它 lane 已改的 RULE_LI（`fkv_cache` 已无规则、
  `archive_crond`→`fkv_runtime` 等）与 issue 目录计数。
- `Cargo.lock`：cargo 自动删除 16 行 `fkv_root` / `fkv_root_cpp` 条目。

验证：

- `./tool/orphan.sh` → 退出 0，白名单 7 项，不再含 `fkv_root`。
- `cargo check --workspace --all-targets --all-features` → 通过（首次因
  `fkv_task_write` 他人未完成编辑失败，等 30s 重试通过）。
- `cargo test -p fkv_ledger` → 7 + 8 passed。
- `cargo clippy -p fkv_ledger --all-targets --no-deps -- -D warnings` → 零警告；
  `cargo fmt -p fkv_ledger -- --check` → 干净。
- `rg -n fkv_root`（排除 target/.git/issue）→ 无命中。

## 复审（2026-11-05 r6b）

通过。

- `crate/core/` 与全仓 `crate/` 下无 `fkv_root` / `fkv_root_cpp` 目录（find 0 命中）。
- ledger 映射已改：`tool/fkv_ledger/src/map.rs:29-30`（`root_meta.h`/`root_meta.cpp` → `fkv_store(engine)`）。
- `tool/orphan_allow.tsv` 无 `fkv_root` 行（与删除决策一致）。
- 删除依据（C++ 侧 `first_unflushed_*` 已废弃、改用 live allocator 高水位）与本仓
  `core-23` 的 InflightGuard 实现互证，语义未丢失。
