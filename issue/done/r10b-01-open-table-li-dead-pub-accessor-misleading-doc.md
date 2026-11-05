# r10b-01 P3：`Store::open_table_li` 零生产消费，且文档声称的 detach 消费点不存在（零死代码纪律 + 文档失真）

## 现象（亲读）

`crate/engine/fkv_store/src/lib.rs:647-655`：

```rust
/// 已打开表的快照，供 detach 驱逐使用。
pub fn open_table_li(&self) -> Vec<TableIdent> {
  self
    .table_map
    .iter()
    .filter(|(_, state)| state.root.is_some())
    .map(|(tbl, _)| tbl.clone())
    .collect()
}
```

全仓引用计数（`grep -rIn "open_table_li" . --include="*.rs"`，排除 `target/` 与 `issue/`
目录，逐条核对）：

- 定义体：`fkv_store/src/lib.rs:648`。
- 生产消费：**0**。`fkv_shard`（`lib.rs` / `partition.rs` / `roots.rs`）、`fkv_runtime`、
  `fkv_api`、`fkv_cli`、`fkv_bench` 全无命中。
- 测试消费：`fkv_store/tests/table_state_cleanup.rs:42/:60/:76`、
  `fkv_store/tests/root_meta_budget.rs:131`（皆本 crate 自测，断言「打开列表」清空 / 计数）。

文档声称的消费点不存在：detach / attach 身份切换走
`fkv_shard/src/partition.rs:257` 的 `PartitionMap::clear()` —— 它枚举自身 `map` 的 key
逐个 `evict_table`（`:244`），**不经** `Store::open_table_li`；`admit_detach`
（`fkv_shard/src/lib.rs:1538`）与所有 `evict_table` 调用点（`partition.rs:230/:244/:260/:396/:467`、
`lib.rs:1889/:2045/:3090`）都按显式 `tbl` 操作，无一处调用本符号。即 detach 驱逐是
shard 自持表清单完成的，`open_table_li` 与之无关。

与 r9b-01（`fkv_task_compact` 单文件包装：零生产消费 + 无接缝声明 → 删除）、
r8a-01（死 pub 常量）同族。本符号既无「测试 / 诊断用」接缝声明（对照同 crate
`index_cache_len` 文档 `:918` 明写「（诊断 / 测试用）」），doc 反而指向一个**不存在**的
生产消费者（detach），属 doc 失真 + 孤儿 pub API。

## 目标

三选一（对齐 r9b-01 的处置口径）：

1. 删除 `open_table_li`，两处测试改用 `is_open` + 测试自持的表清单做断言；
2. 或保留符号但修正 doc：去掉失真的「供 detach 驱逐使用」，改写为「测试 / 诊断用接缝」
   （与 `index_cache_len` 口径一致）；
3. 或接真实生产消费（当前 shard 自持表清单，detach 无此需求，故该路径不成立）。

## 验收

- `grep -rIn "open_table_li" crate/ --include="*.rs"` 的命中要么消失（删除路径），
  要么定义处带「测试 / 诊断用」接缝注释；
- `cargo nextest run -p fkv_store` 不回归；`./sh/clippy.sh` 零警告。

## 核验记录

- 亲读 `fkv_store/src/lib.rs:640-655`（`is_open` / `open_table_li`）与 `:918`
  （`index_cache_len` 的接缝声明）；
- 全仓引用核对：`grep -rIn "open_table_li" . --include="*.rs"`（排除 `target/`）仅得
  定义处 + 4 处本 crate 测试；生产 crate 零命中；
- 亲读 shard detach 链：`partition.rs:244-263`（`evict_table` / `clear`）、
  `shard/lib.rs:1538`（`admit_detach`）——确认无 `open_table_li` 调用；
- 查重：`issue/` 全目录（含 reviewed / done / reject / new）无 `open_table_li` 条目；
  r9b-01/02/03、r9a-02、r8a-01 均不涉本条（r9b-01 检的是 `fkv_task_compact` 单文件包装，
  与本条的 accessor 孤儿不同符号）；
- mtime 备注：`fkv_store/src/lib.rs` 为并行写入热点，本轮按内容锚点定位（`open_table_li`
  定义唯一），行号漂移不影响判定。

## DONE（2026-11-05）

按处置 2 修正文档：明确为测试 / 诊断接缝，删除不存在的 detach 消费点描述。
