# eng-60b create_branch pre-compact 未覆盖 segment 文件

## 位置

- Rust: `crate/engine/fkv_store/src/lib.rs` `compact_before_branch`（当前只处理 data 文件）
- C++: `BackgroundWrite::DoCompactSegmentFile`，由 `CreateBranch -> Compact()` 调用

## 现状

数据文件 pre-compact 已落地（eng-60d）。segment mapper 无 compaction：
含大 value 的父表创建 branch 时，碎片 segment 文件仍被 branch 继承。

C++ 的 `DoCompactSegmentFile` 按 `segment_file_amplify_factor` 选文件，
把 live segment 重写到文件尾，最后由 `RunFileGc` 删除旧 segment 文件。

## 方案

1. `fkv_task_compact` 增加 `compact_segment_file`：遍历指定 `file_id` 内的 live segment page，
   `read_segment`（整段 `segment_size`）→ `remap_page` → `write_segment`。
2. `compact_before_branch` 增加 `SegmentIo` 约束（或拆到独立 impl 块），
   用 `fkv_gc::file_live_count` + `segment_file_amplify_factor` 选文件，尾部 segment 文件跳过。
3. 处理最后一段短写：重写按整段长度，读取端按 `actual_len` 截断，需确认各后端
   对整段读取短文件的补齐行为（`io-28` 定长段语义）。
4. 完成后调用 `gc`，确认 branch 区间预留（`branch_segment_limit`）不与预压缩搬移冲突。

## 验收

- 含大 value 的碎片父表 `create_branch` 后，segment 文件集合无死文件。
- branch 大 value 读取正确，reopen 后一致。
- `cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast` 全绿。
- `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

本文件为遗留项，未实现：segment compaction 能力未就绪，待后续 issue 处理。

## 复审（2026-11-05 r6）

结论：**通过（经 sto-56b 承接落地）**。本文件 DONE 小节「未实现」表述已过时——
segment compaction 基础设施与 create_branch 接线均已在当前源码存在。

- `fkv_task_compact/src/lib.rs:124-147` `compact_segment_file_li`（自带批次，失败 abort）；`:183-205` 批式整段读→remap→写回。
- `fkv_store/src/lib.rs:2939-2944` `compact_before_branch` 的 segment 臂：`segment_factor != 0` → `segment_compact_plan` → `compact_segment_file_li`；`:2947` 末尾无条件 `gc`（对齐 C++ `Compact()` + `RunFileGc`）。
- `:3074` `segment_compact_plan`（复用 fkv_gc `should_compact`，`fkv_gc/src/lib.rs:74`）。

测试缺口（遗留，非阻塞）：`tests/segment_compact.rs` 覆盖 `compact_segment_if_needed`
维护路径与 branch 引用保留，`tests/branch_store.rs:299` 只测 data 侧 precompact；
「create_branch 触发 segment 预压缩」这一 exact 路径无直测。记入 r6 lane 报告遗留风险。

## 更正（2026-11-05 r9b-02）

上节「测试缺口（遗留）」已闭环：`create_branch` 触发 segment 预压缩的 exact 路径直测
已由 r9b-02 补齐——`crate/engine/fkv_store/tests/branch_store.rs:388`
`create_branch_precompacts_fragmented_segment_parent`（含 factor=0 对照 :591），
断言搬移位置、预留区间不重叠、死文件回收与 branch 全量可读。
跟踪：`issue/done/r9b-02-create-branch-segment-precompact-exact-path-zero-test.md`。
