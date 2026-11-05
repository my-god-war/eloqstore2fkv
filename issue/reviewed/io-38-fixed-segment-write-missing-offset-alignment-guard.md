
## DONE（2026-11-04）

改动：`crate/io/fkv_io_compio/src/lib.rs` → `write_segment_fixed`。

- 整段 `WRITE_FIXED` 提交前新增护栏：`direct_segment()` 且 `offset` / `len` 未按
  `PAGE_ALIGN` 对齐 → `KvError::InvalidArgs`（不再交给内核摊平成 `IoFail`）。
- 与 issue 原建议「回落 buffered」不同，按本轮指令改为显式拒绝。理由：写有副作用，
  静默换路径会掩盖上层越界/错位；read 回落只影响性能，write 回落会让同一调用在两种
  IO 模式下语义分叉。
- 尾段 `len != segment_size` 仍走 `write_segment`（补零 + 对齐缓冲、段起始偏移），
  该路径 O_DIRECT 下安全，保持不变。

测试 `crate/io/fkv_io_compio/tests/fixed.rs`：

- `direct_fixed_write_rejects_misaligned_offset`：O_DIRECT + 偏移 512 + 整段 4096 →
  `InvalidArgs`；对齐偏移 0 照常落盘且往返一致。
- `buffered_fixed_write_allows_misaligned_offset`：未开 O_DIRECT 时非对齐偏移整段写
  仍然成功（护栏只作用于 direct 路径）。

验证：

- `cargo check -p fkv_io_compio` ✅
- `cargo nextest run -p fkv_io_compio --no-fail-fast` ✅（macOS 38 passed）
- `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`
  ✅ 零警告（macOS 与 `--target aarch64-unknown-linux-gnu` 各一遍）
- Linux 实跑：docker `rust:bookworm` + `--security-opt seccomp=unconfined`
  `cargo test -p fkv_io_compio` 全绿（含 io_uring 下 5 个 fixed 用例）
- 未跑全仓 `./test.sh`（按分工要求）

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:2535-2539`：`write_segment_fixed` 整段 WRITE_FIXED
  提交前护栏——`direct_segment()` 且 `offset`/`len` 未按 `PAGE_ALIGN` 对齐 →
  `KvError::InvalidArgs`，注释写明不静默回落 buffered 的理由（写有副作用）；
  尾段 `len != segment_size` 走 `write_segment` 补零路径（`:2524-2528`）。
- 测试 `tests/fixed.rs:151 direct_fixed_write_rejects_misaligned_offset`
  （偏移 512 → InvalidArgs，对齐 0 往返一致）、`:178
  buffered_fixed_write_allows_misaligned_offset`（未开 O_DIRECT 非对齐照常成功）。
