# rev-01 FileIo::write_file 缺少 rename 前 fdatasync,tmp+rename 原子性有durability洞

## 位置

- Rust: `fkv/crate/io/fkv_io_compio/src/lib.rs:3026-3047`(`impl FileIo for CompioStore::write_file`)
- Rust 对照(正确实现,同文件): `lib.rs:1935-1961`(`atomic_write_file`,line 1939 `file.sync_data().await` 后才 `rename`)
- C++ 基线: `eloqstore/src/async_io_manager.cpp:2887-2965`(`IouringMgr::WriteSnapshot`,tmp 写完先 `Fdatasync`(line 2938)再 `Rename`(line 2948))
- compio: rename 是 io_uring `RenameAt` SQE(`compio/compio-driver/src/sys/op/fs/iour.rs:244-248`),rename 作为目录项元数据随 journal 提交即持久,而 buffered 写的数据此时仍只在 page cache

## 现状

`write_file` 做 tmp + rename 原子替换,但写完 `write_whole(&mut file, bytes, false)` 后直接 `drop(file)` + `rename(&tmp, &path)`,**没有对 tmp 执行 `sync_data()`**:

```rust
let tmp = tmp_path(&path);
let mut file = File::create(&tmp).await.map_err(map_io_err)?;
if let Err(err) = self.write_whole(&mut file, bytes, false).await { ... }
drop(file);
rename(&tmp, &path).await.map_err(map_io_err)?;   // ← 数据未 fsync
```

`write_whole(direct=false)` 走 buffered 写,数据停留在 page cache。断电窗口内 rename(目录项)可先于数据落盘:重启后可见文件为 0 长度 / 稀疏零洞 / 旧 inode 内容,而 `write_file` 已返回 Ok。

## C++ 实现

eloqstore 的全部 tmp+rename 落盘路径(`WriteSnapshot`,含 manifest / archive / branch manifest)统一是 **tmp 写 → `Fdatasync(tmp)` → `Rename` → `Fdatasync(dir)`**(`async_io_manager.cpp:2937-2962`),rename 前数据必已落盘。data 文件页写虽不落在此路径,但其 durability 由 `SyncData` 的批量 `IORING_OP_FSYNC(DATASYNC)` 保证(`async_io_manager.cpp:2292-2333`),且 data 文件是 O_DIRECT 打开(line 1737/2200),无 page cache 依赖。

## 差异与风险

1. fkv 自己的 `atomic_write_file` 与 `append_file_at`(含 fsync 失败回滚截断)都遵守「返回 Ok 即内容可 fsync 恢复」契约,唯独 `write_file` 不 fsync,同文件内自相矛盾。
2. 调用方 `fkv_cloud/src/lib.rs:1660`(`cache_local_file`)与 `:3046`(`FileIo::write_file`)在 `write_file` 返回 Ok 后立即 `upsert_resident(..., true)` 登记本地文件为完整 resident;`:1666-1672` 的注释明确写了「绝不按 bytes.len() 登记未落盘内容——否则后续读写不再回源」。当前实现下断电重启后 resident 扫描按磁盘重建,可见的空洞 / 截断文件被当作有效缓存:后续读命中零页,checksum 校验失败(`Corrupted`)或读到陈旧页,且 cloud 回源逻辑因 resident 命中不再触发——数据不可恢复。
3. 对已存在 file id 的整文件替换(`write_file` 语义)同样适用:rename 提交目录项后数据丢失即整文件损坏。

## 对齐建议

`write_file` 在 `rename` 前补 `file.sync_data().await.map_err(map_io_err)?;`(复用 `atomic_write_file` 的顺序即可:write → sync_data → rename → forget_fd)。若 rename 的目录项 durability 也需保证,可评估随 `dirty_dir` 机制在 `sync_data` 时补目录 fsync(当前新文件名首次出现会置 `dirty_dir`,但同名替换不置,与风险 2 一致,主修复是 tmp fsync)。

## 严重度

P1(云缓存路径断电后本地文件可被登记为有效 resident 而内容缺失,上层不再回源)

## 核验记录

- 通读 `write_file`(lib.rs:3026-3047)与 `atomic_write_file`(lib.rs:1935-1961):前者无 `sync_data` 调用,后者有(1939);全文件 grep `sync_data` 确认 `write_file` 路径上无任何 fsync/fdatasync。
- 读 C++ `WriteSnapshot`(:2887-2965):tmp 用 `O_CREAT|O_TRUNC|O_RDWR|O_DIRECT` 打开,循环 `Write` 后 `Fdatasync`(:2938)成功才 `Rename`(:2948)。
- 读 compio rename 实现:`compio/compio-fs/src/utils/unix.rs:26` → `op::Rename` → `compio/compio-driver/src/sys/op/fs/iour.rs:244-248`(`opcode::RenameAt`,ring SQE),确认 rename 走 ring 提交、非 page-cache 内事务。
- grep 全仓 `write_file` 调用方:`fkv_cloud/src/lib.rs:1660、:3046`,确认成功后立即登记 resident;`fkv_shard/backend.rs:533`、`roots.rs:288` 为纯转发。
- 与既有 issue 无重复:`issue/new/` 现有 r8a-r8e 各文件均不涉及 `write_file` durability。

## DONE（2026-11-05）

`CompioStore::write_file` 在 rename 前增加 `file.sync_data()`（失败清理 tmp 并上抛），
并把分区目录登记进 `dirty_dir`，由下一次 `sync_data` 完成 rename 后的目录 fsync。
对齐 C++ `WriteSnapshot` 的 tmp→fdatasync→rename→dir fsync 顺序。

## 复审（2026-11-05 v，lane B）

通过（代码 + 顺序契约），残余护栏拆 v-06。亲读
`crate/io/fkv_io_compio/src/lib.rs:3083-3114`：`File::create(tmp)`（:3092）→
`write_whole(buffered)`（:3093）→ `file.sync_data()`（:3101-3105，失败
`remove_file(tmp)` 后 `map_io_err` 上抛）→ `rename`（:3107）→
`forget_fd`（:3109）→ `mark_dirty`（:3110）→ `dirty_dir.insert`（:3112）。
顺序为「写入→fsync(tmp)→rename→（下一次 sync_data 的 dir fsync）」，与
`atomic_write_file`（:1941-1968）一致，DONE 声称属实。**残余**：`write_file`
区间 `3083-3114` 无任何 failpoint（本文件 `fail_point!` 仅 :1878/:1905/:1948/
:1950/:2186/:2385/:2415/:2491/:2812），去掉 :3101 `sync_data()` 现有测试全绿
（测试面仅 `tests/compio.rs:42` 进程内 roundtrip），耐久语义回归无护栏；已拆出
[`v-06`](../new/v-06-write-file-tmp-fsync-no-failpoint-no-durability-test.md)。
归档 `issue/reviewed/`。
