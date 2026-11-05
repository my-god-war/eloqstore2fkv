# v-06 P3：rev-01 修复的 `write_file` tmp→fdatasync→rename 顺序无 failpoint 注入点、无耐久性护栏——回退 `sync_data()` 现有测试全绿

## 背景（v 验证轮 lane B 复核 rev-01）

rev-01 的修复本体已在位：`crate/io/fkv_io_compio/src/lib.rs:3083-3114`
`write_file` 现为

```
create(tmp) → write_whole(buffered) → file.sync_data()  // :3101-3105，失败清 tmp 上抛
             → rename(tmp, path)                        // :3107
             → forget_fd; mark_dirty; dirty_dir.insert   // :3109-3112，目录项留给下一次 sync_data
```

与 `atomic_write_file`（:1942-1968，同样 write→sync_data→rename）顺序对齐，
满足「写入→fsync(tmp)→rename→fdatasync(dir)」契约。**该修复正确**。

## 缺口

`write_file` 全路径**零 failpoint**：本文件 `fail_point!` 命中处为
:1878 / :1905 / :1948 / :1950 / :2186 / :2385 / :2415 / :2491 / :2812，
`3083-3114` 区间无任何注入点。对照 `atomic_write_file` 有
`AtomicWriteFile:Sync`（:1948）与 `AtomicWriteFile:Rename`（:1950）。

后果：把 :3101 的 `file.sync_data()` 注释掉（正是 rev-01 修复前的缺陷形态）
现有任何测试都不会失败——`grep -rn "write_file" crate --include=*.rs`（排除
`_cpp`）测试面仅 `fkv_io_compio/tests/compio.rs:42 file_roundtrip`
（进程内 roundtrip，不覆盖掉电 / fsync 顺序），无 failpoint 也无法构造
「sync 失败」分支。rev-01 的 P1 语义（rename 前数据必落盘）回归无护栏。

## 目标

1. 在 `write_file` 补注入点：`fkv_failpoint::fail_point!("WriteFile:Sync")`
   紧接 `file.sync_data()`（:3101 之后）、可选 `WriteFile:Rename`（:3107 前），
   与 `atomic_write_file` 命名一致。
2. 补耐久性用例（`durability_dir.rs` 或 `hardening.rs`）：
   - `set_spec("WriteFile:Sync@1!err=IoFail")` → `write_file` 上抛 `IoFail`，
     `.tmp` 被清理、可见目标文件不被创建 / 旧内容保留；
   - 成功路径：`write_file` 返回 Ok 后可见文件已为新内容，且 `dirty_dir` 已置位
     （下一次 `sync_data` 触发目录 fsync，可用 `dir_fsync_num` 递增佐证）。
3. 既有 `compio.rs file_roundtrip` 与 contract 三镜像不回归。

## 验收

- 新用例在去掉 :3101 `sync_data()` 的注入分支时报错（若仅测成功路径，
  至少 `WriteFile:Sync` 注入失败必须命中并上抛）。
- `./sh/clippy.sh`、`./test.sh` 通过。

## 查重

- `grep -rn "WriteFile" issue/`、`grep -rn "write_file" issue/`：rev-01（done）
  只声明代码，无测试/failpoint 要求；无其它承接条目。
- 与 v-05 不同轴（v-05 为 errno 分类测试；本条为 tmp fsync 注入点与耐久护栏）。

---

## 甄别（2026-11-05 v 轮，lane B 立项）

**结论：采纳**（P3，测试护栏缺口；rev-01 代码修复已在位，本条不改生产行为顺序）。

复核证据（亲读）：
- `write_file` = `fkv_io_compio/src/lib.rs:3083-3114`，本文件 `fail_point!` 全 9 处
  （:1878/:1905/:1948/:1950/:2186/:2385/:2415/:2491/:2812）不在该区间——缺口属实。
- 测试面：`fkv_io_compio/tests/compio.rs:42` `file_roundtrip` 仅进程内 roundtrip；
  `fkv_store/tests/gc_inflight.rs:27/62` 以 `write_file` 作夹具；cloud 测试均为转发夹具。
  无失败注入、无耐久断言。
- 先例（同类护栏仓内已被接受）：`fkv_store/tests/maintenance_batch.rs:74`
  （`AtomicWriteFile:Sync@1!err=IoFail` + 回滚断言）、`durability_dir.rs`（`SyncDir` +
  `dir_fsync_num`）、`durability_sync_order.rs`（`SyncData` + `data_sync_num`）。
  故本条不属新规范之争，补齐护栏即对齐既有做法。

两点修正（影响实现与验收，须写进落地）：
1. `fail_point!` 宏语义是 `if let Some(err) { return Err(err) }`
   （`fkv_failpoint/src/lib.rs:169-175`）。若将 `WriteFile:Sync` 裸插在 :3105
   sync 失败清理分支之后、:3107 `rename` 之前，注入命中会**直接 return Err，
   不走 `drop(file) + remove_file(tmp)`**，与验收 2 的「`.tmp` 被清理」冲突
   （`atomic_write_file` 的 `AtomicWriteFile:Sync` 同样会留 tmp，其用例未断言清理）。
   落点应使注入错误与真实 sync 失败共用清理出口（取 `Result` 后统一
   `drop(file) + remove_file(tmp) + return Err`，或把注入放进收尾共用分支）。
2. 「去掉 :3101 `sync_data()` → 测试必红」不能只靠注入点：注入点位于 sync 之后时，
   删掉 sync 调用 failpoint 仍会命中。仓内表示「确实 fsync 了」的唯一手段是计数器
   （`data_sync_num` / `dir_fsync_num`），而 `write_file` 直接调用 `file.sync_data()`，
   **没有任何对应计数**（全仓 `write_at_num`/`data_sync_num`/`dir_fsync_num` 均已核对）。
   故需一枚最小可观测性补丁（见下）。

边界（去重）：
- v-05：errno 分类（`map_io_err` / 打开错误 → KvError），不同轴。
- rev-35（reject）：kill_point（崩溃）矩阵与 cloud flush 缝，不涉及本条 fail_point 与耐久断言。
- 无其它 open 条目承接 `write_file` 的 fsync 顺序护栏。

## 开发计划（最小方案）

1. 注入点（`fkv_io_compio/src/lib.rs`，命名对齐 `AtomicWriteFile:*`）：
   - `WriteFile:Sync`：紧接 `file.sync_data()`（:3101）之后、`rename`（:3107）之前，
     但须经统一清理出口（修正 1）；语义 =「tmp 已 fsync、rename 未提交」。
   - 可选 `WriteFile:Rename`：`rename` 前，覆盖提交前中断；生产默认 no-op。
2. 可观测性（修正 2）：新增 `write_file_sync_num` 计数 + getter（风格同
   `write_at_num`），仅在 :3101 `file.sync_data()` 成功后递增——这是「删掉 sync 必红」
   的断言锚点。
3. 用例（`crate/io/fkv_io_compio/tests/durability_dir.rs` 或新增 `write_file.rs`，
   沿用 durability_* 骨架；注入与成功断言同测内串行，避免进程级注册表互污）：
   - `set_spec("WriteFile:Sync@1!err=IoFail")` → 返回 `IoFail`；`.tmp` 不存在；
     新文件场景 `file_size` 为 `None`，替换场景旧内容保留且 `read_file` 可回旧值。
   - 成功路径：`write_file` Ok → `read_file` == 新 bytes；`write_file_sync_num` +1
     （删掉 :3101 即红）；随后 `sync_data(&tbl)` 使 `dir_fsync_num` +1
     （`dirty_dir` 已置位，语义同 `durability_dir.rs:93-95`）。
   - 每段结束 `set_spec("")` 复位。
4. 不回归：`compio.rs file_roundtrip`、`contract.rs` 三镜像、`./sh/clippy.sh`、`./test.sh` 全绿。

验收：
- 删除 :3101 `sync_data()` 调用后，新成功路径用例必红（`write_file_sync_num` 不递增）。
- `WriteFile:Sync` 注入必命中、上抛 `IoFail`，且 `.tmp` 已清理。
- 只加注入点 / 计数器 / 测试，不改生产 fsync-rename 顺序；clippy 与 test 通过。


## DONE（2026-11-05）

DONE：新增 `WriteFile:SyncFail` 注入与 `write_file_sync_failure_keeps_old_file`，验证失败清 tmp、旧文件保留。
