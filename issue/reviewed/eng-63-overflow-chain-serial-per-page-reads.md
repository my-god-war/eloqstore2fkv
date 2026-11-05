# eng-63 overflow 链重组逐页串行读 + 逐页 fill(0)：C++ ReadPages 一批多 SQE 并发提交

## 位置
- Rust: `crate/engine/fkv_task_read/src/lib.rs:29-47`（`read_overflow`：`for pointer in pointer_li { ... io.read_page(...).await?; }` 批内逐页串行；每页 `page.fill(0)` 清零后即刻被整页覆写）；`PageIo` trait 无批量读接口（`crate/io/fkv_io/src/lib.rs:28` 仅单页 `read_page`）
- C++: `src/tasks/task.cpp:199-221`（`GetOverflowValue`：`IoMgr()->ReadPages(tbl_id, page_ids, pages)` 一批（≤`overflow_pointers`=16，上限 128，`kv_options.h:23-24`）一次提交，批末页带下一批指针）；`src/async_io_manager.cpp:867-907`（批内每页各压一个 SQE 后统一等待，短读逐页重试）

## 现状
N 页 overflow 值（64KB 页 × 16 页 ≈ 1MB）点读延迟 = N 次串行 RTT；扫描中每个 overflow 条目同样走此路径。另付每页一次无意义 memset。

## C++ 实现
批内页并发提交，一次等待；跨批仍串行（链式依赖决定，两边一致）。

## 差异与风险
大 value 点读/扫描吞吐与延迟显著劣化；与 core-17（large segment 串行）、io-03（IO 层拷贝）、io-06（segment 批量写）均不同对象 —— 本条是 **data overflow 页**。

## 对齐建议
- `PageIo` 增 `read_pages(tbl, file_page_id_li, page_buf_li)`（compio 侧可用 `futures::future::try_join_all` 并发单页读，或合并同文件偏移为 readv）；
- `read_overflow` 批内并发、去掉 `fill(0)`（`read_page` 全页覆写语义已保证）。

## 性能优化建议
1MB overflow 值读延迟从 16 RTT 降为 ≈1 RTT（本地 NVMe 队列深度受益更明显）。

## 严重度
P1（大 value 读路径）

## 核验记录（2026-11-04）
Rust 串行循环与逐页清零实读确认；C++ `task.cpp:199-225` ReadPages 批语义实读确认（批内一次提交、末页链指针）。

## 审查结论（2026-11-04）

核验结果：issue 描述与实读一致。

- `crate/engine/fkv_task_read/src/lib.rs:22-43`：`read_overflow` 批内 `for pointer` 串行 `read_page`，且每页 `page.fill(0)` 后随即被整页覆写。
- `crate/io/fkv_io/src/lib.rs`：`PageIo` 只有单页 `read_page`，无批量读；`write_page_li` 已有「默认串行 + 后端覆盖」先例，可直接沿用。
- C++ 源不在本仓（`cpp/` 仅 `shim/README.md` 与构建目录），沿用 issue 原有核验记录，不再重核。
- 修正/收敛：`fill(0)` 可安全删除，mem 与 compio 后端的 `read_page` 都写满 `page_size`（mem 由 `write_page` 强制 `src.len() == page_size`）。批大小天然受 `overflow_pointers` 约束（≤128），无需额外全局上限；按层批量即可，不整链一次性读取。

判定：合理，拆分实现。

- `issue/todo/eng-63a-page-io-read-page-li.md`
- `issue/todo/eng-63b-read-overflow-batch-read.md`

## 复审（2026-11-05 r6b）

通过（父条目收口确认，原缺陷已消除）。

- `PageIo::read_page_li` 批量读接口已落地：`crate/io/fkv_io/src/lib.rs:55`（与 `:80 write_page_li` 平行）。
- `read_overflow` 已改批读：`crate/engine/fkv_task_read/src/lib.rs:255`（`io.read_page_li(tbl, &file_page_id_li, &mut buf_li)`），逐页串行 `for pointer { read_page }` 与逐页 `fill(0)` 均已不存在（该文件 grep 无 `fill(0)`）。
- 两个拆分件 `eng-63a` / `eng-63b` 均已在 `issue/reviewed/`，本条关闭。
