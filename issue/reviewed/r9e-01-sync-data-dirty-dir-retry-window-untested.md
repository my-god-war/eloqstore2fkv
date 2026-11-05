# r9e-01 P3：r8e-01 修复代码在位但验收要求的测试未落地——`sync_data` 的 dirty_dir「失败保留、重试补同步」窗口零覆盖

## 背景（R2 复核 r8e-01）

`issue/done/r8e-01-sync-data-dirty-dir-cleared-before-dir-fsync.md` 的修复本体已核实
在位：`crate/io/fkv_io_compio/src/lib.rs` `sync_data`（:2517-2524）改为
`sync_dir` 成功后才 `dirty_dir.borrow_mut().remove(tbl)`，失败路径保留条目、注释明确。
该部分闭环无异议。

## 现象（亲读，2026-11-05 06:2x 版）

r8e-01 验收条款明确要求：

> 补 `tests/durability_dir.rs` 用例：`sync_data` 路径注入 `SyncDir@1!err=IoFail`
> → 首轮失败上抛、`dirty_dir` 仍在（`dir_fsync_num` 不变 + 二轮重试后递增并成功）；
> 同时断言重试轮里已成功 fdatasync 的文件不会被重复清掉（dirty 集合语义不回归）。

全仓核验结果：

- `grep -rn "SyncDir" crate/ --include="*.rs"`（排除 `_cpp`）只有两处测试引用，全在
  `crate/io/fkv_io_compio/tests/durability_dir.rs`（:5 注释、:66 注入），且该文件的
  唯一测试 `partition_dir_creation_syncs_parent_entries` (c) 段走的是
  `write_page → ensure_dir → sync_dir_entry` 路径——`write_page`（src/lib.rs:2374-2401）
  只 `mark_dirty` 不调 `sync_data`，与 r8e-01 修复的 `sync_data` 分支（:2520-2524）
  无交集。
- `crate/io/fkv_io_compio/tests/hardening.rs:171 sync_data_flushes_dirty_files_and_clears_set`
  只覆盖 `sync_data` 成功路径（dirty 清空 + 幂等），无失败注入。
- `crate/engine/fkv_store/tests/durability_sync_order.rs` 注入的是 `SyncData@1`
  （:73/:159），不触 `SyncDir`。
- 全仓无任何测试断言「`sync_data` 目录 fsync 失败 → 重试必须重新 fsync 目录」。
  这恰是 r8e-01 立项的故障窗口：回退该修复（把 `remove` 挪回 `sync_dir` 之前）
  现有测试全绿——回归无护栏。

## 目标

1. 在 `tests/durability_dir.rs`（或新文件）补 `sync_data` 路径用例：先写页
   （目录已就绪、`dirty_dir` 由 `write_path` 登记为新文件名首次出现），注入
   `SyncDir@1!err=IoFail` 后调 `sync_data`：断言首轮 `IoFail` 上抛、
   `dir_fsync_num` 不变、`dirty_file_num` 仍非零（失败保留）；清除注入后重试：
   断言 `dir_fsync_num` 递增（目录被重新同步）且 `dirty_file_num == 0`。
2. 同用例内覆盖「重试轮不再重复 fdatasync 已成功的文件」：重试轮 `data_sync_num`
   递增但文件 fdatasync 只对仍在 dirty 集合的文件发生（可用首轮已成功的文件
   被清脏后不重复计数佐证，或最小化为断言重试后 dirty 全清）。

## 验收

- 新用例在修复回退（`remove` 提到 `sync_dir` 前）时必须失败。
- 既有 `durability_dir.rs` / `hardening.rs` / contract 三镜像不回归；
  `./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- R9 独立审查（r9e，2026-11-05）亲读 `fkv_io_compio/src/lib.rs` 全文与
  `tests/{durability_dir,hardening}.rs`、`fkv_store/tests/durability_sync_order.rs`
  注入点；`grep -rn "SyncDir\|dirty_dir" crate/`（排除 `_cpp`）零额外命中。
- 查重：r8e-01 的 DONE 只声明代码改动；io-44 / eng-89 只覆盖 ensure_dir 与
  SyncData 注入；r7 组 2 复审（00-r7-group2.md）核对 io-44 时同样只核
  `ensure_dir` 路径。无承接条目。


## DONE（2026-11-05）

DONE：`durability_dir.rs` 新增 `sync_data_retries_dir_fsync_after_failure`：注入 `SyncDir` 失败后计数不前进，重试成功并清 dirty。

## 复审（2026-11-05 v，lane B）

通过。亲读 `crate/io/fkv_io_compio/tests/durability_dir.rs:78-96`：`write_page`
（经 `data_path`→`write_path` 置 `dirty_dir` :1218-1220、`mark_dirty` :2387/:2406）
后 `set_spec("SyncDir@1!err=IoFail")` 调 `sync_data` → 断言上抛 `IoFail`（:89）、
`dir_fsync_num == before`（:91）、清除注入重试后 `dir_fsync_num == before + 1`
（:94）且 `dirty_file_num == 0`（:95）。注入点命中 `sync_dir_entry` 的
`fail_point!("SyncDir")`（src/lib.rs:2186），计数在 fsync 成功后才递增（:2191）。
回退 r8e-01 修复（`remove` 提前）时第二轮断言必失败——护栏有效。
归档 `issue/reviewed/`。
