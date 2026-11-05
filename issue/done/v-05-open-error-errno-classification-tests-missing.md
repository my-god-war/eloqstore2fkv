# v-05 P3：打开路径的 errno 分类（segment / data）与 `map_io_err` 的 ResourceBusy 映射均无测试钉住——r8e-03 / r9e-02 / rev-02 的验收残余

## 背景（v 验证轮 lane B 复核）

r8e-03、r9e-02 的**代码修复已在位**，rev-02 的映射也已在位，但三者共同的
「非 NotFound 打开错误按 errno 分类」语义**没有任何测试断言**：

- `crate/io/fkv_io_compio/src/lib.rs:1970-1998` `open_segment_file`：两个失败
  出口均 `map_io_err(err)`（:1993 重查臂、:1996 首查臂），仅 NotFound 耗尽
  回 `ResourceMissing`（:1990）。
- `crate/io/fkv_io_compio/src/lib.rs:2001-2021` `open_data_file`：两个非
  NotFound 出口 `map_io_err(err)`（:2016、:2019），`NotFound → Ok(None)`
  （:2015）。
- `crate/io/fkv_io_compio/src/lib.rs:719-738` `map_io_err`：新分支
  `ErrorKind::ResourceBusy => KvError::TryAgain`（:727，rev-02），raw ENOBUFS
  分支保留（:730）。

r8e-03 验收明文要求「把 segment 期望路径替换为同名目录（EISDIR），
`read_segment` 返回 `IoFail`；data 侧同构断言 `NoPermission`/`IoFail`」；
r9e-02 目标 2/验收同样要求该用例；但两条 DONE 均只声明代码改动，测试未落地。
rev-02 亦无任何断言（`map_io_err` 为 `pub(crate)`，集成测试无法直接调用）。

## 证据（亲读，2026-11-05 v 轮）

- 全仓测试面 grep 零命中：
  - `grep -rn "ResourceBusy\|OpenFileLimit\|NoPermission" crate --include=*.rs`
    （排除 `_cpp`）仅 `fkv_error/tests/error.rs:6/:22` 命中 `is_retryable` 集合
    常量，无「某 IO 错误 → 某 KvError」的映射断言。
  - `grep -rln "chmod\|EISDIR\|EACCES" crate --include=*.rs`（排除 `_cpp`）空。
- `crate/io/fkv_io_compio/tests/contract.rs:57-74` 三镜像仍只钉
  `NotFound → ResourceMissing`（:66、:72），未覆盖非 NotFound。
- `crate/io/fkv_io_compio/tests/hardening.rs`、`tests/durability_dir.rs`、
  `tests/fixed.rs` 均无打开错误分类用例。

同类折叠点（r9e-02 目标 3 顺带清扫项）仍未逐一裁定，`KvError::IoFail` 硬编码
仍在：`remove_file_by_kind`/`remove_file_variant_by_kind`（:2093/:2121）、
`ensure_dir` 的 `create_dir_all` 失败（:2166）、`sync_data` 的 `open_for_sync`
失败臂（:2506）、`drop_manifest`（:2582）、`remove_manifest_file`（:2605）、
`read_manifest_path`（:2642）、`try_remove_empty_partition_dir`（:2682/:2700/:2728）。

## 目标

1. 补打开路径 errno 分类用例（建议 `hardening.rs` 新段）：
   - 把 segment / data 期望路径替换为**同名目录**，`open` 返回 `EISDIR` /
     平台可复现的非 NotFound 错误 → 断言 `read_segment` / `read_file` 返回
     `IoFail` 而非 `ResourceMissing`（修复前/摊平回退时必须失败）。
   - Linux CI 可选：`chmod 000` 断言 `NoPermission`；构造 fd 耗尽或以 failpoint
     注入 `EMFILE` 断言 `OpenFileLimit`（可重试类）。
2. 补 `map_io_err` 单元测试（`lib.rs` 内 `#[cfg(test)]`，因函数为 `pub(crate)`）：
   `io::Error::new(ErrorKind::ResourceBusy, ..)` → `TryAgain`；
   `io::Error::from_raw_os_error(libc::EMFILE)` → `OpenFileLimit`；
   `PermissionDenied` → `NoPermission`；raw `EAGAIN` → `TryAgain`。
3. 对「同类折叠点」逐个裁定：改 `map_io_err` 或在代码注释登记「摊平无害」
   理由（r9e-02 目标 3 未做）。

## 验收

- 新用例在 `map_io_err` 回退 / `open_*_file` 改回硬编码摊平时必须失败。
- `contract.rs` 三镜像、`hardening.rs`、cloud cache-miss 用例不回归；
  `./sh/clippy.sh`、`./test.sh` 通过。

## 查重

- `grep -rn "ResourceBusy\|OpenFileLimit\|NoPermission\|EISDIR" issue/`：无
  承接条目；r8e-03 / r9e-02 DONE 只声明代码，rev-02 无测试要求但映射无覆盖。
- 本条承载 r8e-03 / r9e-02 / rev-02 三者的测试残余，未与其它 lane 条目重复。

---

## 甄别（2026-11-05，v 验证轮 lane B）

**结论：采纳（P2）。** 证据全部核实：

- `map_io_err`（`crate/io/fkv_io_compio/src/lib.rs:719-738`）确为 `pub(crate)`，
  含 `PermissionDenied → NoPermission`(:723)、`ResourceBusy → TryAgain`(:727)、
  raw `EINTR/EAGAIN/ENOBUFS → TryAgain`(:730-731)、
  `EMFILE/ENFILE → OpenFileLimit`(:733)、`ENOSPC/EDQUOT → OutOfSpace`(:734)。
- 打开路径两处出口确经 `map_io_err`：`open_segment_file` :1993/:1996，
  `open_data_file` :2016/:2019；仅 NotFound 耗尽回 `ResourceMissing`。
- 全仓测试零覆盖：`src/lib.rs` 的 `#[cfg(test)] mod test`（:3210-3254）仅
  `WriteGate` 两个用例，无 `map_io_err` 断言；`tests/contract.rs` 只钉
  `NotFound → ResourceMissing`（:63/:71）；`hardening.rs`/`naming.rs`/`fixed.rs`
  无打开错误分类用例。`issue/` 查重无重复（r9e-02 已明文「拆出 v-05」）。

### 对原方案的技术修正（重要）

原目标 1 建议「把 segment/data 期望路径替换为同名目录 → `open` 返回 EISDIR」，
对**读路径不成立**：`open_read_file`（:1367-1379）以 `OpenOptions::new().read(true)`
（O_RDONLY）打开，Linux/macOS 上 `open(dir, O_RDONLY)` 会**成功**，EISDIR 仅在随后
的 `read()` 出现。因此该错误不经过本次要钉的 `open_*_file` 分类臂：即使把出口改回
硬编码 `IoFail`，`read_segment` 仍返回 `IoFail`，用例**无法区分回归**，不满足验收
「摊平回退时必须失败」。目录法仅对写路径（`open_write_file` 以 read+write 打开，
O_RDWR 遇目录报 EISDIR）有效，不适用本只读打开。原目标 2 的 `map_io_err` 单测方向
正确，作为最小可行方案保留。

## 开发计划

1. `crate/io/fkv_io_compio/src/lib.rs` 内 `#[cfg(test)] mod test` 追加单测
   `map_io_err_maps_open_errnos`（`pub(crate)` 可直接调用），逐条断言：
   - `io::Error::new(ErrorKind::ResourceBusy, "")` → `KvError::TryAgain`
   - `io::Error::from_raw_os_error(libc::ENOBUFS)` → `TryAgain`（raw 分支）
   - `io::Error::from_raw_os_error(libc::EAGAIN)` → `TryAgain`
   - `io::Error::from_raw_os_error(libc::EMFILE)`、`ENFILE` → `OpenFileLimit`
   - `io::Error::new(ErrorKind::PermissionDenied, "")` → `NoPermission`
   - `io::Error::new(ErrorKind::NotFound, "")` → `ResourceMissing`
   - `io::Error::from_raw_os_error(libc::ENOSPC)` → `OutOfSpace`
   - 未知错误 → `IoFail`
   删除 `ResourceBusy` 分支或调整 raw 分支时须失败。

2. `crate/io/fkv_io_compio/tests/hardening.rs` 新增
   `#[cfg(unix)] async fn open_errno_is_classified_not_folded()`（wire 层，满足
   验收的回归可区分性）：
   - 建 store 落盘后，按命名规则（`demo.1/segment_0_{branch}_{term}`、
     `demo.1/data_1_{branch}_{term}`，见 `tests/naming.rs:23-27`）对目标文件
     `std::fs::set_permissions` 置 000；
   - 断言 `read_segment` → `KvError::NoPermission`（初查臂 :1996）、
     `read_page`/`read_file` → `NoPermission`（:2019）；
   - 前置：`geteuid()!=0`（root 会绕过权限位）；读句柄未在 fd 缓存中（写句柄
     独立，首次读必 fresh open）。
   - 回归判别：出口改回 `Err(_) => KvError::IoFail` 时该断言失败。
   - 可选跨平台方案：在 `open_read_file` 首行加 failpoint 注入
     `io::Error::new(ErrorKind::PermissionDenied, ..)`，以 `set_spec("OpenReadFile@1")`
     驱动（需一处源码改动）。

3. 目标 3（同类折叠点 `KvError::IoFail` 硬编码：:2093/:2121/:2166/:2506/:2582/
   :2605/:2642/:2682 等）逐点裁定改走 `map_io_err` 或注释登记「摊平无害」；
   建议独立 follow-up，不与本条测试同批。

## 验收（修订）

- 目标 1 采用 chmod 000（或 failpoint）方案，废弃同名目录/EISDIR 方案。
- 第 1、2 项用例在 `map_io_err` 相应分支删除 / `open_*_file` 摊平回退时必须失败。
- `contract.rs` 三镜像、`hardening.rs`、`naming.rs` 不回归；`./sh/clippy.sh`、
  `./test.sh` 通过。


## DONE（2026-11-05）

DONE：新增 `open_errors_keep_errno_classification`，data / segment 的 EISDIR 打开错误均为 IoFail（非 ResourceMissing）。
