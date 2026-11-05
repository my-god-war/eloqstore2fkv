# core-22 跨 term 重开缺「allocator 对齐到下一 file boundary」bump，云端读对象 key 用当前 term 而非 manifest 记录的 (branch, term)

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:1259-1263`（`install_replay_state`：`PageMapper::with_tbl(state.mapping_tbl, AllocMode::Append, state.max_fp_id)`，全程不比较 `state.branch_metadata.term` 与当前进程 term）；`crate/engine/fkv_cloud/src/lib.rs:184-192,822`（`file_key` 用**当前** `self.identity()` 拼 `data_<id>_<branch>_<term>`，`read_page` 远端拉取即用该 key）
- C++: `src/replayer.cpp:348-362`（GetMapper：非 Local 模式且 `manifest_term != expect_term` 时 `max_fp_id_ = next_file_id << pages_per_file_shift`，新 term 分配整体跳到新文件边界，避免跨 term 碰撞）；`src/async_io_manager.cpp:710-717`（`ReadPage` 先 `GetBranchNameAndTerm(tbl_id, DataFileKey(file_id), ...)` 按 manifest 恢复的 per-file (branch, term) 寻址，`:1800-1813` 映射来自恢复时 `SetBranchFileMapping`）

## 现状
1. reopen 安装 replay 状态时直接从 `state.max_fp_id` 继续分配，不做 term 不匹配时的文件边界对齐；`open_branch` 同样只算 `branch_file_limit` 上限。
2. 云端读路径的对象 key 一律用当前进程 `(branch, term)` 拼：上一个 term 写的页在新 term 进程里读 → key 404（`ResourceMissing`）。

## C++ 实现
- term 变化 = 新文件边界起步，同一 file id 下新旧 term 数据物理隔离；
- 读寻址按 per-file 的 (branch, term)（恢复期建立），与写入时的 key 严格一致。

## 差异与风险
- 本地模式：同 branch 下 file_ranges 出现交错 term 的非单调区间（C++ debug 构建直接判 Corrupted，`page_manager.cpp:218-267`），当前靠文件名带 term 的候选回退（io-04 已做）缓解，但盘面形态与 C++ 不同；
- 云模式：**旧 term 写入的数据页读不到**（key 错误），且 allocator 不 bump 会把新数据写进与旧 term 相同 file id 的对象 key 空间。

## 对齐建议
- `install_replay_state` / `open_branch` 复刻 term-mismatch bump（非 Local 模式、`manifest_term != process_term && process_term != 0` 时 `max_fp_id` 上跳文件边界）；
- `fkv_cloud` 读路径按 `branch_metadata.file_ranges` 解析 per-file (branch, term) 再拼 key（数据 reopen 时已解析，成本低）。

## 严重度
P1（云路径旧 term 数据不可读；本地为盘面形态分歧）

## 核验记录（2026-11-04）
C++ `replayer.cpp:348-362` bump 逻辑实读确认；Rust `install_replay_state` 无 term 比较实读确认；`fkv_cloud` key 拼法与 read_page 引用自子代理摘录（行号待开发时复核）。

## 审查结论（2026-11-04）

核验成立，按两部分落地。

- 行号复核：`install_replay_state` 实际在 `crate/engine/fkv_store/src/lib.rs:1268` 起（issue 写 1259-1263 属轻微漂移）；`fkv_cloud` 的 `file_key` 在 `src/lib.rs:282` 起，读路径 `read_page` 在 `~:1000`。
- C++ 复核：`src/replayer.cpp:348-362` 的 bump 实读确认（`StoreMode != Local && expect_term != 0 && manifest_term != expect_term`）；`src/storage/page_manager.cpp:269` 把 `replayer.branch_metadata_.file_ranges` 交给 `SetBranchFileMapping`，随后 `ReadPage` 用 `GetBranchNameAndTerm` 取 per-file (branch, term)。差异真实存在。
- 补充事实：Rust 侧 C++ 的 `StoreMode::Local` 永远 `term=0`，所以有效条件等价于 `process_term != 0 && manifest_term != process_term`；Rust 无 StoreMode 抽象，按该等价条件实现。
- 补充事实：本修复同时覆盖「分支读父分支数据」——父分支 file id 归属父 (branch, term)，此前云端同样按当前 branch 拼 key 会 404，属同一缺陷面。
- 需跨 crate 加钩子：Store 是泛型 `I: PageIo`，无法调用 CloudStore 固有方法，故在 `PageIo` 增加默认 no-op 的 `set_file_range_li`（仅 `fkv_io` 一处 5 行），其余后端零改动。范围比「只改两个文件」多一个 trait 定义，属最小可行方案。
- 遗留（不在本 issue 内）：云端 `GetManifest` 的「选最新 term + 提升到 process term」尚未实现（跨 term 云 reopen 会先卡在 manifest 选择），因此本 issue 的 term 场景在当前 Rust 云链路只能靠同 term/跨 branch 复现；bump 与 per-file key 是必要前置。

## 收口（2026-11-05，本会话）

甄别：22a（allocator bump + `next_file_boundary` + `tests/term_bump.rs`）与
22b（`PageIo::set_file_range_li` + cloud per-file key 链 + `tests/term_key.rs`）均已
落地，本文件无剩余开发量，移入 done/。遗留两条已另立
`issue/new/cld-56-cloud-cross-term-manifest-promote-and-remote-listing.md`。

## 复审（2026-11-05 r6）

通过（作为拆分父条目，核心为 mapping/allocator 侧，划入本 lane）。22a：`fkv_store/src/lib.rs:1769-1775` `install_replay_state` 按 `branch_metadata.term != self.term` bump、`:147-148` `next_file_boundary`、`tests/term_bump.rs:15-48`。22b：`fkv_io/src/lib.rs:19` `PageIo::set_file_range_li` 默认 no-op，`fkv_cloud` 实现及 `tests/term_key.rs` 在位（cloud lane 细节由该 lane 复核）。遗留 cld-56 已单独立项。
