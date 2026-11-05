# io-43 P3：`evict_table`（detach 驱逐）不清 dirty / dirty_dir / manifest_offset / preallocated

来源：第 5 轮 io/compio lane 侦查（子代理摘录，协调者抽查了对照函数）。

## 证据

`crate/io/fkv_io_compio/src/lib.rs:1796-1800`：

```rust
fn evict_table(&self, tbl: &TableIdent) {
  let dir = self.part_dir(tbl);
  self.fd_cache.borrow_mut().forget_under(&dir);
  self.dir_index_map.borrow_mut().remove(tbl);
}
```

对照同文件 `try_remove_empty_partition_dir`（`:2486-2491`）清理四个集合、
`forget_fd`（`:1191-1196`）连带清 offset/preallocated——evict 路径漏了：
`dirty[tbl]`、`dirty_dir`、`manifest_offset`（含该分区）、`dir_ready`。

## 问题

1. 多 shard 反复 attach/detach 下长驻泄漏（dirty 集、offset 表增长）。
2. 同名表重新 attach 后旧 `manifest_offset` 仍命中：`append_file_at`
   （`:1548` 一带）直接用缓存偏移跳过 fstat，若期间文件被外部重建（standby /
   运维），append 落错偏移。

## 目标

evict 路径与 `try_remove_empty_partition_dir` 的清理集合对齐：fd 缓存、
dir 索引之外，清掉该表的 dirty 项、manifest_offset 项（dirty_dir 按 dir 前缀）
与 preallocated 记录。可抽 `fn forget_table_state(&self, tbl)` 一处定义两个
调用点共用。

## 验收

- 测试：attach → 写 → detach(evict) → 再 attach 同名表，断言 dirty/offset
  表不含残留；新 append 偏移来自 fstat 而非旧缓存。
- `cargo nextest run -p fkv_io_compio`。

## 依赖 / 重叠

- eng-73a（done，store 层表状态）/ sto-61a（done，shard 生命周期）不涉及 IO 层。
- eng-37（done，detach 驱逐）引入本函数时的遗漏。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上；`append_file_at` 缓存偏移用法开发时验证。

## DONE（2026-11-04）

判定：合理。`CompioStore::evict_table` 只清 fd 缓存与 dir 索引，`dirty` /
`dirty_dir` / `manifest_offset` / `segment_preallocated` / `dir_ready` /
`manifest_dir_synced` 全部残留；同名表重新 attach 会复用旧 manifest 偏移。

改动（`crate/io/fkv_io_compio/src/lib.rs`）：

- 新增 `fn forget_table_state(&self, tbl)`：按分区目录前缀清 fd / dir 索引 /
  dir_ready / dirty / dirty_dir / manifest_offset / segment_preallocated /
  manifest_dir_synced。
- `PageIo::evict_table` 与 `try_remove_empty_partition_dir` 共用该函数，清理集合不再漂移。
- 新增 `crate/io/fkv_io_compio/tests/evict.rs`：evict 后 `dirty_file_num` 归零；
  外部截短 manifest 后 append 走 fstat（结果 `bbcc` 而非留下空洞）；segment
  预分配标记作废后重新 `set_len` 到整文件长度。

验证：

- `cargo nextest run -p fkv_io_compio`：38 passed（含 2 个新测试）
- `cargo check -p fkv_cloud` rc=0
- `cargo nextest run -p fkv_cloud -p fkv_store --no-fail-fast`：142 passed
- `cargo clippy -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` rc=0

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:1839-1858 forget_table_state`：清 fd 缓存、dir 索引、
  dir_ready、dirty、dirty_dir、manifest_offset（按分区目录前缀 retain）、
  segment_preallocated、manifest_dir_synced 共 7 项；
  `evict_table`（`:1911-1912`）与 `try_remove_empty_partition_dir`（`:2630` 一带）
  共用该函数。
- 测试 `tests/evict.rs:35 evict_table_clears_dirty_and_manifest_offset`：evict 后
  `dirty_file_num == 0`；外部把 manifest 截短为 `bb` 后 append 得 `bbcc` 而非
  留空洞（证明偏移来自 fstat 而非旧缓存，`:57-62`）；第二个测试覆盖 segment
  预分配标记作废后重新 set_len。
