
## DONE（2026-11-04）

落地方式（原“阻塞”已解除，`fkv_io_compio/src/lib.rs` 本 lane 可改）：

- `fixed.rs`：新增 `type FdGuard = SharedFd<std::fs::File>`；`FixedRead` / `FixedWrite`
  的 `fd: RawFd` 改为 `fd: FdGuard`，`create_entry` 与 `call_blocking` 均经
  `self.fd.as_raw_fd()` 取号，op 不再持裸 fd 号。`FixedRead::new` / `FixedWrite::new`
  参数由 `RawFd` 改为 `FdGuard`，从 API 层面禁止再传裸 fd。
- `lib.rs`：三处构造点改传 `file.to_shared_fd()`（`:1380` `submit_fixed_write`、
  `:2336` `read_segment_fixed`、`:2387` `write_segment_fixed`），删掉
  `use std::os::fd::AsRawFd`，新增 `#[cfg(target_os = "linux")] use compio::driver::ToSharedFd`。
- 语义未变：I/O 参数、failpoint 顺序、`call_blocking` 回退逻辑与短读补零均未动；
  仅把 fd 生命周期从调用方局部绑定上移到 op 内部（与 compio 内建 op 一致）。

生命周期依据（复核 compio 源码）：

- `compio-fs 0.12.1 file.rs:302` `impl_raw_fd!(File, std::fs::File, inner)` →
  `File::to_shared_fd()` 返回 `SharedFd<std::fs::File>`（`Arc` 语义引用计数）。
- `compio-runtime 0.12.6 future/future.rs:69-76`：`Submit` 的 `PinnedDrop` 只
  `driver.cancel(key)`，op 本体由 driver 的 `Key<T>` 持有直到完成/取消确认，
  所以 op 里的 guard 覆盖整个在飞窗口。

测试：

- 新增 `fixed::tests::fd_guard_outlives_source_file_drop`：op 持 guard，源句柄
  drop 后 `fcntl(F_GETFD)` 仍成功、`call_blocking` 仍能读；op drop 后 fd 关闭
  （`EBADF`）。原 `blocking_fallback_read_and_write` 改为直接用 `SharedFd::new(std)`
  构造，保持同步测试、不依赖 compio runtime。
- 验证命令与结果：
  - `cargo check -p fkv_io_compio --all-targets`（macOS）通过；
    `--target x86_64-unknown-linux-gnu --all-targets` 通过（fixed.rs 仅 Linux 编译，
    macOS 目标为 cfg 空编译，故必须加 Linux 目标才真正校验）。
  - `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`
    两个目标均 0 警告。
  - `cargo nextest run -p fkv_io_compio --no-fail-fast`（宿主）：36 passed。
  - Linux 运行验证（宿主 macOS 无法直接跑 Linux 目标）：aarch64-unknown-linux-gnu
    交叉构建集成测试，在 `debian:bookworm-slim`（arm64, io_uring 可用）容器内
    9 个集成测试二进制共 41 项全通过；x86_64 目标 lib 单测（含新增 guard 用例）
    在 amd64 容器内 2 passed。

后续可选：引入 IO 超时 / select 取消后，本 guard 已覆盖 SQE 悬垂风险，无需再改。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/fixed.rs:42 type FdGuard = SharedFd<File>`；
  `FixedRead.fd: FdGuard`（`:352`/`:364`）、`FixedWrite.fd: FdGuard`
  （`:439`/`:451`）；`create_entry`/`call_blocking` 均经 `self.fd.as_raw_fd()`
  取号（`:400`/`:409`/`:487`/`:495`）。
- `lib.rs` 三处构造点传 `file.to_shared_fd()`：`submit_fixed_write:1457`、
  `read_segment_fixed:2477`、`write_segment_fixed:2545`。
- 测试 `fixed.rs:587 fd_guard_outlives_source_file_drop`：源句柄 drop 后
  `fcntl(F_GETFD)` 仍成功，op drop 后 fd 关闭。
