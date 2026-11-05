# eng-78 死代码与只写不读状态：7 个零调用 API、两组仅测试使用的 GC 旧接口、`garbage_file_page_li` 无界增长

## 位置

按当前源码全仓 grep（`crate/` + `tool/`）核对，以下符号在非测试代码中零调用或取出即丢：

| 符号 | 定义 | 使用情况 | 建议 |
|---|---|---|---|
| `CachedFd::identity` | `crate/io/fkv_io_compio/src/lib.rs:114` | 三处 `insert` 全部传 `None`（`open_read_file` / `open_write_file` / `create_file`），`get_read` 的第二个返回值在 `:842` 被 `_` 丢弃 | 删除字段，`get_read -> Option<File>`，`insert` 去掉参数 |
| `Store::check_branch_segment_limit` | `crate/engine/fkv_store/src/lib.rs:714-717` | 空实现；两处调用 `:1825`、`:1905` 只传参后 `?` | 删除函数与两处调用 |
| `MappingTbl::tbl_mut` | `crate/core/fkv_mapping/src/lib.rs:554-556` | 0 引用 | 删除 |
| `PageMapper::take_dirty_page_li` | `crate/core/fkv_mapping/src/lib.rs:571-573` | 0 引用（生产用只读 `dirty_page_li` + `clear_dirty_page_li`） | 删除 |
| `PageMapper::allocate_file_page` | `crate/core/fkv_mapping/src/lib.rs:608-610` | 0 引用（内部直接 `allocator.allocate()`） | 删除 |
| `PageMapper::free_page_head` getter | `crate/core/fkv_mapping/src/lib.rs:670-672` | 仅 `fkv_mapping/tests/mapping.rs` 5 处 | 改 `#[cfg(test)]` 或删除 |
| `FixedBuffer::registered` / `registered_arc` | `crate/io/fkv_io/src/lib.rs:165-167` / `:170-172` | 0 引用；生产只用 `registered_arc_with_hook` | 删除两个构造器 |
| `fkv_task_gc::free_page_li` | `crate/engine/fkv_task_gc/src/lib.rs:13-21` | 仅 `tests/gc.rs` | 移到测试或删除 |
| `fkv_task_gc::gc_files` / `gc_segment_files` | `crate/engine/fkv_task_gc/src/lib.rs:24-45` / `:48-69` | 仅测试；生产用 `*_with_retained`（`:127` / `:146`） | 删除旧接口，测试改用 retained API |
| `fkv_gc::unreferenced_file_page_li` | `crate/engine/fkv_gc/src/lib.rs:17-25` | 仅 `tests/gc.rs` | 删除 |
| `fkv_gc::file_amplification` | `crate/engine/fkv_gc/src/lib.rs:27-33` | 仅 `tests/gc.rs`；生产 `should_compact` 未调用 | 删除（或将 `should_compact` 改为调用它，二选一） |
| `fkv_gc::retained_file_page_set` | `crate/engine/fkv_gc/src/lib.rs:6-15` | 生产只剩 `fkv_task_gc` 的测试专用 `gc_files` 在用；生产等价逻辑是 `add_mapping_retained`（`crate/engine/fkv_task_gc/src/lib.rs:87-93`） | 合并到 `add_mapping_retained`，保留一个填充函数 |

另有两个“不是死代码、但语义已死”的状态：

- `PageMapper::garbage_file_page_li`（`crate/core/fkv_mapping/src/lib.rs:419`）在
  `remap_page` `:627`、`free_page` `:661` 每次追加，唯一的取出点是
  `truncate` `:1324` / `:1331` 的 `let _ = take_garbage_file_page_li()`；
  正常写路径从不消费。等于每个被 COW/释放的物理页都往一个无人读的 Vec 里追加一个
  `FilePageId`，长跑进程里无界增长。GC 实际以 manifest retained set 为准
  （`fkv_task_gc::manifest_retained_file_page_set`），不需要这个列表。
  `core-18` 的审查记录已提过“只写不读”；本条把无界增长证据与清理动作单独列出。
- `MappingTbl::value_li`（`crate/core/fkv_mapping/src/lib.rs:194-197`）仍被
  `fkv_recovery::max_file_page_id`（`:82-90`）使用，但它与 `to_vec` 完全同构，
  且两者都会摊平整张表；恢复路径可直接改 `iter_values()`，删除这个“兼容旧接口”。

## 现状

- 这些 API 都因为 `pub` 逃过了 dead_code 警告；`cargo clippy` 当前只剩两条无关警告，
  说明编译器层面没有暴露它们。
- `gc_files` / `gc_segment_files` 与 `*_with_retained` 是同一逻辑的两代实现：
  旧版从内存 mapper 算 retained，新版从 manifest ∪ 内存算；生产只应保留新版。
  旧版留在库里，测试继续验证它，会给人“两条 GC 路径都有效”的错觉。
- `garbage_file_page_li` 的写入点是每个 COW 页，读点是 truncate；如果 truncate 从不发生，
  列表只增不减。当前没有测试覆盖其长度上界。

## C++ 实现

C++ 的 retired 物理页列表是 `PageMapper::to_free_file_pages_`（`include/storage/page_mapper.h:160-174`），
由 `FreeMappingSnapshot`（`src/storage/page_manager.cpp:731-756`）在快照引用清零后消费，
不是只写不读；也没有第二套 GC 入口。Rust 的 `garbage_file_page_li` 既没有消费者，
也没有快照引用语义，属于半成品状态。

## 建议

1. 一次性删除表中 12 个零调用/仅测试 API；`free_page_head` 若确需给测试断言，改
   `#[cfg(test)] pub fn`（不导出到生产 API）。
2. `gc_files` / `gc_segment_files` 删除，`tests/gc.rs` 改用
   `gc_files_with_retained` + `retained_file_page_set`（或新的统一 helper）；
   `fkv_gc::retained_file_page_set` 与 `add_mapping_retained` 只保留一个。
3. `garbage_file_page_li` 二选一：
   - 确认 GC 不需要 → 删除字段与三处 push，简化 `PageMapper`；
   - 未来要用于“物理页延迟释放” → 接入消费点（manifest 落盘后清空 / 交给 GC），
     并加不变量测试，禁止只写不读。
4. `value_li` 的恢复调用改 `iter_values().filter(...)`，随后删除 `value_li`/`to_vec` 中
   不必要的一个（`to_vec` 在 snapshot 重构后也应消失，见 eng-76）。
5. 在 CI 加 `cargo +nightly udeps` 或 `cargo machete` 只覆盖 crate 依赖；
   对 `pub` API 用 `cargo public-api` diff 或人工清单，防止再长回来。

## 严重度

P2（无行为错误；`garbage_file_page_li` 是无界内存，其余是维护噪音与测试误导）

## 核验记录（2026-11-04）

逐符号 `grep -rn` 确认：`tbl_mut` / `take_dirty_page_li` / `allocate_file_page` 各 1 处（仅定义），
`registered` / `registered_arc` 零引用，`free_page_li` / `gc_files` / `gc_segment_files` /
`unreferenced_file_page_li` / `file_amplification` 仅测试引用，
`garbage_file_page_li` 的 push 点 `:627/:661` 与唯一 take 点 `:1324/:1331` 实读。
C++ `to_free_file_pages_` / `FreeMappingSnapshot` 引用自 `include/storage/page_mapper.h:160-174`、
`src/storage/page_manager.cpp:731-756`。

## DONE（2026-11-04）

eng-81/eng-82 已先删掉 `unreferenced_file_page_li` / `file_amplification` / `preload_tables` / `inner_mut`，本 issue 按当前全仓 grep 复核后处理剩余条目。

判定与动作：

- 删除（零调用，无外部契约）
  - `CachedFd::identity`：三处 insert 全传 `None`，`get_read` 丢弃第二个返回值；删字段、参数，`get_read/touch -> Option<File>`。
  - `Store::check_branch_segment_limit`：空实现，唯一调用点删除。
  - `MappingTbl::tbl_mut`、`PageMapper::take_dirty_page_li`、`PageMapper::allocate_file_page`、`PageMapper::free_page_head` getter。
  - `FixedBuffer::registered` / `registered_arc`：生产只用 `registered_arc_with_hook`。
  - `fkv_task_gc::free_page_li` / `gc_files` / `gc_segment_files`：测试改用 `*_with_retained` + `add_mapping_retained`；`fkv_task_gc` 去掉 `fkv_gc` 依赖。
  - `fkv_gc::retained_file_page_set`：与 `add_mapping_retained` 重复，只保留后者（manifest 并集路径在生产使用）。
  - `garbage_file_page_li`：三处 push、`take_garbage_file_page_li`、`BatchJournal.garbage_len` 与 truncate 两处 `let _ =` 取出点全部删除。GC 以 manifest retained set 为准，该列表无消费者且无界增长。
  - `MappingTbl::value_li`：仅 `fkv_recovery::max_file_page_id` 一处生产调用；改为 `iter_values()`，测试调用点同步替换。快照编码已走 `iter_values`（eng-76），不需要兼容层。
- 保留
  - `manifest_retained_file_page_set`（生产入口）。
  - `MappingTbl::serialize/deserialize`、`iter_values`、`iter`：快照 / 重放契约。
  - `should_compact` / `file_live_count` / `pick_compact_file`：`fkv_store` compact 路径在用。
  - `PageMapper::dirty_page_li` + `clear_dirty_page_li`、`take_changed_page_li`：生产读写路径在用。

测试断言替换：`fkv_large` 两处 garbage 长度断言改为「无活跃 `FilePageId` 槽位」；`free_page_head` 5 处断言由 `free_page_cnt` + 后续 `allocate_page` 覆盖，并补一条「链表空后追加新槽位」。

验证（2026-11-04）：

- `cargo check --workspace --all-targets --all-features` 通过
- `cargo nextest run --workspace --all-features --no-fail-fast`：484 passed, 1 skipped
- `cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings` 零警告

## 复审（2026-11-05 r6）

通过。`garbage_file_page_li`/`take_garbage_file_page_li` 在 `fkv_mapping`/`fkv_store` 全删（grep 零命中）；eng-81 注中归属本条的 `tbl_mut`/`take_dirty_page_li`/`allocate_file_page` 亦零命中；`fkv_large` 仅剩注释提及 garbage（src/lib.rs:267）。
