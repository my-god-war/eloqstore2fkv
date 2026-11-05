# io-21 data / segment 文件从未以 O_DIRECT 打开，全部读写走 page cache，与 C++ 强制 O_DIRECT 行为不一致

## 位置
- Rust: `crate/io/fkv_io_compio/src/lib.rs:615`（`open_read_file` 用 `OpenOption::new().read(true).open()`）、`:630-636`（`open_write_file` 无 `custom_flags`）、`:646-654`（`create_file` 用 `File::create`）、`:666-671`（`open_for_sync`）。全 crate `grep O_DIRECT|custom_flags` 零命中
- C++: `eloqstore/src/async_io_manager.cpp:1736-1740`（`OpenOrCreateFD`：`O_RDWR | (direct ? O_DIRECT : 0) | (create ? O_CREAT : 0)`，data/segment/manifest 全部 `direct=true`）、`:2154`（`CreateFile`：`O_CREAT | O_RDWR | O_DIRECT`）、`:2893`（`WriteSnapshot`：`O_CREAT | O_TRUNC | O_RDWR | O_DIRECT`）、`:2199-2200`（`OpenFile` 断言 data/segment 必带 `O_DIRECT`）
- compio: `compio-fs-0.12.1/src/open_options/unix.rs:55`（`OpenOption::custom_flags(i32)` 已暴露，`rustix OFlags` 可加 `O_DIRECT`）；`compio-fs-0.12.1/src/file.rs:202-207`（`read_at` 单次 `IORING_OP_READ`，buffered/O_DIRECT 透明）
- Rust 对齐 buffer 已就绪：`crate/core/fkv_buf/src/lib.rs:11,30-32`（`PAGE_ALIGN=4096`，`Layout::from_size_align` 分配）

## 现状
`CompioStore` 所有打开路径（读句柄、写句柄、create、sync 句柄）均不带任何 flag：data 页、segment、manifest 全部经 page cache 读写。`read_page` 从 `BufPool` 取 `Vec<u8>`（普通堆内存，非 4 KiB 对齐）再 `copy_from_slice` 到 `dst`。设计文档 `todo.md §2/§4.1` 声称 `IouringMgr → fkv_io_compio: FD LRU、O_DIRECT、合并写`，实现未落地 O_DIRECT。

## C++ 实现
C++ 数据面（data/segment/manifest）恒定 `O_DIRECT`：
- 读页 = 1 次 `IORING_OP_READ`（fixed file），绕过 page cache，冷点读恰好 1 次磁盘 IO（`todo.md §1.1` 验收项）；
- 写页直接落盘缓冲，`fdatasync` 语义就是 C++ 的 durability anchor；
- buffer 一律 `posix_memalign(page_align)`（`BootstrapRing:276`）且 offset/len 均 4 KiB 对齐（`ConvFilePageId:1953` 断言）。

## 差异与风险
- **冷点读 p99 / RSS 与验收口径失真**：Rust 读页命中 page cache 时偏快、未命中时多一次 page cache 拷贝（内核 page → 用户 buffer → `dst` 双重拷贝，另见 io-03）；大 store 下 page cache 被 data/segment 灌满，RSS 上限与驱逐行为与 C++ 完全不同，`§7.6 RSS ≤ 1.1×` 无法对比。
- **写路径 fsync 语义偏移**：buffered write + `fdatasync` 与 O_DIRECT + `fdatasync` 的落盘顺序/脏页回写窗口不同，crash 测试（10k kill point）结论与 C++ 不可比。
- **写放大差异**：C++ O_DIRECT 下小页写直接按页落盘；Rust buffered 写触发内核 4 KiB 以下 RMW 与回写合并，`nvme` 层面写放大与 C++ 不同。
- 与 fixed buffer 路径不一致：`fixed.rs` 注册的是 4 KiB 对齐 `AlignedBuf`（可按 O_DIRECT 用），普通路径却是非对齐 `Vec<u8>`，一旦补开 O_DIRECT 会立刻 EINVAL —— 两条路径的 buffer 策略必须统一。

## 对齐建议
- `open_read_file` / `open_write_file` / `create_file` / `open_for_sync` 对 data/segment（及 C++ 同样 O_DIRECT 的 manifest）追加 `custom_flags(libc::O_DIRECT)`（非 Linux `cfg` 下不加，保持现状）。
- 读页缓冲改用 `fkv_buf::AlignedBuf`（或带 `is_aligned(4096)` 断言的池），`read_page`/`write_page`/`write_segment`/`write_file` 的 buffer 与 offset 保证 4 KiB 对齐（当前 offset 已对齐，buffer 未对齐）。
- 非对齐读（`read_whole` 的 manifest 任意长度、standby 尾部）保留 buffered 句柄或尾部 `pwrite` 兜底，对齐 C++ `WriteSnapshot` 的整文件对齐写思路。

## 性能优化建议
- O_DIRECT 打开后，读页可零拷贝直落 `dst`（`dst` 来自 page cache 槽位本身即对齐内存），消除 io-03 描述的 `BufPool` 双重拷贝；
- 配合 io-26 的 registered write buffer，写页也走 fixed buffer 直写，CPU 拷贝归零。

## 严重度
P1（行为不一致 / 性能验收口径失真）

## 审查结论（2026-11-04）

**采纳（P1）**。

- `open_read_file` / `open_write_file` / `create_file` / `open_for_sync` 均未带
  `O_DIRECT`，data / segment / manifest 全走 page cache，与基线不一致。
- 开启 O_DIRECT 需同时满足：Linux `custom_flags(O_DIRECT)`、buffer 4 KiB 对齐
  （`BufPool` 当前是普通 `Vec`）、offset/len 对齐、短读重试（io-22）。
- 与 io-03（零拷贝）、io-22（错误分类）、io-27（注册写缓冲）是一组。
- 阻塞：`fkv_io_compio` 正由 io-06 修改，完成后按 io-03 → io-21 → io-22 → io-27 顺序做。

## 进展（2026-11-04，数据页部分完成）

- `CompioStore::with_direct_io(true)`：仅当 `page_size` 为 4 KiB 整数倍时生效；
  默认关闭，不影响现有测试。
- Linux 下 `data_*` 文件打开自动加 `O_DIRECT`（manifest / segment 仍 buffered）。
- 新增 `DirectBuf`（`AlignedBuf` + `IoBuf/IoBufMut/SetLen`）：
  - `read_page` 直读对齐缓冲，checksum 通过后拷入 `dst`；
  - `write_page` / `write_page_li` 从对齐缓冲直写，合并组仍并发提交。
- Linux 交叉编译验证通过：
  `./docker-test.sh -p fkv_io_compio`，含新增
  `hardening::direct_io_page_roundtrip`（4096 页写读一致）。

未做：segment / manifest 的 O_DIRECT（segment 尾段长度非 4 KiB 对齐，
需统一补零/对齐策略）；`fkv_buf::AlignedBuf` 池化读缓冲（当前每次 new）。

## 审查结论（2026-11-04，segment 收尾）

**合理，剩余范围收窄为 segment 文件**。

- data 页部分已落地（见上「进展」），本轮只补 segment，避免重复实现。
- manifest 保持 buffered，符合本 issue「非对齐读保留 buffered 句柄」的建议：
  `ManifestIo::append_manifest` 对外接受任意长度 record（测试直接传 `b"one"`），
  C++ 侧由 `ManifestBuilder` 保证 record 4 KiB 对齐并在 `AppendManifest` 断言；
  Rust 若在 IO 层补零会改变 `read_manifest` 逐字返回语义，需另立 issue 统一
  manifest framing 契约，不在本轮扩范围。
- segment 需要同时修三处，否则 O_DIRECT 直接 `EINVAL`：
  1. `is_direct_path` 只匹配 `data_`，segment 打开仍是 buffered；
  2. `read_segment` 用任意长度 `Vec`，需改为页对齐长度读入 `DirectBuf` 再拷前缀；
  3. `read_segment_fixed` 的注册读缓冲在 `offset` / `len` 非 4 KiB 对齐时不能走
     `O_DIRECT`，需回落 `read_segment`。
- 验收：Linux 下新增 segment 对齐读写往返测试（含非对齐 `dst.len()`），
  macOS 无 `O_DIRECT`，测试用 `cfg(target_os = "linux")` 隔离。

## DONE（2026-11-04）

实现要点（`crate/io/fkv_io_compio/src/lib.rs`）：

- `is_direct_path` 扩展匹配 `segment_`；新增 `direct_segment()`，
  要求 `segment_size` 为 `fkv_buf::PAGE_ALIGN` 整数倍，否则保持 buffered。
- `read_segment`：O_DIRECT 时按 `len.next_multiple_of(PAGE_ALIGN)` 读入
  `DirectBuf`，再拷有效前缀到 `dst`；`read_len < len` 判 `IoFail`。
- `write_segment`：沿用整段定长 + 尾部补零语义，direct 分支改用对齐
  `DirectBuf`（`segment_size` 整页），经 `write_chunk` 归还对齐池。
- `read_segment_fixed`：direct 时仅当 `offset` / `len` 都页对齐才走注册
  `FixedRead`，否则回落 `read_segment`，避免 `EINVAL`；buffered 行为不变。
- `read_whole` / `write_whole` 增加 `direct` 参数：`FileIo` 读写 data 文件
  在 O_DIRECT 下用对齐缓冲与对齐长度；manifest 仍传 `false`（buffered）。
- `with_direct_io` 文档改为 data / segment 生效，manifest 明确保持 buffered。
- 新增 `DIRECT_POOL_BUDGET`（16 MiB）：segment 读长度按值大小分桶，归还对齐
  缓冲时按全池字节数封顶，避免常驻内存被小桶放大。

测试：

- `crate/io/fkv_io_compio/tests/hardening.rs::direct_io_segment_roundtrip`
  （`cfg(target_os = "linux")`）：16 KiB 段整段写、4099 字节非对齐长度读、
  跨段（fp id 3）读写一致。

验证命令与结果：

- `cargo check -p fkv_buf -p fkv_io_compio`：通过。
- `cargo nextest run -p fkv_buf -p fkv_io_compio`：34 passed（macOS，
  Linux-only 测试跳过）。
- `cargo clippy -p fkv_buf -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`：零警告。
- `CARGO_TARGET_DIR=/tmp/fkv-target-aarch64-unknown-linux-gnu cargo clippy -p fkv_io_compio --target aarch64-unknown-linux-gnu --no-deps --all-features --all-targets -- -D warnings`：零警告。
- `./docker-test.sh -p fkv_io_compio`：hardening 12 passed，含
  `direct_io_segment_roundtrip`。

遗留：

- manifest 仍 buffered。C++ `ManifestBuilder` 保证 record 4 KiB 对齐并在
  `AppendManifest` 断言；Rust 的 `ManifestIo` 对外接受任意长度 record，
  要开 O_DIRECT 必须先统一 framing 补零 / 截断契约，另立 issue。

## 复审（2026-11-05 r6）

通过。

- `is_direct_path` 匹配 `data_` + `segment_` 且 `direct_segment()` 要求
  `segment_size` 为 `PAGE_ALIGN` 整数倍（`fkv_io_compio/src/lib.rs:790-806`）；
  `with_direct_io` 要求 `page_size` 4 KiB 倍数（`:877-881`），manifest 文档明确
  保持 buffered（`:875-876`）。
- 打开路径 `apply_direct_flag`：`open_read_file:1213` / `open_write_file:1230` /
  `open_for_sync:1271`。
- `read_segment` direct 分支按页对齐长度读入 `DirectBuf` 拷有效前缀
  （`:2325-2348`）；`read_segment_fixed` direct 仅 offset/len 均页对齐才走注册读，
  否则回落（`:2462-2496`）；`read_whole(direct)`（`:1563-1590`）；
  `DIRECT_POOL_BUDGET=16 MiB` 归还封顶（`:90`、`:1296-1309`）。
- 测试：`tests/hardening.rs:301 direct_io_segment_roundtrip`（16 KiB 段整段写 +
  4099 非对齐长度读 + 跨段 fp id 3，linux-gated）、`:285 direct_io_page_roundtrip`。
- 遗留（manifest O_DIRECT 需 framing 契约另立 issue）未立项，与 md 记录一致。
