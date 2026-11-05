# perf-78 scan 预取仅 1 页且用 Pin<Box<dyn Future>> 每页堆分配，违反热路径禁 dyn

## 位置
- Rust: `crate/engine/fkv_task_scan/src/lib.rs:181-200`（单页预取 + `Pin<Box<dyn Future>>`）、`lib.rs:293-310`（`drain_prefetch`/`poll_page_io` 同样 `dyn Future`）、`lib.rs:183`（`spare` 每页 `fill(0)`）
- C++: `include/tasks/scan_task.h:27-29`、`types.h:58`（`kDefaultScanPrefetchPageCount = 6`）、`scan_task.h:61-64`（`prefetched_leaf_ids_` 固定数组）、`src/tasks/scan_task.cpp`（多页预取）

## 现状
- fkv scan 注释明确「数据页是单向链表，只能预取 1 页」（`lib.rs:182`），每页只提前提交 1 个 `read_page`。
- 预取 future 用 `Pin<Box<dyn Future<Output=Result<()>> + '_>>`（`lib.rs:193`），**每页一次堆分配**（`Box::pin`）。
- `poll_page_io`/`drain_prefetch` 签名同样 `Pin<Box<dyn Future>>`（`lib.rs:294`、`lib.rs:309`）。
- `spare` 每页 `fill(0)`（`lib.rs:198`）。

## C++ 实现 / 目标口径
- C++ `ScanTask` 预取 `kDefaultScanPrefetchPageCount = 6` 页（`types.h:58`），用 `std::array<PageId, max_read_pages_batch>` 固定数组（`scan_task.h:61-62`），无堆分配。
- 多页预取把 IO 与解码更充分重叠，scan 吞吐更高。
- todo.md 硬约束 4：热路径禁 `dyn`，用 enum dispatch / 泛型。

## 差异与风险
- **预取深度不足**：Rust 1 页 vs C++ 6 页，scan 吞吐验收 ≥ 0.9× 可能不达标——IO 与解码重叠窗口小，磁盘延迟暴露更多。
- **违反热路径禁 dyn**：`Pin<Box<dyn Future>>` 每页堆分配 + 虚调用，scan 每页一次分配与 vtable 跳转，是 todo.md 明令禁止的。
- **每页 fill(0)**：`spare.fill(0)` 对 4KB 页是多余 memset，浪费 CPU。

## 对齐建议
- 预取深度对齐 C++：至少 2-4 页（`kDefaultScanPrefetchPageCount=6` 的子集），用固定数组/环形 buffer 持有多页 spare。
- 消除 `dyn Future`：用 `Option<Pin<Box<impl Future>>>` 或 enum `PrefetchOp` 泛型化，避免每页堆分配与虚调用。
- `spare` 不需 `fill(0)`：`read_page` 写满整页，fill 是冗余。
- 预取 future 复用：跨页复用同一 future 槽，避免重复 `Box::pin`。

## 性能优化建议
- 多页预取用 `io_uring` 批量提交（SQE 数组），一次系统调用提交 N 页读，减少 syscall 与 ring 开销。
- 预取 buffer 用 `fkv_buf` 对齐池化 buffer，避免每 scan 分配。

## 严重度
P1（口径不一致/功能缺失，scan 吞吐验收风险）

## 审查结论（2026-11-04）

逐条核验：

- 行号核实：`crate/engine/fkv_task_scan/src/lib.rs` 现为 319 行。
  `:183` spare 分配、`:193` `Pin<Box<dyn Future>>`、`:198` `spare.fill(0)`、
  `:293-296` / `:308-311` 两个 helper 的 `dyn` 签名与 issue 一致；`:165` 另有一处
  同类冗余 `page.fill(0)`。
- **`dyn` + 每页堆分配**：成立。`read_page` 返回 `impl Future`，完全可以用
  `std::pin::pin!` 固定在栈上、泛型 helper 接收 `Pin<&mut F>`，零堆分配、无 vtable。
  与 `todo.md` 硬约束 4（热路径禁 `dyn`）、SKILL.md 第 9 条一致。→ 采纳。
- **`spare.fill(0)` 冗余**：成立。`fkv_io_mem::read_page` 覆写 `dst[..src.len()]`
  且 `write_page` 强制整页，compio `read_page` 覆写 `dst[..page_size]`，
  两处 `fill(0)`（`:165`、`:198`）都在整页读之前，可删。→ 采纳。
- **预取深度 1 页 → 6 页**：不成立，不采纳。扫描遍历顺序来自 data page
  `next_page_id` 单向链表，第 N+1 页的 id 只能从第 N 页 header 得到，
  在当前遍历契约下预取深度上界就是 1 页。C++ 能预取 6 页是因为它沿
  `index_stack_` 推导叶子顺序（eng-65 已确认并决定保留链式遍历）。
  要解除该限制必须先重写扫描器为 index 栈推进，属功能级改造，
  与 eng-65 结论冲突，且会改变扫描语义（本 issue 验收要求语义不变）。
- **io_uring 批量提交 / fkv_buf 池化**：不适用。批量提交需要一次拿到多个页 id，
  同上受链式遍历限制；`spare` 每 scan 一次分配，不在热循环内，改池化收益低于复杂度。
- C++ 源码目录 `cpp/eloqstore` 为空，`:27-29` / `types.h:58` 无法二次核验，
  沿用 issue 与 eng-65 的核验记录。

判定：部分合理。只落地「消除 `dyn`/每页堆分配」+「删冗余 `fill(0)`」，
预取深度维持 1 页并在注释里写明原因。拆分：

- `issue/todo/perf-78a-scan-prefetch-stack-pinned-no-dyn.md`

## DONE（2026-11-04）

- 采纳部分由 `perf-78a` 落地：消除 `Pin<Box<dyn Future>>` 与每页堆分配、
  删两处冗余 `fill(0)`，扫描语义不变。
- 预取深度维持 1 页（链式遍历上界），6 页预取与批量提交不采纳，理由见上「审查结论」。

## 复审（2026-11-05 r6）

通过（部分采纳拆分记录，落地在 perf-78a）。

- 「预取深度 1 页 → 6 页不采纳」的理由（data page 单向链表遍历，第 N+1 页
  id 只能从第 N 页 header 获得，C++ 6 页预取依赖 index 栈 = eng-65）经查
  `fkv_task_scan` 现状成立：`pin!(async {...})` 预取仍是单页深度。
