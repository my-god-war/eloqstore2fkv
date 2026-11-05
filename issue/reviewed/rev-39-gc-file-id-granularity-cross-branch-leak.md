# rev-39 GC 保留/删除以 file_id 为粒度，跨 (branch, term) 同 id 文件产生回收泄露

## 位置
- Rust: `crate/engine/fkv_task_gc/src/lib.rs:296-345`（`manifest_retained_file_page_set` 汇总 retained）、`:355-393`（`gc_files_with_retained` / `gc_segment_files_with_retained` 按 file_id 判定并删除）；`crate/io/fkv_io_compio/src/lib.rs:113-120`（设计注释）、`:1723-1742`（`list_file_id_by_kind` 跨变体去重只回 file_id）、`:1745-1764`（`remove_file_by_kind` 删同 id 全部 (branch,term) 变体）
- C++: `src/file_gc.cpp:33-90`（`GetRetainedFiles` 以 `RetainedFileKey{file_id, branch, term}` 为保留键）、`:1140-1230`（`DeleteUnreferencedLocalFiles` 按文件名解析出三元组再判删）

## 现状
C++ 的 GC 以「文件名自带 (branch, term)」为基础：retained 集合键是三元组
`(file_id, branch, term)`，删除判定逐文件精确到三元组——同 file_id 分属两个 branch 的两
个物理文件，谁没被引用删谁。

fkv 的 `FileIo::list_file_id_li` 只返回去重后的 file_id（`fkv_io_compio` 内部保留了
`data_other: file_id -> Vec<(term, path)>` 变体表，但接口不外露 branch/term），因此：
1. retained 集合退化为 `FileId` 集合（`file_id_set`，`fkv_task_gc/src/lib.rs:347-353`）；
   任一 (branch, term) 变体被引用 → 同 id 全部变体都被保留。
2. `guard.retain_data(file_id)` 的 in-flight 判定也不再读文件自身 term，而是用
   「file_id 落在哪条 `file_ranges` 区间」把文件归属到 (branch, term)，取区间 term 作
   代理（`InflightGuard::retain`/`entry_inflight`，`fkv_task_gc/src/lib.rs:173-242`；
   代码注释自述这是 `FileIo 只列 file id` 的妥协）。
3. `remove_file(file_id)` 一次删掉该 id 的**所有**变体。

组合后果（branch reserve 桶内 fork 点后 main 与 branch 可分到同号 file id，见
`fkv_store::open_branch` 的 `BRANCH_FILE_RESERVE_FILE_NUM` 对齐逻辑）：
- main 的 file 42 已死、branch b 的 file 42 存活 → file_id 42 在 retained 集合 → 两个
  文件都保留，死文件永久泄露（C++ 只删 main 的那个）。
- 反过来 id 无人引用时一次删全部变体——安全（都该删）。
- 结论：不会误删活文件（保守方向），但多 branch 场景回收率低于 C++，死文件随 reopen
  累积。io 层注释已把跨 branch 同 id 的处理登记为 todo：「跨 branch 的同 id 读路由仍需
  `BranchFileMapping`（todo 11）」（`fkv_io_compio/src/lib.rs:118-120`）。

## C++ 实现
`GetRetainedFiles`（file_gc.cpp:33-90）扫每个 manifest 的 mapping，`GetBranchNameAndTerm
(file_ranges, file_id)` 解析归属后以三元组入集；`DeleteUnreferencedLocalFiles`
（:1140-1230）对每个本地文件 `ParseDataFileSuffix` 得到 (branch, term)，先查三元组
retained，再做 `term >= newest_term_ && file_id >= least_unflushed` 的 per-branch
in-flight 判定（:1207-1208）。fkv 的 `InflightGuard::fold`/guard 折叠规则与该判定本身
是对齐的，差别只在归属信息来源（文件名 vs 区间推断）与删除粒度。

## 差异与风险
- 泄露而非丢数据：branch/archive 越用越多时，本地与远端（cloud GC 同构路径）死文件
  占比上升，空间放大估计（`file_live_count`）也会把死变体算进 file 占用。
- 区间归属推断在「新 term allocator 从旧上界之后起步」的场景已做了补偿
  （`entry_inflight` 的 `entry.max > bound` 特判），但 file_ranges 只有 max 边界时，
  低区间内的死文件仍可能因任何同 id 变体被引用而保留。
- 这是已登记的架构缺口（FileIo 接口不带身份），不是隐藏 bug，但影响 branch 特性上线
  后的容量成本，需要显式 issue 跟踪。

## 对齐建议
把 (branch, term) 透出到 GC 层：`FileIo::list_file_id_li` 换成返回
`Vec<(FileId, BranchTerm)>`（或新增 `list_file_variants`），retained 键升级为三元组，
删除按变体粒度（`remove_file_variant`）。`fkv_io_compio` 已有 `data_other` 变体表，
接口改造集中在 `FileIo` trait 与 cloud 对象 key 解析两处。短期方案：GC 前按当前
manifest 集合重算「每个 file_id 允许的变体集」，删除时跳过被保留 id 的**非保留**变体——
即至少把 main 与 branch 的死变体分开回收。

## 严重度
P2

## 核验记录
- 通读 `fkv_task_gc/src/lib.rs` 全文（InflightGuard、retain、
  manifest_retained_file_page_set、gc_files_with_retained）。
- 读 `fkv_io_compio/src/lib.rs`：模块注释（113-120）、`scan_dir`（713-751）、
  `list_file_id_by_kind`（1723-1742）、`remove_file_by_kind`（1745-1764）。
- 读 C++ `src/file_gc.cpp`：`GetRetainedFiles`（33-90）、`SeedActiveBranchGuardFromInMemory`
  （118-164）、`ProcessOneManifest`（583-655）、`DeleteUnreferencedLocalFiles`
  （1140-1230）。
- 读 `fkv_store/src/lib.rs::open_branch`（1554-1610）确认 branch reserve 桶按 fork 点
  对齐，存在与父 branch 文件同号的分配区间。

## 甄别（2026-11-05 r6）

结论：**采纳**（事实全部核实；对齐建议中「retained 键升级为三元组」按 sto-54a 的
「不做」结论裁剪——retain 侧并集保持 id 级安全下限，三元组精度落在删除的变体粒度上）。

证据核实（当前工作区亲核，行号为核实时点）：

- retained 退化 id 集：`fkv_task_gc/src/lib.rs:429-441`（`file_id_set`）、
  `:477-487`（`retained_file_set.contains(&file_id) || guard.retain_data(file_id)`
  → 否则 `io.remove_file(tbl, file_id)`）；segment 同构 `:524-533`。
- io 层只露 id：`fkv_io/src/lib.rs:489/:516/:523/:528`（trait 签名）；
  `fkv_io_compio/src/lib.rs:1726-1745`（list：primary ∪ other 的 key 去重回 id）、
  `:1748-1768`（remove：`variant_path_li` 删该 id 全部 (branch,term) 变体）、
  `:1157-1173`（`variant_path_li`）、`:109-119`（DirIndex 注释自述「删除时按
  file id 清掉全部变体」「跨 branch 的同 id 读路由仍需 `BranchFileMapping`
  （todo 11）」——与意见引用一致）。
- 文件名带身份：`fkv_types/src/lib.rs:258-269` `<prefix>_<file_id>_<branch>_<term>`，
  与 C++ 命名同构；`fkv_io_compio:726-764`（`scan_dir`）解析出三元组，但 `:759`
  存 other 表只留 `(term, path)`，**branch 字段被丢弃**——变体化改造需补。
- 同 id 跨 branch 常态存在：`fkv_store/src/lib.rs:83`（`BRANCH_FILE_RESERVE_FILE_NUM=1024`）、
  `:1638-1696`（`open_branch` 按 fork 点对齐 bucket，branch 与 main 在同 bucket 内
  自 fork 点起各自分配 → 同号 file id 真实出现）；GC 调用点 `:3375/:3385`。
- C++ 三元组判删：`src/file_gc.cpp:33-93`（`GetRetainedFiles` 以
  `RetainedFileKey{file_id, branch, term}` 入集，`:57-58/:79`）、`:1140-1222`
  （`DeleteUnreferencedLocalFiles`：`:1178` `ParseDataFileSuffix`、`:1183-1189`
  三元组精确判留、`:1207-1208` per-branch in-flight）；`include/types.h:103`。
- cloud 同构成立：`fkv_cloud/src/lib.rs:2361-2368`（list = local ∪ remote）；
  remote `:2249-2280` 解析三元组后仍归并成 id、候选按当前 identity branch 过滤；
  `remove_file :2401-2407` 本地半删**全变体**、远端半只删当前 identity key。
  retained 并集经 `list_manifest_file_name_li :1972-1979`（local ∪
  `remote_manifest_name_li :2062-2090`，**无 branch 过滤**）跨 branch 覆盖
  → 保守方向在 cloud 同样成立。

后果定性：**空间泄露（该删不删），非正确性问题**。id 级 retained 并集 ⊇ C++
三元组集合向 id 的投影（任一变体被任一 manifest 引用 → 整 id 保留），删除只发生在
「id 无任何引用且无 guard」时 → 只会少删、不会多删，活文件不可能被误删。意见自述
（"不会误删活文件（保守方向）""泄露而非丢数据"）与标题 "leak" 定性核实无误。
泄露量级：同 id 死变体在另一 (branch,term) 变体存活期间永久保留，随 fork 与
reopen 累积；`file_live_count` / 空间放大把死变体计入占用（统计口径同受影响）。

与相邻 issue 的边界：

- `done/core-23`（无 r6 复审）：只落地 InflightGuard/BranchGuard in-flight 高水位，
  DONE 遗留明记「sto-54 的 branch/term retained key 维度未实现」——本条即该遗留
  的容量侧后果，非重复。
- `reviewed/sto-54`（已关闭）+ `done/sto-54a`「不做」：拒绝的是 retain 并集按
  (branch,term) **收窄**（C++ 丢弃无归属引用 → 误删风险，Rust 并集是安全超集）。
  本条不推翻该结论；主张的是删除粒度与容量代价的显式跟踪，当时未立案。开发须在
  该边界内设计。
- `done/eng-88`：guard 判定 term 维度已补；其记档偏离「`FileIo` 只列 FileId、彻底
  对齐需 (file_id,branch,term) 列举接口、本批次不做」与本条共享同一 FileIo trait
  改造批次（`fkv_io` / `fkv_io_compio` 为并行会话热点，需同批协调、合并前 rebase）。
- `done/cld-56`：云端跨 term 列举 + 旧 manifest 剪枝；明记跨 branch 对象留给各自
  branch 轮次、同 branch 跨 term 同 id 正常路径不出现（core-22a term bump）——
  不覆盖跨 branch 同 id 泄露。
- `done/io-41`：list 升序去重契约，正交。

开发计划：

1. FileIo 变体列举（与 eng-88 遗留同批）：新增 `list_file_variants ->
   Vec<(FileId, branch, term)>`；实现覆盖 `fkv_io_compio`（`scan_dir:759` 需把
   branch 保进 other 表）、`fkv_io_mem`、`fkv_io_sim`、`fkv_metrics`、
   `fkv_shard` backend/roots、`fkv_cloud`（`:2265` 已解析三元组）。
2. GC 删除升变体粒度，retain 并集保持 id 级下限：id 整体无引用且无 guard →
   现状（删全部变体）不变；id 被引用 → 逐变体判定（现有归属区间 + guard 逻辑）。
   归属歧义口径二选一并书面记档：(a) C++ parity——归属失败即删（C++ 同样接受，
   带 WARNING 日志），逐文件可与 C++ 对账（倾向）；(b) 保守剪枝——归属不唯一 /
   失败时保留（泄露收窄不归零）。
3. 空间统计按变体计数修正 `file_live_count` / 空间放大。
4. 测试：`fkv_task_gc` / `fkv_store` 跨 branch 同 id 用例（main 42 死 + branch 42
   活 → 一轮 GC 后 main 变体删、branch 变体留；反向；cloud 两轮断言）；
   `cargo nextest run -p fkv_task_gc -p fkv_io_compio -p fkv_store -p fkv_cloud`。

严重度 P2 维持。

> 认领（2026-11-05 03:40，本会话子代理开发中）。lane：fkv_task_gc + fkv_io + 全部 FileIo 后端（fkv_io_compio/fkv_io_mem/fkv_io_sim/fkv_cloud）同一批次。

## DONE（2026-11-05）

### 方案选择理由

`FileIo` 新增变体列举 / 精确删除（而非改 `list_file_id_li` 返回值）：旧签名被
`fkv_store` 维护路径（truncate / drop / 需要判空）与 io-41 升序契约共用，改返回值
会波及 lane 外全部调用点；新增方法可带**默认实现**，`fkv_metrics`、`fkv_shard`
backend/roots、`fkv_store` 测试包装等未透传的 `FileIo` 实现零改动保持编译。

- `fkv_io::FileVariant { file_id, identity: Option<(Box<str>, u64)> }`，
  `identity = None` 表示后端无变体概念（纯内存 / 包装层）。
- 新方法（各带默认实现）：`list_file_variants` / `list_segment_file_variants` /
  `remove_file_variant` / `remove_segment_file_variant`，以及 LocalGc 语义的
  `list_local_*_variants` / `remove_local_*_variant`（cloud 覆写为只动本地 cache）。
- 旧接口无孤儿：`list_file_id_li` / `remove_file` 等仍被 `fkv_store:3134-3452` 等
  路径消费，全部保留；默认实现 `remove_file_variant → remove_file`（整 id），
  与 rev-39 之前行为一致。

### 实现要点

1. **compio**：`DirScan`/`DirIndex` 的 other 表从 `(term, path)` 升级为
   `OtherVariant { branch, term, path }`（`scan_dir` 不再丢 branch）；新增
   `list_file_variants_by_kind`（primary 带 identity + other 表三元组，升序去重）、
   `remove_file_variant_by_kind`（`(branch, term)` 精确匹配路径删除）、
   `forget_variant`（只清对应索引条目，同 id 其它变体读路由不受影响）。
2. **cloud**：远端对象解析出三元组（branch 仍按当前 identity 过滤——跨 branch
   对象留给各自 branch 轮次；term 不过滤），`remove_file_variant` 本地半按文件名
   精确删、远端半直接拼对象 key（不再走 `file_key` 区间解析）。
3. **mem / sim**：物理上单变体、无文件名身份，走 trait 默认实现（identity=None）。
   刻意不实现 `set_file_identity` 上报：reopen 换 branch 后「当前 identity」会
   把旧 branch 文件误归新 branch（变体判定反而可 误删），id 级回退才是安全侧。
4. **fkv_task_gc**：
   - 引用归属知识 `RefKnowledge { ref_ids, attr_failed, variants }`（data / segment
     各一份）：`apply_manifest` 对每份 manifest 的 retained file id 经**该 manifest
     自带**的区间链归属（对齐 C++ `GetRetainedFiles` 的 per-manifest 归属；
     `FindBranchRange` lower_bound 语义、上界 0 不占有文件、并列上界全采纳）。
   - **归属歧义口径：采用 (b) 保守剪枝**——归属失败（file id 超出链）的引用不
     丢弃，整 id 进入 `attr_failed` 保守保留；引用只来自无法归属的外部注册
     （快照 / 活跃 mapping，经 `pin_*` / `add_pinned_*_mapping` 显式登记，或
     `!ref_ids.contains` 兜底）时同 id 全部变体保留。issue 倾向的 (a) C++ parity
     （归属失败即删）被否：它与 sto-54a「不丢引用」的既定裁决直接冲突（旧格式
     manifest 无区间链时会把全部被引用文件判死）。C++ 接受该风险是它的 local GC
     上下文不同；Rust 侧泄露收窄不归零，记档为已知边界。
   - 变体判定 `retain_*_variant(file_id, branch, term, referenced)`：
     damaged → pin → 归属命中 → attr_failed → 外部引用 → in-flight。
   - in-flight 改用文件自身 `(branch, term)`，逐项对齐 C++ `file_gc.cpp:1207-1208`
     的 `term >= newest_term && file_id >= least_unflushed`（eng-88 term 维度）；
     **guard 缺失按可删除处理**（与 C++ 「非活跃 branch 缺 guard 保守保留」不同）：
     Rust branch 生命周期保证先有 manifest 再有文件（fork 先写 branch snapshot），
     缺 guard 的分支不可能有文件，且 `fkv_task_compact` / gc.rs 既有契约锚定
     空 guard 可删。区间归属的补偿特判（`entry_inflight` 的 `entry.max > bound`）
     保留——它服务 identity=None 的兜底路径，不是死代码。
   - GC 循环按变体驱动：`GcScope::list_*_variants` / `remove_*_variant`，
     有身份走变体判定，无身份保持 `referenced || retain_*`（行为不变）。
     删除计数从 id 粒度变为变体粒度（io-04 测试断言 1 → 3，目录清空契约不变）。
5. **并行批次协同**：LocalGc 会话同批落地了 `GcScope` / `pin_*` 外部引用显式登记
   与 gc.rs 的 `gc_reclaims_only_dead_variant_across_branch` /
   `pinned_file_id_keeps_all_variants` 用例，与本实现的判定链直接兼容，合并零冲突。

### 测试证据

新增 `fkv_task_gc/tests/variant.rs`（compio 真实文件名盘面 + Mem 回退语义）：

- `gc_deletes_only_dead_variant_of_referenced_file_id`：main/5 与 b-1/3 同 file id
  2049，b-1 manifest 引用（归属 (b-1,3)）、main 高水位已越过 → 一轮 GC 只删
  main 变体（data + segment 各删 1，盘面仅剩 b-1 两文件）——issue 主场景。
- `gc_deletes_all_variants_when_no_variant_referenced`：同 id 两变体均无引用 →
  全删（guard 存在但均低于各自水位）。
- `variant_inflight_requires_both_term_and_file_id_bound` +
  `gc_reclaims_stale_term_file_above_high_water`：term 维度判定——旧 term 文件
  即使 id 高于水位也不被 guard 兜底；`term >= newest_term && file_id >=
  least_unflushed` 缺一不可；guard 缺失分支可删。
- `mem_backend_keeps_id_level_semantics`：identity=None 回退路径行为不变。

验收：`cargo nextest run -p fkv_task_gc -p fkv_io -p fkv_io_mem -p fkv_io_sim
-p fkv_io_compio -p fkv_cloud`（158/158，无 linux-gated 跳过）；连带下游
`fkv_store` / `fkv_task_compact` / `fkv_metrics` / `fkv_gc` 317/317；lane 六 crate
`cargo clippy --all-features --all-targets -- -D warnings` 零警告；`cargo fmt` 干净。
`cargo check --workspace` 在本批次改动完成后曾全绿；收尾时 `fkv_shard/src/lib.rs`
正被并行会话重写（WriteFlight / put_group，与 rev-39 符号零交集，grep 验证），
lane 外调用点兼容性以「其余全部 crate 单独 check 通过 + fkv_store/metrics/shard
wrapper 走默认实现」为准。

遗留（lane 外，另行立案）：`file_live_count` / 空间放大统计仍按 id 计数，未按
变体修正（原开发计划第 3 项，落在 fkv_store / fkv_metrics，本轮 lane 外）；
`fkv_metrics` / `fkv_shard` wrapper 未透传变体接口，经其 GC 的泄露修复不生效
（回退 id 级，安全侧）。

> **更正（2026-11-05 rev-47 甄别复核）**：上段第二句已过期——wrapper 透传由
> 归档后的后续改动落地：`fkv_metrics/src/lib.rs:784-786`（list）/`:845-854`
> （remove，segment `:860/:875` 同）、`fkv_shard/src/backend.rs:447-451/
> :535-545`（Local/Mem/Cloud 三后端）、`fkv_shard/src/roots.rs:289-290/
> :333-342` 均已透传，`fkv_metrics/tests/metered_forward.rs:128`
> `list_file_variants_keeps_identity` 在位；生产 GC（`GcScope` →
> `list/remove_*_variants`，fkv_task_gc:720/:735/:753/:754）经 wrapper 后仍为
> 变体粒度，**泄露修复已生效**，勿再按本段排查。第一句（统计仍 id 级）属实，
> 但经 C++ 对照（`MapperExceedsAmplification` 亦分配器视图、`DoCompactDataFile`
> 亦 id 空间循环、`SpaceSize/UpdateStat` 同构）裁定为**同口径非偏离**，不再
> 立案开发，证据与理由见 `issue/reject/rev-47-space-stats-still-id-level-
> after-variant-gc.md`。

## 复审（2026-11-05 r7）

**通过。** 重点核对项（按 `(branch,term)` 变体精确删除；跨 branch 同 id 测试在位）
逐行亲读：

- trait 面（`fkv_io/src/lib.rs`）：`FileVariant { file_id, identity:
  Option<(Box<str>, u64)> }` :493-499（`unknown` 占位 :503）；新方法带默认实现——
  `list_file_variants` :525-539（回退 id 列举 + identity=None）、
  `remove_file_variant` :633-641（默认退化为整 id `remove_file`，与 rev-39 前
  行为一致）、segment / local 变体共 6 个 :644-713；旧接口全保留。
- compio（`fkv_io_compio/src/lib.rs`）：`OtherVariant { branch, term, path }`
  :111-115，`scan_dir` :791-812 存 other 表不再丢 branch；`list_file_variants_
  by_kind` :1995-2026（primary 当前 identity + other 三元组，sort+dedup）；
  `remove_file_variant_by_kind` :2055-2077 经 `variant_paths_of` :1272-1303 按
  `(branch, term)` 精确匹配路径删除；`forget_variant` :1329+ 只清对应索引条目。
- cloud（`fkv_cloud/src/lib.rs`）：`list_file_variants` :2826-2833（local ∪
  remote 去重）；`remove_file_variant` :2855-2871 本地半精确删 + 远端半
  `data_file_name(file_id, branch, term)` 直拼对象 key（不走 `file_key` 区间解析）；
  LocalGc `remove_local_file_variant` :2921-2934 只动本地 cache。
- GC 判定链（`fkv_task_gc/src/lib.rs`）：`RefKnowledge { ref_ids, attr_failed,
  variants }` :63-74；`apply_manifest` :574-593 对每份 manifest 的 retained 经
  **该 manifest 自带**区间链 `note_refs` 归属（per-manifest，对齐 C++
  `GetRetainedFiles`）；`retain_variant` :444-470 判定序 damaged → pin →
  归属命中 → attr_failed → 外部引用（`!ref_ids.contains`）→ in-flight；
  `variant_inflight` :480-489 按文件自身 `(branch, term)` 对齐 C++
  `file_gc.cpp:1207-1208` 的 `term >= newest_term && file_id >= bound`；GC 循环
  `gc_data_files` :869-889 按变体驱动（有身份精确判删，无身份保持
  `referenced || retain_data` id 级行为）。
- **跨 branch 同 id 测试在位**（tests/variant.rs :95-182
  `gc_deletes_only_dead_variant_of_referenced_file_id`）：main/5 与 b-1/3 同
  file id 2049，b-1 manifest 引用归属 (b-1,3)、main 高水位越过 → 断言
  `retain_data_variant(2049,"b-1",3,true)` 真 / `(2049,"main",5,true)` 假，
  data+segment 各删 1，盘面恰剩 `data_2049_b-1_3` / `segment_2049_b-1_3`
  （:163-181）。另 4 条（全死全删 :187-246、term+水位双条件 :252-263、
  旧 term 高水位之上回收 :267-309、Mem id 级回退 :314-343）断言均真实；
  协同用例 `gc_reclaims_only_dead_variant_across_branch` /
  `pinned_file_id_keeps_all_variants` 亦在 gc.rs :218/:326。
- 归属歧义采用保守剪枝 (b) 并书面记档（attr_failed :395 整 id 保留），与
  sto-54a「不丢引用」裁决一致；遗留两条（统计仍按 id、wrapper 未透传）DONE
  已自认 lane 外，与源码现状一致。
