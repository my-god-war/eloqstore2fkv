# eng-76b store 四处 snapshot 统一走 snapshot_record，零中间 Vec

## 位置

- `crate/engine/fkv_store/src/lib.rs:719-752` `snapshot_record`
- `crate/engine/fkv_store/src/lib.rs:1002-1016` `put_at_inner` first_manifest 分支
- `crate/engine/fkv_store/src/lib.rs:1090-1106` `append_mapping_log` 超限/字典脏分支
- `crate/engine/fkv_store/src/lib.rs:1367-1412` `truncate`

## 方案

1. `snapshot_record` 改为 `ManifestBuilder::new().snapshot_with(...)`，
   mapping 直接传 `mapper.tbl().iter_values()`（用 `Option::into_iter().flat_map` 统一类型，
   表不存在时为空迭代器），segment 传 `Option::map(iter_values)`。
   `max_fp_id` / `max_segment_fp_id` 缺省值仍取 mapper 的 `max_fp_id()`。
2. `put_at_inner` first_manifest：删掉内联 snapshot 构造，直接
   `self.snapshot_record(tbl, new_root, new_ttl_root, None, None, &branch)?`。
3. `append_mapping_log`：同上，`self.snapshot_record(tbl, root_id, ttl_root_id, None, None, &branch)?`。
4. `truncate`：删掉 `dict_bytes.to_vec()` 与内联构造，改为
   `self.snapshot_record(tbl, PageId::MAX, PageId::MAX, None, None, &branch)?`；
   注意 mapper 的可变借用需在调用前结束。
5. `MappingTbl::to_vec` 在 store 不再被调用；若全仓无调用则删除（保留 `value_li` 兼容测试）。

## 验收

- 四处 `tbl().to_vec()` 归零。
- `cargo check -p fkv_store` 通过。
- 盘面字节不变（76c 回归 + 现有 store 测试）。

## DONE（2026-11-04）

- `snapshot_record` 改走 `snapshot_with`，mapping 用
  `Option::into_iter().flat_map(iter_values)`、segment 用 `Option::map(iter_values)`，
  统一迭代器类型且零中间 Vec。
- `put_at_inner` first_manifest、`append_mapping_log` 滚动 snapshot、`truncate` 三处内联构造
  全部改为调用 `self.snapshot_record(...)`；truncate 删掉 `dict_bytes.to_vec()`。
- `MappingTbl::to_vec` 全仓无调用，已删除（`value_li` 保留给测试/兼容）。
- 验证：`cargo check -p fkv_store --all-targets`、clippy lib 零警告、nextest 94 用例全绿。

## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:1079-1112` snapshot_record 走 `ManifestBuilder::snapshot_with` 流式迭代器（注释标 eng-76）；`MappingTbl::to_vec` 已删（`fkv_mapping/src/lib.rs` 仅存 `iter_values`:244）。注：DONE 里"`value_li` 保留"一句已过时——后续清理把该便捷方法也删了，测试统一用 `iter_values().collect()`，无行为影响。
