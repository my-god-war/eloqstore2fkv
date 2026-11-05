# io-44 新建分区目录只 create_dir_all，不同步父目录：manifest/data 落盘后目录项仍可能丢失

来源：2026-11-05 r6 durability 复审新发现（与 eng-89 同批，复审时未入册）。

## 现状（修复前）

`crate/io/fkv_io_compio/src/lib.rs::ensure_dir`：

```rust
let target = dir.clone();
if !spawn_dir_io(move || create_dir_all(&target).is_ok()).await? {
  return Err(KvError::IoFail);
}
self.dir_ready.borrow_mut().insert(dir);
Ok(())
```

只建目录，不 fsync 任何目录项。分区目录路径 `root/<tbl>.<partition>` 直接挂在
`root` 下，而 `root` 本身也可能是本次 `create_dir_all` 懒创建（生产路径没有单独的
建 root + fsync 步骤）。

已有的目录持久化只覆盖「目录内文件」：

- `dirty_dir` + `sync_data` 会 fsync **分区目录**，持久化 data / segment 新文件名
  （`sync_data` 末尾 `self.sync_dir(&dir)`）；
- `append_file_at` / `atomic_write_file` 会 fsync **分区目录**，持久化 manifest 文件名。

但「分区目录本身在 root 里的 entry」「root 自身在其父目录里的 entry」没有任何
fsync 点。`CompioStore::new` 也不创建 / 同步 root。

## C++ 对照（eloqstore 82b774c）

目录创建后同步父目录：`mkdirat` 建目录，随后对目录 fd 做 fdatasync（见
`src/async_io_manager.cpp` / `src/storage/*` 的目录创建路径；复审记录
「C++ mkdirat 后 fdatasync 目录」）。即目录 entry 的持久化是创建流程的一部分，
而不是等后续文件 fsync 顺带完成。

## 差异与风险

崩溃窗口：分区目录新建 → 目录内 data / segment / manifest 全部 fsync 成功 →
进程崩溃 / 掉电 → root 目录的 entry（`demo.1`）未落盘 → 重启后整个分区目录不可见，
该分区已提交的数据全部丢失。与 eng-89 的区别：eng-89 是「manifest 先于数据」导致
存活数据损坏；本条是「文件都持久了但目录 entry 没持久」，丢失范围是整个分区，
且发生在每一次首次写新分区时（首次写、新 branch、truncate 后重建目录都可达）。
在 `root` 也被懒创建的场景，root 的 entry 同样未持久化，丢失范围扩大到整个 store 目录。

## 修复（本 pass 已实现）

`crate/io/fkv_io_compio/src/lib.rs`：

- `ensure_dir`：`create_dir_all` 成功后
  - fsync `root`（持久化分区目录 entry）；
  - 若创建前 `root` 不存在（`root.is_dir()` 为 false），再 fsync `root` 的父目录
    （持久化 root 自身 entry；父路径为空时跳过，如相对路径）;
  - 全部成功后才写入 `dir_ready`；任一 fsync 失败上抛 `KvError`，下一次写重试。
- 新增 `sync_dir_entry`：目录 fsync 的单一入口，含 failpoint `SyncDir`、
  成功计数 `dir_fsync_num`；目录句柄**不进 `fd_cache`**（避免挤占 `fd_limit`
  与污染 `fd_open_num`），`sync_dir`（manifest 目录，带 `manifest_dir_synced`
  去重）复用它。
- 指标：`CompioStore::dir_fsync_num()` 暴露成功目录 fsync 次数（测试 / 诊断用；
  `MeteredIo` 的 `dir_fsync` 只统计 manifest 操作，看不到后端内部目录 fsync）。

## 验收

`crate/io/fkv_io_compio/tests/durability_dir.rs`（新增，单个测试内串行以避免
进程级 failpoint 污染）：

1. root 懒创建：首次写分区 → `dir_fsync_num == 2`（root + root 父目录）；
   同分区重复写不再增加；新分区 → +1；断言目录真实存在。
2. root 已存在：首次写分区 → `dir_fsync_num == 1`。
3. `SyncDir@1!err=IoFail`：`write_page` 返回 `IoFail`，失败不入计数、不标记
   `dir_ready`；reset 后重试成功且计数 +1。
4. 负向验证：临时移除 `ensure_dir` 的父目录同步后测试失败（`dir_fsync_num == 0`）。

风险 / 备注：

- `invalidate_path_cache` / `evict_table` 清空 `dir_ready` 后，下一次写会对已存在
  的分区目录重复 fsync 一次父目录（一次 `open+fdatasync`/表/缓存失效周期），
  换取「外部重建目录」场景下的正确性；未另设持久化标记。
- 目录删除（`try_remove_empty_partition_dir`）未同步父目录：崩溃后可能复活一个
  空目录，不影响数据正确性，未在本条处理。

## 复审（2026-11-05 r7）

**通过。** 实现与测试逐处核实（`fkv_io_compio/src/lib.rs` 03:48 仍被并行会话
触碰，内容与声称一致）：

- `ensure_dir`（lib.rs:1825-1853）：`create_dir_all` 前探测 `root_existed`
  （:1835-1836），成功后先 `sync_dir_entry(&self.root)`（:1842）持久化分区目录
  entry；root 本次新建时再 fsync 其父目录（:1843-1850，空父路径跳过）；全部成功
  才写 `dir_ready`（:1851），失败上抛。
- `sync_dir_entry`（:1859-1867）：单一入口，`fail_point!("SyncDir")` :1860、
  成功后 `dir_fsync_num` +1 :1865；目录句柄 open+sync 即弃、不进 `fd_cache`
  （注释 :1856-1858）。`sync_dir`（:1872-1878）复用并带 `manifest_dir_synced`
  去重。访问器 `dir_fsync_num()` :990-991。
- 测试 `tests/durability_dir.rs`（单测试串行，防进程级 failpoint 污染）：
  (a) root 懒创建首写 `dir_fsync_num == 2`（:39-43）、同分区重写不加（:46-47）、
  新分区 +1（:50-52）且目录真实存在（:53-54）；(b) root 已存在 == 1（:61-62）；
  (c) `SyncDir@1!err=IoFail` → `IoFail` 上抛、失败不计入计数（:66-70）、重试
  成功再 +1（:71-72）。负向逻辑（删父目录同步则计数变 1、断言 2 失败）由计数
  断言结构保证，与 DONE 验收 4 同型。
