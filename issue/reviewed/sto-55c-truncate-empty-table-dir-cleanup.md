# sto-55c truncate 后空表的分区目录收尾

## 位置
- `crate/engine/fkv_store/src/lib.rs`：`Store::truncate` 写空 snapshot 并保留活跃 manifest；`Store::gc` 仅在目录完全为空时移除目录。
- `crate/io/fkv_io_compio/src/lib.rs`：`try_remove_empty_partition_dir`。

## 现状
`truncate` 后 data/segment 文件被 GC 删光，但活跃 manifest（`manifest_main_0` / branch manifest）仍在，目录非空，`Store::gc` 按保守规则不移除目录。C++ `TryCleanupLocalPartitionDir` 在 data/archive 全空时连同 manifest 一起移除目录。

直接删除活跃 manifest 的风险：`TableState.manifest_started` / `manifest_size` 仍认为文件存在，下一次写会按追加偏移写 log，缺少 snapshot 头，replay 会损坏。需要与 Store 状态机联动，不能只删文件。

## 方案
1. `Store::gc` 改为可拿到 `&mut self`（或在 `maintain` 内做收尾），检测「表逻辑为空」：root 为 `PageId::MAX` / `None`，data 与 segment mapper 无 live 项，目录无 data/segment 文件。
2. 满足条件时：删除活跃 manifest（`drop_manifest` / `remove_manifest_file`），重置 `manifest_started = false`、`manifest_size = 0`，再尝试移除空目录。
3. 若 Store 仍在 `table_map` 中，保持状态可继续写：下一次 `put` 走完整 snapshot 路径（验证 `write_manifest_record` 分支）。
4. branch 模式只删当前 branch manifest；archive / 其它 branch 不动。

## 验收
- 新增测试：truncate → gc 后分区目录消失；随后同一 Store 继续 `put` / `get` 正常；重新 `reopen` 后数据可读。
- in-flight 场景：manifest 未提交 / 有数据文件时不删目录。
- `cargo nextest run -p fkv_store --no-fail-fast` 全绿。

## DONE（2026-11-04）

- 落地方式：`Store::maintain` 在 `gc` 之后调用新增的 `Store::cleanup_empty_partition`（`gc` 仍为 `&self`，避免牵动 fkv_shard / fkv_runtime 的调用签名）。
- 空表判定（缺一不可）：已打开、`manifest_started`、`root == NULL_PAGE`、data / segment mapper 无活跃映射、目录 data / segment 文件列表为空。
- 满足时只删当前 Store 的活跃 manifest（main 走 `drop_manifest`，branch 走 `remove_manifest_file`），重置 `manifest_started=false` / `manifest_size=0`，再调用既有 `try_remove_empty_partition_dir`；其它 branch / archive manifest 与任何残留文件都会让目录保留。
- 测试：`crate/engine/fkv_store/tests/truncate_drop.rs` 新增 truncate → maintain 目录删除、同一 Store 继续读写与 reopen、非空 / branch 场景不删目录。
- 验证：`cargo check -p fkv_store`、`cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings`、`cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast`（144 passed, 1 skipped）。

## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:3197`（`cleanup_empty_partition`，由 `maintain` 在 gc 之后
调用）+ `:3196` 注释（archive 的 manifest 不在此删除）；空表判定五条件与 DONE 一致
（已打开、manifest_started、root==NULL_PAGE、两 mapper 无活跃映射、目录无 data/segment
文件）。测试 `fkv_store/tests/truncate_drop.rs`（6 个测试）存在。
