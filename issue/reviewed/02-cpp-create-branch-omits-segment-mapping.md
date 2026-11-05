# 02 P1：C++ `CreateBranch` 的 snapshot 省略 segment mapping，Rust 为 large value 继承有意保留

来源：2026-11-05 manifest snapshot 语料修复（`issue/reviewed/01-cpp-oracle-and-fixtures.md`
修复记录）。本条目只记录**字节结论 + 决策缺口**，本批未改 C++ 外部源码。

## 字节结论（eloqstore 82b774c）

- `src/tasks/background_write.cpp:681-682` `BackgroundWrite::CreateBranch` 调用
  `wal_builder_.Snapshot(root, ttl_root, mapping, new_max_fp_id, dict_bytes,
  branch_metadata)`；`include/storage/root_meta.h:61-68` 的后两个参数
  `segment_mapping = nullptr`、`max_segment_fp_id = 0` 是默认值。
- `src/storage/root_meta.cpp:141-153` `ManifestBuilder::Snapshot` 只在
  `segment_mapping != nullptr` 时追加
  `max_segment_fp_id(varint64) | seg_len(4B) | seg mapping`。
- 因此 C++ branch snapshot **确实没有**可选 segment 段；不是解析器漏读。
  对照 `CreateArchive`（`background_write.cpp:570-584`）显式传
  `meta->segment_mapper_ ? GetMapping() : nullptr`，archive 有该段。

## 影响

- C++ 从带 large value 的父分支 fork 后，branch manifest 不含 segment 映射；
  reopen 时 `Replayer::GetSegmentMapper`（`src/replayer.cpp:431-437`）在
  `segment_mapping_tbl_.size()==0 && max_segment_fp_id_==0` 时返回 nullptr，
  底页的 large_value 指针无法解析。即 C++ 的 branch + large value 继承在
  manifest 层是断的（上游未覆盖该场景，`tests/branch_operations.cpp` 无 large value）。
- Rust `create_branch` 在启用 large value（`large_value_layout.is_some()`）或已有
  segment mapper 时创建并预留 segment 区间、写 segment 段，使：
  1. 分支能继承父分支 large value（测试
     `fkv_store/tests/branch_archive.rs::branch_snapshot_with_large_value_keeps_segment_mapping`）；
  2. main / branch 的 segment file_id 区间不重叠（`branch_isolation.rs`）。
- plain store（未启用 large value）已与 C++ 逐字节一致：不创建 segment mapper、
  不写 segment 段、branch metadata 的 `max_segment_file_id=0`。

## 待决策

1. **以 C++ 字节为唯一权威**：Rust 也省略 branch segment 段，明确「branch 不继承
   large value」，删除/改写 branch segment 继承与预留逻辑；需要接受行为回退。
2. **保留 Rust 扩展**：要求 C++ 侧修复 `CreateBranch` 传 segment mapping（或声明
   branch + large value 不在互操作范围），并把该差异写入 `COMPAT_BASELINE` 排除项。
3. 当前批次选择 2 的 Rust 侧行为，但未改 C++；因此 `COMPAT_BASELINE` 的
   L1-partial 仍不应把「branch + large value」计入逐字节等价。

## 验收（任一决策落地）

- 决策记录进 `COMPAT_BASELINE`；若上游修复，更新 baseline commit 并重新生成
  `fixtures/cpp` / `fixtures/rust`（含 branch snapshot 的 segment 段）。
- 若选择 1：Rust 测试删除/改写 branch large value 继承断言，并补一条
  「branch snapshot 无 segment 段」的字节断言（无论父表是否有 segment）。
- 若选择 2：Rust 保留现有测试；新增 C++ 侧（或 issue）确认 C++ branch 不写该段
  的已知限制，避免后续误判为 Rust 回归。

## 甄别（2026-11-05）

结论：**采纳为「登记收尾」任务（原待决策 2 已生效，剩余为文档同步）**。
字节结论经本人实读双方源码逐项核实属实；决策登记已完成大半，未到「拒绝
（已登记）」程度——`doc/migration.md` 排除项清单与 6 处引用路径仍待同步。

### 核验记录（eloqstore 82b774c / fkv 当前源码，均本人实读）

- C++ `src/tasks/background_write.cpp:681-682`：`CreateBranch` 的 `Snapshot` 仅传
  6 参，`segment_mapping`/`max_segment_fp_id` 取默认值（`include/storage/root_meta.h:67-68`
  `= nullptr` / `= 0`）。对照 `CreateArchive`（同文件 `:570-584`）显式传
  `seg_mapping` / `max_seg_fp_id`。属实。
- C++ segment 段条件追加实际位于 `src/storage/root_meta.cpp:119-130`
  （原文写 `:141-153`，行号小误，内容属实）：仅 `segment_mapping != nullptr`
  时追加 `max_segment_fp_id(varint64) | seg_len(4B) | mapping`。
- C++ 断链因果属实：`src/replayer.cpp:431-437` 无 segment 段时
  `GetSegmentMapper` 返回 nullptr；`src/tasks/read_task.cpp:53-57` segment
  mapper 为 null 时 `seg_mapping_ref` 为 null，large value 指针不可解析。
  `tests/branch_operations.cpp` 无 large value/segment 用例（grep 为空）。
- Rust `crate/engine/fkv_store/src/lib.rs:2943` `create_branch`：
  `:2989` `use_segment = segment_mapper.is_some() || large_value_layout.is_some()`
  → `:3046` 向 `snapshot_record` 传 `Some(branch_segment_fp_id)`，分支 snapshot
  写 segment 段并预留区间；`:1084-1086` 注释明确 plain store 省略段与 C++ 同构。
- 对齐 C++ 会破坏继承的因果属实：`lib.rs:2828-2830` `resolve_value` 在
  `is_large_value` 且无 segment mapper 时直接 `ResourceMissing`；reopen 侧
  `lib.rs:1860` 仅当 replay 有 segment mapping 才重建 mapper。故若省略段，
  分支重开后父分支 large value 全部不可读。`eng-60b`（segment 预压缩）也以
  「branch 继承 segment」为前提，佐证有意扩展。
- 测试已双覆盖：`fkv_store/tests/branch_archive.rs:368`
  （plain store branch/archive 无 segment 段，字节同构）与 `:439`
  （large value branch 保留段 + 重开读回）。

### 登记现状核查

- 已登记：`COMPAT_BASELINE:10-13`（「branch + large value …属有意差异，不纳入
  逐字节等价」）；`doc/architecture/06-gaps.md:47`；`doc/progress.md:6`（ledger
  镜像）；`fixtures/README.md:157`。原意见验收第 1 条与「若选择 2」条款已满足。
- 未登记：`doc/migration.md` §8.2「L1-partial 范围与排除项」表格无该行
  （§8.1 亦无）；该表自称升级「已验证」前不得省略的完整清单，缺行会在升级
  验收时漏检此排除项。§8.1 已有同类先例（core-29 有意偏离条目）。

### 去重

无重复立项：eng-60d（precompact+空分区）、eng-60b（segment 预压缩，已落地）、
sto-59（互操作限定 data_append_mode=true）、sto-63a（baseline 口径）均不覆盖
「branch snapshot segment 段差异」的登记与文档同步。

### 残留开发计划（纯文档，无代码/测试改动）

1. `doc/migration.md` §8.2 表格补一行「branch + large value：C++ branch
   manifest 无 segment 段，Rust 为 large value 继承与区间隔离仍写（COMPAT_BASELINE
   排除项）」；或在 §8.1 按先例补「有意偏离」条目（二选一，§8.2 更贴合
   「排除项」语义）。
2. 本文件从 `new/` 移至 `todo/` 后，更新 6 处引用路径
   `issue/new/02-…` → `issue/todo/02-…`：`COMPAT_BASELINE:13`、
   `doc/progress.md:6`（随 `fkv_ledger sync` 镜像）、`fixtures/README.md:157`、
   `doc/architecture/06-gaps.md:47`、`issue/reviewed/01-cpp-oracle-and-fixtures.md:402`、
   `issue/done/eng-91…:139`（历史 DONE 记录可不改，前 4 处必改）。
3. 条件项（保留原验收）：若上游修复 C++ `CreateBranch` 传 segment mapping，
   更新 baseline commit 并重新生成 `fixtures/cpp` / `fixtures/rust`。
4. 测试：无需新增（plain/large 两侧断言均已存在）。

## DONE（2026-11-05 r6）

纯文档收尾，无代码/测试改动，未运行 cargo（构建锁由并行会话持有）。改动文件与内容：

1. `doc/migration.md` §8.2「L1-partial 范围与排除项」表新增「branch + large value」
   一行：C++ `CreateBranch`（82b774c）branch snapshot 不传 segment mapping、
   manifest 无 segment 段，Rust 为 large value 分支继承与 main/branch segment
   file_id 区间隔离仍写——判定「**有意差异，不纳入逐字节等价**」（plain store
   未启用 large value 时两侧 branch snapshot 字节同构）；注明双向后果（Rust 若
   对齐 C++ 省略段，分支重开后父分支 large value 全部不可读；C++ 侧自身该组合
   reopen 断链——`GetSegmentMapper` 返回空、底页 large value 指针不可解析，上游
   `branch_operations` 无该场景用例）与依据（`COMPAT_BASELINE:10-13`、
   `doc/architecture/06-gaps.md:45` 已有登记；甄别写 :47，现文件实际行号 45）；
   跟踪列指向本条归档终位 `issue/done/02-…`。
2. `issue/new/02-…` 引用路径全仓更新为归档终位 `issue/done/02-…`：
   - 甄别所列 4 处：`COMPAT_BASELINE:13`、`doc/progress.md:6`（`fkv_ledger
     report` 镜像行——构建锁未解除无法重跑 report，手改为与再生成输出逐字一致的
     子串替换，report 重读 `COMPAT_BASELINE` 后不会回滚）、`fixtures/README.md:157`、
     `doc/architecture/06-gaps.md:45`。
   - 甄别未列、grep 新发现并一并更正：`doc/architecture/04-btree-and-formats.md:89`
     与 `doc/migration-tooling.md:198`（后者为并行 rev-41 lane 期间新增的引用，
     均把链接文字 `issue/new/02` 与目标同步改）。
   - 38 份 `fixtures/{cpp,rust}/**/meta.txt` 的 `baseline=` 行：`fkv_ledger
     fixture` 把 `COMPAT_BASELINE` 全文压行固化进语料，CI `fixture-diff` 对
     `fixtures/cpp` 做 `git diff --exit-code` 漂移检查，不刷新则必报漂移；因构建锁
     无法重跑 `scripts/gen_fixtures.sh`，以等价子串替换刷新（与重生成输出逐字节
     一致，payload 不受影响；38/38 更新，全仓 `issue/new/02` 除下述历史记录外
     0 残留）。
   - 按甄别不改的历史记录：`issue/reviewed/01-…:402`、`issue/done/eng-91-…:139`。
3. 本文件追加本节后移至 `issue/done/02-cpp-create-branch-omits-segment-mapping.md`，
   上列新引用均指向该终位。

残留（非本条范围，移交）：

- 并行 rev-41 lane 已在 `COMPAT_BASELINE:14-16` 增补「页缓存」排除项，38 份
  `meta.txt` 的 `baseline=` 尚未随其刷新（本条仅同步 02 路径子串）；该 lane 收尾
  重跑 `scripts/gen_fixtures.sh` 时应一并刷平，否则 CI `fixture-diff` 会因该差异
  报漂移。
- 甄别条件项维持原验收：若上游修复 C++ `CreateBranch` 传 segment mapping，更新
  baseline commit 并重新生成 `fixtures/cpp` / `fixtures/rust`。

## 复审（2026-11-05 r7）

结论：**通过**。纯文档登记任务，逐项核对当前工作区（均亲读）：

- `doc/migration.md:263` §8.2 表已有「branch + large value」行：判定「**有意差异，
  不纳入逐字节等价**」+ 双向后果 + 依据（`COMPAT_BASELINE:10-13`）+ 跟踪列指向
  本文件。
- 引用路径 6 处全部指向 `issue/done/02-…`：`COMPAT_BASELINE:13`、
  `doc/progress.md:6`、`fixtures/README.md:157`、`doc/architecture/06-gaps.md:46`
  （DONE 记 :45，行号漂 1 行，链接本身正确）、
  `doc/architecture/04-btree-and-formats.md:89`、`doc/migration-tooling.md:198`。
- fixtures：38/38 `meta.txt` 的 `baseline=` 含 `issue/done/02`，0 份残留
  `issue/new/02`；全仓 `issue/new/02` 仅剩 2 处按甄别保留的历史记录
  （`issue/reviewed/01-…:402`、`issue/done/eng-91-…:139`）。
- 残留项（rev-41 lane 的 38 份 meta.txt 未随 `COMPAT_BASELINE:14-16` 页缓存行
  刷新）边界划分清晰，归该 lane 收尾，不构成本条不通过。
- 本文件移入 `issue/reviewed/` 后，上列 6 处 `issue/done/02-…` 链接需要下一轮
  ledger 同步统一改指 `issue/reviewed/02-…`（本轮硬性约束不允许改这些文件）。
