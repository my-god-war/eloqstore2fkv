# rev-02 map_io_err 的 ENOBUFS 分类在 completion 路径不可达:compio 把 CQE -ENOBUFS 改写为无 raw code 的 ResourceBusy

## 位置

- Rust: `fkv/crate/io/fkv_io_compio/src/lib.rs:719-735`(`map_io_err`,ENOBUFS 分支在 line 727)
- compio: `compio/compio-driver/src/sys/driver/iour/mod.rs:544-558`(`create_result`:CQE `-ENOBUFS` → `io::Error::new(ErrorKind::ResourceBusy, "buffer ring has no available buffer")`)
- C++ 基线: `eloqstore/src/async_io_manager.cpp:1505-1533`(`ToKvError`:`-EINTR / -EAGAIN / -ENOBUFS` → `KvError::TryAgain`)

## 现状

`map_io_err` 依赖 `err.raw_os_error()` 匹配 `libc::ENOBUFS` 映射为 `KvError::TryAgain`(瞬时、可重试,`is_retryable()` 覆盖 TryAgain)。但 compio io-uring driver 收割 CQE 时,`create_result` 把 `-ENOBUFS` 用 `io::Error::new(...)`(而非 `from_raw_os_error`)包装成 `ErrorKind::ResourceBusy` 且**丢弃 errno 数值**:`raw_os_error()` 返回 `None`。于是:

1. 凡经 CQE 完成的 ENOBUFS(provided-buffer 环耗尽等内核路径),`map_io_err` 的 ENOBUFS 分支永不命中,`ResourceBusy` kind 又不在前置的 `WouldBlock | Interrupted` 分支内,最终落到 `_ => IoFail` —— 瞬时错误被归类为不可重试硬错误,与 C++ `ToKvError(-ENOBUFS) = TryAgain` 相反。
2. 该 ENOBUFS 分支目前只对「提交期 syscall 原样透传的 errno」(如 `io_uring_enter` 返回的原始错误)生效,对 completion 错误是死代码。

当前热路径被掩盖:`read_segment` 的 managed read 用 `if let Ok(Some(buf)) = file.read_managed_at(..)`(lib.rs:2732-2738)把 `Err(ResourceBusy)` 直接当回落信号,不经过 `map_io_err`;写路径的 `write_page_group` 不使用 provided-buffer。因此现阶段无直接故障,但任何未来把 managed/provided-buffer op 的 completion 错误交给 `map_io_err` 的改动(或 fixed 缓冲路径出现 ENOBUFS 类错误)都会得到 `IoFail`,上层按硬错误中断写 / 读事务。

## C++ 实现

`ToKvError(int err_no)` 直接按 CQE 的负 errno 分类:`-EINTR/-EAGAIN/-ENOBUFS → TryAgain`,随后 `ReadPage` 的 `while (... || ToKvError(res) == KvError::TryAgain)`(async_io_manager.cpp:772-773)与 `ReadPages/ReadSegments/WriteSegments` 的重试循环都按瞬时处理无限重投 SQE。errno 全程保留原值,不存在被改写丢失的问题。

## 差异与风险

- 分类错误(实现但不正确):`map_io_err` 的 ENOBUFS 分支在 compio completion 语义下不可达,实际错误形态 `ResourceBusy` 未映射,瞬时错误被升级为 `IoFail`。
- 注释误导:lib.rs:718 声明「保留可重试 / 可诊断语义」,读者会误信 ENOBUFS 已被覆盖。
- 目前唯一产生 ResourceBusy 的 fkv 路径(managed read)恰好用 `Ok(Some(..))` 匹配吞掉了错误,问题不可见 —— 属于潜伏分类洞,不修会在后续引入 provided-buffer 写或直用 `submit(op)` 时引爆。

## 对齐建议

`map_io_err` 增加 `ErrorKind::ResourceBusy => KvError::Busy`(或 `TryAgain`,与 `is_retryable()` 对齐)前置分支;如需保留内核 ENOBUFS 语义可在注释中标注 completion 形态。managed read 调用点维持现回落逻辑不变。

## 严重度

P2(当前被 managed-read 回落掩盖;分类逻辑确实错误,未来路径会踩中)

## 核验记录

- 读 compio `iour/mod.rs::create_result`(:544-558):`-result == ENOBUFS` 分支使用 `io::Error::new`,`raw_os_error()` 为 None —— 用 std `io::Error::new` 语义确认(非 `from_raw_os_error`)。
- 读 `map_io_err`(lib.rs:719-735):kind 匹配仅 `NotFound/PermissionDenied/WouldBlock/Interrupted/OutOfMem`,ENOBUFS 只在 `raw_os_error` 分支;`ResourceBusy` 无匹配 → `IoFail`。
- 读 C++ `ToKvError`(:1505-1533)与 ReadPage 重试条件(:764-773):C++ 中 ENOBUFS 为可重试。
- 读 `File::read_managed_at`(compio-fs/src/file.rs:285-294):CQE 错误经 `take_buffer` 以 `Err` 透出;fkv lib.rs:2732-2738 用 `if let Ok(Some(..))` 回落,当前不经过 map_io_err —— 确认现网无直接故障、仅分类洞。

## DONE（2026-11-05）

`map_io_err` 增加 `ErrorKind::ResourceBusy => KvError::TryAgain`，覆盖 compio
io_uring completion 丢失 raw ENOBUFS 的形态；原 ENOBUFS raw 分支保留。

## 复审（2026-11-05 v，lane B）

通过（代码），残余测试并入 v-05。亲读 `crate/io/fkv_io_compio/src/lib.rs:719-738`：
新增 `ErrorKind::ResourceBusy => KvError::TryAgain`（:725-727，注释说明 compio
completion 丢 raw errno），raw 分支 `EINTR/EAGAIN/ENOBUFS → TryAgain`（:730）、
`EMFILE/ENFILE → OpenFileLimit`（:733）保留。DONE 声称属实，分类与 C++
`ToKvError` 一致。**残余**：`map_io_err` 为 `pub(crate)`，无 `#[cfg(test)]`
用例钉住该映射（`lib.rs:3210` 的 `mod test` 只测 `WriteGate`），ResourceBusy →
TryAgain 无断言；已并入
[`v-05`](../new/v-05-open-error-errno-classification-tests-missing.md)。
归档 `issue/reviewed/`。
