# eng-86 P2：点读下降缓存命中路径双重 `get`（hit 计数翻倍）+「读盘→seek→插缓存」四份拷贝

来源：第 5 轮 core + read/gc 双 lane 独立发现（互相印证），协调者抽查。

## 证据

`crate/engine/fkv_task_read/src/lib.rs:137-141` 先探测缓存：

```rust
let cached = ctx
  .index_cache
  .is_some_and(|cache| cache.borrow().get(page_id).is_some());  // 第 1 次 get：置引用位 + inc_hit
if !cached { ... } else {
  let child = descend(io, tbl, mapper, page_id, key, &mut page, ctx).await?;
```

`descend`（`:92-95`）开头又查一次：

```rust
if let Some(node) = cache.borrow().get(page_id) { return node.seek(key); }  // 第 2 次 get + inc_hit
```

缓存命中的每层下降 = 2 次 HashMap 查找 + 2 次 hit 计数；miss 只计 1 次 →
hit 绝对值虚高，与 C++ 指标不可比（关联 sto-60 的对账口径）。
scan 路径（`fkv_task_scan/src/lib.rs:130-137`）已是正确姿势：单次 `get` 拿 node
直接 `seek`。

另：「`IndexNode::new` → `seek` → `insert` → child==page_id 环检测」块在
`read_entry:157-167`、`read_floor_entry:225-235`、`fkv_task_scan:146-156`、
descend 内共四份展开——今天的未提交 diff 恰好因签名变更（`IndexNode::new(file_page_id, page)`）
同步手改四处，正是复制粘贴模式的实锤。

## 目标

1. read 路径照 scan 改单次 `get`：命中直接 `node.seek`，miss 才读盘。
2. 四份「读盘→构造→seek→插缓存→环检测」抽成一个泛型 helper
   （如 `fn descend_step(...) -> Result<PageId>`），一处定义。

## 验收

- 单测：缓存命中路径 `cache_hit` 每层 +1（不再翻倍）；
  read/floor/scan 三入口走同一 helper（grep 单一定义点）。
- `cargo nextest run -p fkv_task_read -p fkv_task_scan`。

## 依赖 / 重叠

- eng-70b（done，双 IndexNode 构造）的残留收尾；27b（todo）改读路径签名时
  **顺路**做目标 2，避免二次扰动。

## 核验记录

- 子代理摘录（两 lane 交叉印证，未逐行复核）：行号如上。

## DONE（2026-11-04）

结论：成立，已实现。

改动：

- `fkv_task_read` 新增 `descend_step`（返回 `Descend::{Data, Child}`）与 `descend_child` 环检测，一处定义「缓存探测 → 读盘 → 页类型判定 → `IndexNode::new` → `seek` → 回填 → 环检测」。
- `read_entry`、`read_floor_entry`、`fkv_task_scan::scan_page_with` 三入口统一走该 helper，删除原 `descend` 与三处展开块；`IndexNode::new` 在 engine 内只剩 helper 一处。
- 单次 `get`：命中路径每层只计 1 次 hit，不再翻倍。
- 顺带删除 scan / floor 下降后「补读 data page」的冗余读与 `on_data` 标志。

测试：

- 新增 `cache_hit_counted_once_per_level`：两级 index 首读 miss=2 / hit=0，复读 hit=2 / miss=2。
- `cargo nextest run -p fkv_task_read -p fkv_btree -p fkv_store` 132 passed；`fkv_task_scan` 3 passed。
- `cargo clippy -p fkv_task_read -p fkv_btree -p fkv_task_scan --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_task_read/src/lib.rs:201-262`：`Descend::{Data, Child}` 枚举 + `descend_step`
一处定义「单次 get 探测（:225-228，命中直接 `node.seek`）→ miss 读盘 → 页型判定 →
`IndexNode::new` → seek → 回填 → `descend_child` 环检测」；`read_entry`(:281)、
`read_floor_entry`(:334)、`fkv_task_scan/src/lib.rs:153`（`scan_page_with` 下降）三入口统一
走该 helper；`fkv_task_read` 内 `IndexNode::new` 仅 :246 一处。测试
`fkv_task_read/tests/read.rs:145-186`：断言首读 `(hit,miss)==(0,2)`、复读 `(2,2)`，
hit 不再翻倍。
