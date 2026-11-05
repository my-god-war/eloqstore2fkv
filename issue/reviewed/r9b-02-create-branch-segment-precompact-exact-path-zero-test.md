# r9b-02 P3：create_branch 的 segment 预压缩 exact 路径零直测——eng-60b 复审记录的「遗留测试缺口」悬置三轮未闭环

## 现象（亲读）

代码在位（本轮亲读确认）：

- `crate/engine/fkv_store/src/lib.rs:3592-3597`（`compact_before_branch` 的
  segment 臂：`segment_factor != 0` → `segment_compact_plan` → 
  `compact_segment_file_li`）与 `:3600`（末尾无条件 `file_gc`，对齐 C++
  `Compact()` + `RunFileGc`）。

测试缺口（本轮逐文件核对）：

- data 侧有直测：`tests/branch_store.rs:327`
  `create_branch_precompacts_fragmented_parent`（`with_file_amplify_factor(1)`，
  断言死文件被 GC、branch 全量可读）——但它**未启用大 value**
  （无 `with_large_value`），segment 臂恒不触发。
- segment 侧仅有两条，都不触达 create_branch 路径：
  `tests/segment_compact.rs:36`（`compact_segment_if_needed` 维护路径）、
  `:126`（branch 归档后 GC 保留 branch 引用文件，走 `main.create_branch` 但
  断言对象是 GC 保留、非预压缩搬迁）。
- 即「create_branch 触发 segment 预压缩 + 与 `branch_segment_limit` 区间预留
  交互」这一 exact 组合零测试覆盖。`compact_before_branch` 同时推进 data /
  segment 两侧搬移后再统一 `reserve_until`（`create_branch` :3518-3526 段侧
  预留、:3538-3544 data 侧预留），搬移后的高水位与预留边界相减是否留白、
  branch 重开是否可读，全链无断言。

欠账出处（非本轮新发现，是悬置欠账）：

- `issue/reviewed/eng-60b-segment-precompact-for-branch.md:46-48`：
  「测试缺口（遗留，非阻塞）……『create_branch 触发 segment 预压缩』这一
  exact 路径无直测。记入 r6 lane 报告遗留风险。」
- `issue/reviewed/00-r6-store-write.md:57-60` 遗留风险 1 同文。
- r6（11-05）至今 r7/r8/r9 三轮过去，无 issue 承接、无测试落地。

## C++ 对照

`src/tasks/background_write.cpp:614-655` `CreateBranch` 开头无条件
`Compact()`（data_append_mode 下），`Compact()` 内 `:154-158` 依次
`DoCompactDataFile` / `DoCompactSegmentFile`、`:160` `RunFileGc`——C++ 的
branch 继承稠密 mapping 语义覆盖两侧；上游该路径由 `branch_operations` 测试
族覆盖。Rust 侧 data 臂有对等直测，segment 臂没有：预压缩回归（如计划选错
文件、预留边界吃掉刚搬的段、`update_stat_by_scan` 漏算）只会以
「branch 继承碎片文件」或「segment 区间重叠」的静默劣化形式出现，现有测试
全绿。

## 目标

补一条含大 value 的碎片父表 `create_branch` 直测（对齐
`create_branch_precompacts_fragmented_parent` 的骨架）：

1. `with_large_value` + `with_segment_amplify_factor(非 0)`，先制造碎片
   segment 文件（多轮覆盖写大 value）；
2. `create_branch` 后断言：segment 死文件被回收（`list_segment_file_id_li`
   只剩 live 引用）、branch `open_branch` 后大 value 全量可读；
3. 断言预留区间不与预压缩搬移结果重叠（`branch_segment_limit` 下界 ≥
   搬移后的 `max_segment_fp_id`，可用 `branch_segment_limit()` 访问器）。

## 验收

- `cargo nextest run -p fkv_store` 新用例通过；人为回归（如临时把
  `compact_before_branch` 的 segment 臂短路）时该用例变红；
- `issue/reviewed/eng-60b-*.md:46-48` 的遗留缺口随之闭环，可在其末尾追加
  「已由 r9b-02 补测」的更正记录。

## 核验记录

- 亲读 `fkv_store/src/lib.rs` `compact_before_branch` / `create_branch` /
  `compact_segment_file_li` 全链与两个测试文件全文；
- 亲读 `issue/reviewed/eng-60b-segment-precompact-for-branch.md` 与
  `00-r6-store-write.md` 的遗留记录，确认无后续 issue 承接（`issue/todo/`
  为空、`issue/new/` 仅索引）；
- 查重：r9c-01（GC 测试欠账）、r9e-01（dirty_dir 重试欠账）是各自 scope 的
  独立欠账，与本条（branch segment 预压缩）无重叠；sto-56b 落地的是
  `compact_segment_if_needed` 维护路径及其测试，不含 create_branch exact 路径；
- mtime 备注：`fkv_store/src/lib.rs` mtime 06:29（R1 修复落盘窗口），本轮
  锚点在 06:38 复核行号与内容一致（:3592-3597/:3600/:3518-3526/:3538-3544）。

> 认领（2026-11-05 07:06，本会话子代理开发中）。lane：fkv_store/fkv_task_write 相关 tests。

## DONE（2026-11-05）

补测落地：`crate/engine/fkv_store/tests/branch_store.rs` 新增两条 + 一个辅助函数，
只走公开接口（`put_large` / `create_branch` / `open_branch` / `get_large` /
`list_segment_file_id_li` / `segment_mapper` / `branch_segment_limit`）：

1. `create_branch_precompacts_fragmented_segment_parent`（exact 路径）：
   `SegmentLayout::new(4096, 2)` + `with_large_value(…, 1024)` +
   `with_segment_amplify_factor(1)`，常驻 key + 13 轮覆盖写 9KB 大 value，
   制造「fork 点在 segment 中部」的碎片父表（含全死文件，live 段散布在尾文件
   之前）。`create_branch` 后断言：
   - 非尾文件的 live 段全部被搬到旧尾文件之后（预压缩搬移确实发生）；
   - 所有 live 段文件 < branch 预留块下界（搬移结果不与预留区间重叠）；
   - 磁盘 segment 文件集合 == 父 mapping 实际引用集合（死文件被末尾
     `file_gc` 回收），且文件数严格减少；
   - 父 segment 分配器 `max_fp_id` 精确等于预留上界
     `(2*BRANCH_FILE_RESERVE_FILE_NUM) << shift`（整区间跳过）；
   - `open_branch` 后 `branch_segment_limit() == Some(预留上界)`、branch
     segment 分配器从预留块下界精确起步；branch 写新大 value 落在预留块首
     文件、`max_fp_id` 留在 `[base, limit)` 区间内；keep / big / extra 全量
     可读，branch 重启后一致。
2. `create_branch_without_segment_factor_keeps_fragmented_segment_files`
   （factor=0 对照）：同场景不设放大率，断言磁盘文件集合逐字节不变、live
   段位置不变（低号文件 live 段保留原位），branch 数据仍可读——与上一条
   互为镜像，锁死「预压缩由 segment 臂触发」这一因果。
3. `live_segment_file_li`：从 segment mapper 提取 live 段文件 id（升序去重）。

回归验证：临时给 `compact_before_branch` 的 segment 臂加 env 短路（保留末尾
`file_gc`，模拟「搬移没发生」的最恶劣回归）后重跑，
`create_branch_precompacts_fragmented_segment_parent` 如期变红，失败点正是
「非尾文件的 live 段必须被搬到文件尾 live=[0, 9, 10] tail=10」——死文件会被
GC 顺手回收，单靠文件数断言抓不住该回归，搬移位置断言才是判别器；还原后全绿。
产品代码未发现与 eng-60b 声称不符之处，`src` 零改动。

验收：
- `cargo nextest run -p fkv_store -p fkv_task_write -p fkv_shard --no-fail-fast`：
  285 passed / 0 failed（fkv_shard 首跑被 lane 外 fkv_cloud 的重复
  `SegmentState` 定义挡住，属另一 lane 在途工作，等待其自行修复后复跑通过）；
- `cargo clippy -p fkv_store -p fkv_task_write -p fkv_shard --no-deps
  --all-features --all-targets -- -D warnings`：零警告（修掉一处
  `int_plus_one`）。

备注：目标一节提到的「在 eng-60b 文件末尾追加更正记录」受 lane 纪律约束
（只许动本 issue 文件）未执行，留给有 eng-60b 写权限的轮次补一笔。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 亲读在盘代码与测试：

- 产品代码（`crate/engine/fkv_store/src/lib.rs`，mtime 07:49 属并行会话在途区，以内容
  锚点为准）：`compact_before_branch`（:3640）、segment 臂 `segment_compact_plan` →
  `compact_segment_file_li`（:3672-3674）、末尾无条件 `file_gc`（:3679）——claims 成立。
- exact 路径直测 `create_branch_precompacts_fragmented_segment_parent`
  （`tests/branch_store.rs:388`）亲读关键断言：前置碎片条件（:431-446，含全死文件与
  低号 live 段）、搬移后 live 段全在旧尾文件之后且不入预留区间（:464-473）、磁盘文件
  集合 == live 集合（:480-483）、父分配器跳到预留上界 `(2*reserve)<<shift`（:485-489）、
  `open_branch` 后 `branch_segment_limit()==Some(limit_fp)` 且分配器从下界起步
  （:504-509）、branch 新段落预留块首文件且 `max_fp_id∈[base,limit)`（:534-546）、重启
  一致——均为实质断言，非空跑。
- 对照测试 `create_branch_without_segment_factor_keeps_fragmented_segment_files`
  （:591）亲读：factor=0 时磁盘文件集合逐字节不变、live 段位置不变（:636-658），与上条
  互为镜像，锁死「预压缩由 segment 臂触发」因果；helper `live_segment_file_li`（:675）。
- 遗留闭环：eng-60b 文件末尾的「已由 r9b-02 补测」更正由本轮补记（见该文件
  `## 更正（2026-11-05 r9b-02）`）。DONE 自述「src 零改动」未能从 git 复核
  （`fkv_store/src/lib.rs` 被并行会话改在途），但新增测试只走公开接口，与声称相符。
