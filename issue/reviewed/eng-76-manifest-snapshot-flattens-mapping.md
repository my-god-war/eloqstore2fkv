# eng-76 每次 manifest snapshot 都把整张 mapping 摊平成 `Vec<u64>`；4 处 snapshot 构造再各写一遍

## 位置

- `crate/format/fkv_manifest/src/lib.rs:222-262`：`ManifestBuilder::snapshot` 的参数是
  `mapping_value_li: &[u64]`，只能逐值遍历一个已摊平的切片。
- `crate/engine/fkv_store/src/lib.rs` 四处构造 snapshot 时都先 `tbl().to_vec()`：
  - `snapshot_record` `:679-712`（`:690` / `:698`）
  - `put_at_inner` `:955-977`（`:963` / `:972`）
  - `append_mapping_log` `:1044-1066`（`:1052` / `:1061`）
  - `truncate` `:1333-1354`（`:1340` / `:1349`）
- `crate/core/fkv_mapping/src/lib.rs:190-192`：`MappingTbl::to_vec` 就是
  `iter_values().collect()`；`:174-180` 的 `iter_values()` 已经是零拷贝分块迭代器。
- `crate/format/fkv_manifest/src/lib.rs:297-306`：私有 `append_varint32/64` 又实现了一遍
  `fkv_codec::append_varint32/64`，且 `put_varint64(...).unwrap_or(0)` 在“不可能是 0”的
  路径上静默写出零字节。

## 现状

- 每次 snapshot（首次写、manifest 超限转 snapshot、compaction、truncate、archive、
  create_branch）都要：
  1. 分配 `mapping_len * 8` 字节的临时 `Vec<u64>`；
  2. 把分块 `MappingTbl` 全量复制进去；
  3. 再遍历这个 Vec 编码成 varint 写入 manifest buffer。
  第 1、2 步纯属中间层，manifest buffer 才是最终产物；C++ 直接从 `MappingSnapshot` 序列化。
- 典型量级：100 万逻辑页 → 每次 snapshot 多一次 8 MiB 分配 + 8 MiB memcpy；manifest 本身
  也约 8 MiB 时，等于把 snapshot 的 CPU/带宽翻倍。manifest 超限滚动时每 8 MiB 日志触发一次。
- 四处构造还各自重复“取 mapper / segment_mapper、取 max_fp_id、取 dict_bytes”的样板；
  `snapshot_record` 已经是现成 helper，但 `put_at_inner` 与 `append_mapping_log` 仍内联重写。
- `dict_bytes.len() as u32`、`mapping_len as u32`、`branch.len() as u32` 均为静默截断；
  当前受 manifest_limit 约束不会越界，但格式层应显式 `u32::try_from(...)?`。

## C++ 实现

`include/storage/root_meta.h:61-68` 的
`Snapshot(root_id, ttl_root, const MappingSnapshot *mapping, max_fp_id, dict_bytes, ...)`
直接接收 mapping 对象；`src/storage/page_mapper.cpp:600-620` 的
`MappingSnapshot::Serialize` 原地逐 slot 写 varint，不构造中间 `vector<u64>`。
`ManifestBuilder::Snapshot` 只是把序列化结果拼进自己的 buffer。

## 建议

1. 把 `SnapshotArgs` 里的 `mapping_value_li: &[u64]` 换成迭代器参数，提供：
   ```rust
   pub fn snapshot_with(
     &mut self,
     args: SnapshotArgs<'_>,
     mapping: impl Iterator<Item = u64>,
     segment_mapping: Option<impl Iterator<Item = u64>>,
   ) -> Result<Vec<u8>>
   ```
   `snapshot` 保留为测试辅助（内部 `slice.iter().copied()`）。Store 直接传
   `mapper.tbl().iter_values()` / `segment_mapper.tbl().iter_values()`，零中间 Vec。
   泛型迭代器是静态分派，不引入 dyn。
2. 四处构造统一走 `Store::snapshot_record(tbl, root, ttl_root, max_fp, max_seg_fp)`：
   `put_at_inner` / `append_mapping_log` 目前传的 `mapper.max_fp_id()` 正是
   `snapshot_record` 的默认值，可直接调用，删除内联样板。
3. 删除 `fkv_manifest` 私有 `append_varint32/64`，改用 `fkv_codec::append_varint32/64`
   （无失败路径，不需要 `unwrap_or(0)`）；长度字段用 `u32::try_from(...).map_err(...)`。
4. 加一个回归测试：`MappingTbl` 跨多个 chunk 时，`snapshot_with(iter_values())` 与旧
   `to_vec` 路径字节一致（保证重构不改盘面）。

## 严重度

P1（大表每次 snapshot 多一次全 mapping 分配/拷贝；manifest 滚动、compaction、truncate 都走此路径）

## 核验记录（2026-11-04）

四处 `.tbl().to_vec()` 与 `snapshot` 调用点按当前源码 grep 确认（`:690/:698`、
`:963/:972`、`:1052/:1061`、`:1340/:1349`）；`MappingTbl::to_vec` 与 `iter_values` 实读；
`ManifestBuilder::snapshot` 签名与 `append_varint*` 实读。
C++ `RootMeta::Snapshot` 签名与 `MappingSnapshot::Serialize` 引用自 `include/storage/root_meta.h:61-68`、
`src/storage/page_mapper.cpp:600-620` 实读。

## 审查结论（2026-11-04）

**判定：合理，可执行。**

实读当前源码确认（行号已漂移）：

- `ManifestBuilder::snapshot` 仍是 `mapping_value_li: &[u64]` + `segment_value_li: Option<&[u64]>`（`crate/format/fkv_manifest/src/lib.rs:222`）。
- `SnapshotArgs` 仍带两个切片字段（`:147`）。
- 四处 `.tbl().to_vec()` 仍存在：`crate/engine/fkv_store/src/lib.rs:730/738`（snapshot_record）、
  `:1003/1012`（put_at_inner）、`:1092/1101`（append_mapping_log）、`:1396/1405`（truncate）。
- 私有 `append_varint32/64` 仍在（`fkv_manifest/src/lib.rs:297-307`），
  `put_varint*(...).unwrap_or(0)` 的静默零字节路径存在。
- `MappingTbl::to_vec` 仅被上述四处调用；`iter_values` 为零拷贝分块迭代器。
- 三处长度 `as u32` 静默截断确认。

拆分为 eng-76a / eng-76b / eng-76c 三个 todo。

## DONE（2026-11-04）

- 76a/76b/76c 全部完成，明细见同目录三个 todo 文件。
- 盘面字节不变：新旧路径跨 chunk 逐字节对比测试通过。
- 全量验证：`cargo nextest run -p fkv_manifest -p fkv_mapping -p fkv_store` 94 passed；
  `cargo clippy -p fkv_manifest -p fkv_mapping --no-deps --all-features --all-targets -- -D warnings` 零警告；
  `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` 通过
  （期间 `fkv_io_compio` 由并行会话修复后复跑）。
- 连带更新 API 调用点：`fkv_recovery/tests/recovery.rs`、`fkv_task_reopen/tests/{reopen,auto_reopen}.rs`。

## 复审（2026-11-05 r6b）

通过。

- `snapshot_with` 迭代器 API：`crate/format/fkv_manifest/src/lib.rs:218`（`snapshot` 切片版保留在 `:201` 作测试辅助）。
- 私有 varint 重复已删：`fkv_manifest/src/lib.rs:8` 直接 `use fkv_codec::{append_varint32, append_varint64, ...}`。
- Store 侧零中间 Vec：`crate/engine/fkv_store/src/lib.rs` 内 `.tbl().to_vec()` 0 命中（仅剩两处 key 的 `to_vec`，与 mapping 无关）；snapshot 构造走 `:1103 snapshot_with`。
- 三个拆分件 `eng-76a/76b/76c` 均已在 `issue/reviewed/`。
