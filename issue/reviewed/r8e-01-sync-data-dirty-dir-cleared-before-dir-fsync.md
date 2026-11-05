# r8e-01 P2：compio `sync_data` 先清 `dirty_dir` 再 fsync 目录，目录项 fsync 失败重试后不再补同步，manifest 可能引用掉电即失的文件名

## 现象（亲读，2026-11-05 05:55 版）

`crate/io/fkv_io_compio/src/lib.rs` `PageIo::sync_data`：

```rust
// :2506-2511 文件路径：每个文件只有 fdatasync 成功后才从 dirty 集合移除。
for (path, result) in result_li {
  result.map_err(map_io_err)?;
  if let Some(set) = self.dirty.borrow_mut().get_mut(tbl) {
    set.remove(&path);
  }
}
// :2513-2517 目录路径：先把 dirty_dir 条目 remove 掉，再执行 sync_dir。
if self.dirty_dir.borrow_mut().remove(tbl) {
  let dir = self.part_dir(tbl);
  self.sync_dir(&dir).await?;
}
```

- `dirty_dir` 的语义由 `write_path`（:1252-1255 附近）登记：「新文件名首次出现：
  目录项需要在下一次 durability barrier 落盘」；io-44 / eng-89 的持久化契约是
  **sync_data 成功 ⇒ manifest 引用的文件名目录项已 fsync**。
- `remove(tbl)` 在 `sync_dir` 之前执行：`sync_dir`（打开目录 fd + fsync）返回
  `Err`（EIO / 目录被外部移走）时该条目已丢失。此后：
  - 已写过页的文件仍留在 `dirty` 集合（文件路径按成功逐个移除，失败路径保留），
    调用方重试 `sync_data` 时文件会重新 fdatasync；
  - 但同一批文件名已在分区目录索引里（`write_path` 只在 `previous.is_none()`
    时登记 `dirty_dir`），重写已有文件名**不会**再置位；
  - 于是重试的 `sync_data` 不再 fsync 目录即返回 `Ok`。调用方（store 的
    append_mapping_log / manifest 发布链，见 eng-89 的
    `durability_sync_order.rs` 两路径）随即推进 manifest。
- 后果窗口：目录项 fsync 失败一次 → 重试成功（无目录 fsync）→ manifest 发布
  → 掉电。分区目录里新文件的 entry 可能未持久化，manifest 引用的 data /
  segment 文件整体消失，reopen 后按 `ResourceMissing` 读失败——恰好是 io-44
  立项时要根除的故障形态，只是在「目录 fsync 失败后重试」的窗口里复活。
- 对比同一函数内文件路径的写法（先 fsync 成功再清脏）与 `ensure_dir`
  （`dir_ready` 只在 fsync 全部成功后写、失败可重试，`tests/durability_dir.rs`
  (c) 段钉死该语义）：目录路径是本文件里唯一的「先清标记后执行」反例。
- failpoint 面：`SyncDir` 注入测试只覆盖 `ensure_dir` 路径
  （`tests/durability_dir.rs`），`sync_data` → `sync_dir` 的失败重试行为无测试。

## C++ 对照

- `src/async_io_manager.cpp` `IouringMgr::CreateFile`（:2148-2166）：创建新文件
  后把**目录 fd 本身**标脏（:2166 `dir_fd.Get()->dirty_ = true`）——目录 fd 与
  data fd 同住 `tables_[tbl].fds_`。
- `IouringMgr::SyncData`（:1310-1340）扫描全部 dirty fd（含目录）统一送
  `SyncFiles`；`SyncFiles`（:2335-2348）先 `FdatasyncFiles`，
  `CHECK_KV_ERR(err)` 失败立即返回，**dirty 标志（含目录 fd 的）保持置位**，
  全部成功后才逐个清 `fd->dirty_`。即 C++ 的目录项同步天然具备「失败保留、
  重试必补」语义。
- Rust 的 `dirty_dir` 本应映射 C++ 的「目录 fd dirty 标志」，清除时机却放在
  fsync 之前，偏离对照实现。

## 目标

1. `sync_data` 把 `dirty_dir` 的移除挪到 `sync_dir` 成功之后（失败路径保留，
   与文件路径、`ensure_dir` 的 `dir_ready` 语义对齐）。
2. 补 `tests/durability_dir.rs` 用例：`sync_data` 路径注入 `SyncDir@1!err=IoFail`
   → 首轮失败上抛、`dirty_dir` 仍在（可用 `dir_fsync_num` 不变 +
   二轮重试后 `dir_fsync_num` 递增并成功判定）；同时断言重试轮里已成功
   fdatasync 的文件不会被重复清掉（dirty 集合语义不回归）。

## 验收

- 注入 `SyncDir` 失败后重试：第二轮 `sync_data` 必须再次 fsync 目录（修复前
  该断言失败——flag 已丢）。
- 既有 `durability_sync_order.rs` / `durability_dir.rs` / `evict.rs` /
  contract 三镜像全绿；`./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- R8 独立审查（r8e，2026-11-05）亲读 `fkv_io_compio/src/lib.rs` 全文；
  `sync_data` 锚点行号（:2506-2517）为 05:51 版读数（该文件 05:51 有并行
  会话改动，本条锚点区域改动前后内容一致，以内容锚点为准）。
- C++ 亲读 `async_io_manager.cpp`：`CreateFile` :2148-2166、`SyncData`
  :1310-1340、`SyncFiles` :2335-2348、`MakeDir` 后目录 fdatasync :2139。
- 查重：io-44（分区目录 entry fsync）只覆盖「缺目录 fsync」本体与
  `ensure_dir` 路径，未提失败重试窗口；eng-89（compact 前 data fsync）的
  `SyncData` failpoint 在函数顶部注入，不触及本窗口；r6 io lane 报告与
  r7 组 2 复审均未涉及。`issue/` 全目录 grep `dirty_dir` 无其他承接条目。
- 并行写入区提示：`fkv_cloud/src/lib.rs`（05:48）在我审查期间已把 cloud
  `drop_manifest` 的 term-0 错位修掉并补测试（`tests/cloud.rs:1096`
  `drop_manifest_removes_current_term_anchor`）——该潜在发现已闭环，不重复立项。


## DONE（2026-11-05，第二轮复审修复）

DONE：`sync_data` 改为目录 fsync 成功后才清 `dirty_dir`，失败保留条目下一次重试。

## 复审（2026-11-05 v，lane B）

通过。亲读 `crate/io/fkv_io_compio/src/lib.rs:2524-2531`：`if self.dirty_dir.borrow().contains(tbl) { sync_dir(&dir).await?; dirty_dir.remove(tbl); }`——目录 fsync 成功后才清标记，失败经 `?` 上抛且条目保留，与文件路径（:2518-2523「先 fsync 成功再清脏」）、`ensure_dir` 的 `dir_ready`（:2177）语义一致。验收测试由 r9e-01 落地（`tests/durability_dir.rs:78-96`），failpoint 为真实注入（`SyncDir@1!err=IoFail`，命中 `sync_dir_entry` 的 `fail_point!("SyncDir")` :2186），回退该修复时第二轮 `dir_fsync_num` 断言必失败。归档 `issue/reviewed/`。
