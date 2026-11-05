# rev-35 kill point / fail point 缺 OpenOrCreateFD:CreateFile 与 SyncFiles 两个注入点

## 位置
- Rust: `crate/io/fkv_io_compio/src/lib.rs`（failpoint 接线点集合，无 CreateFile / SyncFiles 同名点）；`crate/foundation/fkv_failpoint/src/lib.rs`（机制本身完备）
- C++: `src/async_io_manager.cpp:1751`（`TEST_KILL_POINT_WEIGHT("OpenOrCreateFD:CreateFile", 100)`）、`src/async_io_manager.cpp:5818`（`TEST_FAIL_POINT_RETURN("SyncFiles", KvError::Corrupted)`，`CloudStoreMgr::SyncFiles` 入口）

## 现状
fkv 侧实际接线的具名注入点（grep 全仓 `fail_point!(`/`kill_point("` 去重）：
`WritePage`、`WriteSegments`、`AppendManifest:Write`、`AppendManifest:Sync`、
`AtomicWriteFile:Sync`、`AtomicWriteFile:Rename`、`AtomicWriteFile:SyncDir`、`SyncData`。

与 C++ 对照缺两个：

1. **`OpenOrCreateFD:CreateFile`（kill point）**：C++ 在「open O_EXCL 失败(ENOENT) →
   决定 CreateFile」与 `CreateFile(...)` 实际创建之间注入（权重 100），专门测「文件将建
   未建」的崩溃窗口。fkv 的文件创建发生在 `open_write_file`（`write_page` /
   `write_page_li` / `write_segment` 内部 `ensure_dir` + open with create），
   `WritePage` kill 点在 **ensure_dir 之前**——进程被杀时新文件尚未出现，覆盖不到
   「文件已创建、页数据未写 / manifest 未指向」的 0 字节残留窗口（该残留恰是 GC / reopen
   容错必须演练的状态）。
2. **`SyncFiles`（fail point）**：C++ 用它让写任务在「文件已分配 + branch 高水位已推进
   之后、flush 同步步骤」以错误返回（不杀进程），专门演练 `WriteTask::Abort` 回滚
   `BranchFileMapping` 高水位（见 `fail_point.h:6-13` 注释）。fkv 对应回滚路径存在
   （`Store::put_at → settle_mapper_batch → mapper.abort_batch`，`refresh_branch_metadata`
   的高水位派生随 mapper 回滚），且 local 路径可用 `SyncData` / `AppendManifest:*` 在
   manifest 提交点注入；但 **cloud 的远端 flush 步骤（`CloudStore::flush_all_tbl` /
   `put_manifest_with_retry` 在途）没有任何具名 fail point**，cloud 写中途失败回滚
   无法用注入点演练。

## C++ 实现
`kill_point.cpp`：`kill_odds_` 非 0 时按概率 `SIGTERM` 自杀；`fail_point.h`：`ArmOnce`
后一次 `TEST_FAIL_POINT_RETURN` 返回指定错误。两者都编译期隔离（NDEBUG）。注入点清单
（基线 82b774c）：`WritePage`(1000) / `WriteSegments`(100) /
`OpenOrCreateFD:CreateFile`(100) / `AppendManifest:Write`(10) / `AppendManifest:Sync`(10)
/ `AtomicWriteFile:Sync` / `AtomicWriteFile:Rename` / `AtomicWriteFile:SyncDir`
+ fail point `SyncFiles`。

## 差异与风险
- 崩溃一致性测试的前提是注入点能落在正确位置。缺 `OpenOrCreateFD:CreateFile` 意味着
  「新文件半创建」这一类盘面状态无法在 fkv 的 crash-recovery 测试中系统生成，GC 对
  0 字节文件的处置、reopen 对半文件的容忍等路径得不到故障注入覆盖（只能靠手工构造）。
- 缺 cloud flush 的 fail point 意味着「远端上传部分成功后写中止」的回滚正确性
  （branch 高水位 / resident 表 / dirty 标记）没有测试缝，而这段恰是 cloud 模式最难
  自然复现的失败面。
- fkv 用 `abort()` 而 C++ 用 `SIGTERM`：对测试等价（进程即死），无需改动。

## 对齐建议
- 在 `fkv_io_compio` 的文件创建路径（`open_write_file` 返回后、首次 `write_at` 之前，
  或按 kind 在 create 决策处）加 `kill_point("OpenOrCreateFD:CreateFile")` 对齐 C++ 语义。
- 在 `CloudStore::flush_all_tbl`（或 `put_manifest_with_retry` 入口）加
  `fail_point!("SyncFiles")`，让写任务能在「文件已分配、manifest 未提交」边界以错误
  返回，配套一条 abort-rollback 回归测试（对齐 C++ `fail_point.h` 登记的用途）。

## 严重度
P2

## 核验记录
- `grep -rh 'fail_point!(\|kill_point("' crate/ --include='*.rs'` 去重得 8 个具名点；确认
  `OpenOrCreateFD` / `SyncFiles` 不存在（后者全仓无匹配）。
- 读 `fkv_io_compio/src/lib.rs`：`write_page`（1983-2009，WritePage kill 在 ensure_dir
  之前）、`write_page_li`、`write_segment`、`append_file_at`（1626-1643）、
  `atomic_write_file`（1646-1661）接线位置。
- 读 C++ `async_io_manager.cpp:1735-1755`（CreateFile 注入点上下文）、`5813-5820`
  （SyncFiles 注入点上下文）、`kill_point.cpp`、`fail_point.h` 全文。
- 读 `fkv_store/src/lib.rs` `put_at`/`settle_mapper_batch`（1262-1305）确认回滚路径存在，
  问题仅是 cloud flush 步骤无具名注入点。

## 甄别结论（2026-11-05）：拒绝

意见的事实核对全部成立，但价值判断不成立：两个缺口各有等效覆盖或无一致性影响，
逐一列出。

### 事实复核（均属实）
- 8 个具名点清单准确（grep 全仓仅 `fkv_io_compio/src/lib.rs` 接线：AppendManifest:Write/Sync
  :1636-1653、AtomicWriteFile:Sync/Rename/SyncDir :1664-1672、WritePage :2010/2040、
  SyncData :2115、WriteSegments :2379）；`OpenOrCreateFD` / `SyncFiles` 全仓无匹配。
- `WritePage` kill 确在 `ensure_dir`/`open_write_file` 之前（write_page :2010-2014、
  write_page_li :2040-2085）；`fkv_cloud` 全 crate 无任何 failpoint（flush_all_tbl :1628、
  put_manifest_with_retry :1269 均裸奔）。
- C++ 两点位置属实：`async_io_manager.cpp:1751` kill 在 `CreateFile` **之前**（open ENOENT
  分支）；`:5818` fail 在 `CloudStoreMgr::SyncFiles` 入口；`fail_point.h:3-6` 注释确认其
  用途（WriteTask::Abort 回滚高水位）。另一关键事实：C++ `WritePage` kill（:982）在
  `OpenOrCreateFD(create=true)` **之后**——C++ 的「0 字节新建文件残留」由 WritePage kill
  覆盖，`OpenOrCreateFD:CreateFile` 只覆盖「文件未创建」窗口。

### 逐点等效覆盖分析
1. **OpenOrCreateFD:CreateFile（kill）——无增量价值**：
   - 「文件未创建」窗口：fkv `WritePage` kill（ensure_dir 前）效果与 C++
     `OpenOrCreateFD:CreateFile` 等同（杀时文件均不存在），已覆盖。
   - 「文件已创建、0 字节、manifest 未指向」窗口确实无法用现有 kill 点生成，但该盘面
     对 fkv 崩溃一致性**无影响**：manifest 是唯一 durability anchor（eng-89 亦确认），
     reopen 按 manifest 加载、不读未引用文件；该文件仅由 GC 当孤儿清理（GC 修复另见
     todo/rev-39）。torn-tail 容错已由 sto-51/sto-51a（reviewed）覆盖读路径。
   - 若真要补，正确做法是把 fkv `WritePage` kill 挪到 `open_write_file` 之后对齐 C++
     WritePage kill 的位置；意见建议「open_write_file 返回后加 kill_point(
     "OpenOrCreateFD:CreateFile")」与 C++ 该点名语义（CreateFile 之前、文件未建）恰好
     相反，会造成点名-语义错位。
   - 消费面：fkv 全部 5 个 kill 点接线均无业务测试消费（全仓唯一 `!kill` 用法是
     fkv_failpoint 机制自测 `tests/failpoint.rs:22`；现有 failpoint 测试 batch_fail.rs /
     maintenance_batch.rs / compio_manifest.rs 全用 err 注入）。没有 crash-recovery kill
     测试矩阵在跑，补第 6 个 kill 点无消费方；待矩阵立项时按需求统一布点。
2. **SyncFiles（fail）——无增量价值**：
   - 其验证路径（写 flush 失败 → 回滚）在 fkv 是 `put_at → settle_mapper_batch(false) →
     mapper.abort_batch`（fkv_store:1230-1245），已有三组覆盖：mapping.rs:211/399
     abort_batch 单测、maintenance_batch.rs（AppendManifest:Write / AtomicWriteFile:Sync
     err 注入回滚）、batch_fail.rs（WriteSegments@3）。C++ SyncFiles 与 AppendManifest:Write
     的注入只是同一条错误传播-回滚链路上的不同缝。
   - local flush 时序：`fkv_io_compio:2115` 的 `SyncData` 点已存在，eng-89（todo）已规划
     消费（eng-89:74-75 明确「与 rev-35 正交，验收可借用 SyncFiles 思路但不依赖」）。
   - cloud 远端 flush（flush_all_tbl / put_manifest_with_retry 在途）虽无具名点，但
     cloud 测试栈已有 ObjectStore wrapper 注入先例（cloud.rs:486 RaceObjectStore、term_cas
     的 TryAgain/ExpiredTerm 注入），按 key 定向注入不可重试 put 错误即可穿过
     manifest_retryable 重试直达回滚，比具名 failpoint 更精准（可区分 manifest put 与
     data put）。
3. **去重**：issue/ 全目录 grep OpenOrCreateFD/SyncFiles/CreateFile 仅本条与 eng-89 的
   一处交叉引用（不依赖）；done/17、48-checksum、eng-40-failpoint、io-* 系列文件在本
   仓库 issue/ 中不存在或与注入点无关，无人覆盖，非重复拒绝。

### 备注（供后续立项参考）
若日后建 crash-recovery kill 测试矩阵：优先把 `WritePage` kill 挪到 `open_write_file`
之后（对齐 C++ :982，一次性补齐「0 字节残留」窗口），cloud 侧用 ObjectStore wrapper
注入而非具名点；届时本条的对齐目标自然达成，无需按意见原方案新增两个具名点。
