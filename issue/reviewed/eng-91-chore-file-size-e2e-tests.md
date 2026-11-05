# eng-91 chore.cpp file_size 端到端测试补齐

## 现状

C++ `tests/chore.cpp` 有 5 个 file size 用例：不同 page size（4K~32K）× 写入后校验文件尺寸、不同 `pages_per_file_shift`（8~16）下的文件尺寸、海量条目注入、`ValidateFileSizes` 全组合校验、failed external reopen 完成 pending reopen waiters（后者已由 runtime reopen 测试覆盖）。

Rust 侧没有对位测试：`MemStore` / `CompioStore` 均未暴露 per-file 字节查询（C++ `ValidateFileSizes` 逐文件比对 `file size == pages_per_file * page_size` 上取整后的期望值）。

## 方案

1. `fkv_io::PageIo` 或 `MemStore` 增加 `file_byte_len(tbl, file_id) -> Result<u64>`（查询口径与 C++ `ValidateFileSizes` 一致：append 模式下文件尾 = 已写 file page 的物理尾）。
2. 新增 `crate/engine/fkv_store/tests/file_size.rs`：
   - page_size ∈ {4K, 8K, 16K, 32K} × shift=10，写 100 条后校验尺寸；
   - shift ∈ {8, 10, 12, 14} × page_size=4K，同上；
   - 对照 `COMPAT_BASELINE`，32K/64K 大页可按内存预算裁剪。

## 验收

- `cargo nextest run -p fkv_store file_size` 全过；
- 不改变任何生产行为（只加查询 API）。

## 阻塞依赖

无；`fkv_io` trait 扩面需同步实现 `fkv_io_mem` / `fkv_io_sim` / `fkv_io_compio` 三处。

## 处置（2026-11-05）

P3 测试/组织性工作，不阻塞功能与正确性；当前测试覆盖已由 `fkv_store` 既有 file
size 断言与各 crate 内联/集成测试承担，移入 `issue/reviewed/` 作为后续可选清理项。
