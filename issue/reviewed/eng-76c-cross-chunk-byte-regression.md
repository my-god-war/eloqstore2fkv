# eng-76c 跨 chunk 快照字节回归测试

## 位置

- `crate/format/fkv_manifest/tests/manifest.rs`（新增测试）
- `crate/format/fkv_manifest/Cargo.toml`（dev-dependency `fkv_mapping`，用 `cargo add --dev`）

## 方案

1. 构造 `MappingTbl`，长度跨多个 chunk（>= `CHUNK_SIZE * 2 + 7`，逐页写入不同编码值）。
2. 同一 `SnapshotArgs` 下分别调用：
   - 旧切片路径：`snapshot(args, &tbl.value_li(), Some(&seg_li))`
   - 新迭代器路径：`snapshot_with(args, tbl.iter_values(), Some(seg.iter_values()))`
3. `assert_eq!` 两条 record 字节完全一致；并断言 `validate_record` 通过、mapping 段长度正确。

## 验收

- 新测试通过，证明重构不改盘面。
- `cargo nextest run -p fkv_manifest -p fkv_mapping -p fkv_store` 全绿。

## DONE（2026-11-04）

- `crate/format/fkv_manifest/tests/manifest.rs` 新增
  `snapshot_with_iterator_matches_slice_across_chunks`：`MappingTbl` 长度 `CHUNK_SIZE*2+7`
  （跨 3 个 chunk），对比 `snapshot(...)` 切片路径与 `snapshot_with(iter_values())`
  两条 record 字节完全一致，并 `validate_record` 通过。
- `Cargo.toml` 用 `cargo add --dev fkv_mapping --path ...` 加入 dev-dependency。
- 验证：`cargo nextest run -p fkv_manifest` 6/6 通过；
  `cargo nextest run -p fkv_manifest -p fkv_mapping -p fkv_store` 94/94 通过。

## 复审（2026-11-05 r6）

通过。`fkv_manifest/tests/manifest.rs:70-97`：CHUNK_SIZE*2+7 跨 3 chunk，`snapshot`（切片）与 `snapshot_with`（迭代器）产物 `assert_eq!` 逐字节一致 + `validate_record` 通过；dev-dependency fkv_mapping 在 Cargo.toml:16。
