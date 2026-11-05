# eng-76a manifest 快照改迭代器入参 + 去私有 varint + 长度 try_from

## 位置

- `crate/format/fkv_manifest/src/lib.rs:147-156` `SnapshotArgs`
- `crate/format/fkv_manifest/src/lib.rs:222-262` `ManifestBuilder::snapshot`
- `crate/format/fkv_manifest/src/lib.rs:297-307` 私有 `append_varint32/64`

## 方案

1. `SnapshotArgs` 去掉 `mapping_value_li` / `segment_value_li`，只留
   `root_id`、`ttl_root`、`max_fp_id`、`dict_bytes`、`branch_metadata`、`max_segment_fp_id`。
2. 新增泛型静态分派入口（禁 dyn）：

   ```rust
   pub fn snapshot_with<M, S>(
     &mut self,
     args: SnapshotArgs<'_>,
     mapping_value_li: M,
     segment_value_li: Option<S>,
   ) -> Result<Vec<u8>>
   where
     M: Iterator<Item = u64>,
     S: Iterator<Item = u64>,
   ```

3. `snapshot(args, mapping: &[u64], segment: Option<&[u64]>)` 保留为切片便捷入口，
   内部 `iter().copied()` 转调 `snapshot_with`。
4. 抽出 `append_varint_section(&mut self, impl Iterator<Item = u64>) -> Result<()>`：
   写 4 字节 LE 长度占位 → 逐个 varint → 回填长度；mapping 段与 segment 段共用，消除重复。
5. 删除私有 `append_varint32/64`，改用 `fkv_codec::{append_varint32, append_varint64}`。
6. `dict_bytes.len()`、mapping 段长度、branch 段长度、segment 段长度统一
   `u32::try_from(len).map_err(|_| KvError::OutOfSpace)?`，不再 `as u32` 静默截断。

## 验收

- `cargo check -p fkv_manifest` 通过。
- 盘面字节与旧实现逐字节一致（由 76c 回归测试锁定）。
- `cargo clippy -p fkv_manifest --no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

- `SnapshotArgs` 去掉 `mapping_value_li` / `segment_value_li`，只剩元信息字段。
- 新增 `snapshot_with<M: Iterator<Item=u64>, S: Iterator<Item=u64>>`：泛型静态分派，逐值流式编码，
  无中间 `Vec<u64>`；`snapshot(args, &[u64], Option<&[u64]>)` 保留为切片便捷入口。
- 抽出 `append_varint_section`，mapping / segment 两段共用，消除重复编码。
- 删除私有 `append_varint32/64`，改用 `fkv_codec::{append_varint32, append_varint64}`，去掉
  `put_varint*(...).unwrap_or(0)` 的静默零字节路径。
- 长度字段（dict / mapping / branch / segment）统一 `length_u32`，超 u32 返回 `KvError::OutOfSpace`。
- 验证：`cargo check -p fkv_manifest`、`cargo clippy -p fkv_manifest -p fkv_mapping --no-deps
  --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_manifest/src/lib.rs:127-134` SnapshotArgs 仅元信息字段；`:218` snapshot_with<M: Iterator, S: Iterator>；`:255` append_varint_section（两段共用）；`:316` length_u32（超 u32 → OutOfSpace）；私有 varint 已删、改用 `fkv_codec`（:8 导入）。字节回归由 76c 锁定。
