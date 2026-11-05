# sto-65 P1：旧时间戳 delete 释放被保留旧值的 overflow/segment，存活条目悬垂

来源：第 5 轮 store/write lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

Rust 释放守卫只对 upsert 检查时序，delete 一律放行——
`crate/engine/fkv_task_write/src/lib.rs:670-689`：

```rust
// 与新值同序（merge：batch ts >= old ts 才覆盖）或删除时才释放旧值。
if !batch.op.is_delete() && batch.timestamp < iter.timestamp() {
  continue;
}
if iter.is_overflow() {
  free_overflow_chain(io, tbl, mapper, iter.value(), page_size).await?;
} else if iter.is_large_value() && ... {
  fkv_large::free_value(segment_mapper, iter.value(), ctx.segment_size)?;
}
// 旧 TTL 索引项同理越过 ts 检查被删（iter.expire_ts() != 0 分支）
```

而 merge 侧对 delete 同样按时序丢弃旧动作——`crate/core/fkv_btree/src/lib.rs:885-893`：

```rust
// timestamp 较旧的 batch 写入被丢弃；相等时 batch 覆盖（对齐旧实现）。
if kept.timestamp < iter.timestamp() {
  sink.push(iter.key(), MergeVal::page(iter))?;   // 旧 entry 存活（不分 op）
} else if !kept.op.is_delete() { ... }
```

C++ 的释放与 TTL 维护**整体**在 `change_ts >= base_ts` 分支内——
`eloqstore/src/tasks/batch_write_task.cpp:564-607`：

```cpp
if (change_ts >= base_ts)
{
    if (base_page_iter.IsOverflow()) { err = DelOverflowValue(base_val); ... }
    ...
}
else
{
    // no change, this update/delete is ignored.
```

## 问题

`Store::delete_at(tbl, key, ts)`（公开 API，`fkv_sdk` 直透）传**小于**旧 entry.ts
的时间戳时：merge 保留旧 entry，但它的 overflow 页链已被 `free_overflow_chain`
释放 / large value 段已被 `free_value` 归还 / TTL 索引项已被删。存活 key 后续读取
跟随已复用指针 → 读到垃圾页；TTL 项丢失 → 过期后 `clean_expired` 永远扫不到。
页池复用后为静默数据损坏。

## 目标

`free_replaced_values` 的守卫改为与 merge 完全一致的单条件：
`batch.timestamp < iter.timestamp()` 即 continue（不分 op），对齐 C++。
TTL 删除项同守卫内生成。

## 验收

- 测试：`put(kv, ts=200, overflow/大值)` → `delete_at(key, ts=100)` →
  `get` 仍返回旧值且值完整（段/页链未被释放）；`delete_at(key, ts=200)` 正常释放。
- `cargo nextest run -p fkv_task_write -p fkv_store`；clippy 零警告。

## 依赖 / 重叠

- 与 sto-66（TTL 同 expire 覆盖删除）同函数不同分支，可同一 PR 修，分两验收。
- 涉及热点文件 `fkv_task_write/src/lib.rs`（并行会话在改），开发前先对齐 owner。

## 核验记录

- 本人实读：`fkv_task_write/src/lib.rs:640-700`、`fkv_btree/src/lib.rs:860-895`、
  `batch_write_task.cpp:556-612`。
- 子代理摘录（未复核）：C++ 侧 UpdateTTL 行号细节。

## 审查结论（2026-11-04）

**成立（P1，数据悬垂 + TTL 索引丢失）**，已修：

- `fkv_task_write` 的旧值释放守卫是
  `if !batch.op.is_delete() && batch.timestamp < iter.timestamp() { continue }`：
  delete 一律越过时序检查，于是「更旧的 delete」也会释放旧条目的 overflow 链 /
  large value 段，并往 `ttl_delete_sink` 推旧 TTL 索引删除项。
- 而 `fkv_btree` 的 merge（`kept.timestamp < iter.timestamp()` 分支）**不看 op**：
  更旧的 delete 被丢弃、旧条目存活。两者叠加 → 存活条目指向已释放的页
  （悬垂读），旧 TTL 索引被清（过期永不触发、TTL 树残留）。
- C++ 把释放与 TTL 维护整体放在 `change_ts >= base_ts` 分支内
  （`batch_write_task.cpp:564-607`），语义与 merge 一致。

修复：守卫改为 `if batch.timestamp < iter.timestamp() { continue }`（注释写明与 merge
同一判定），即「只有 batch 真的覆盖了旧条目才释放/清 TTL」。释放与 TTL 维护仍共用同一
条件，跟 C++ 的结构一致。

## DONE（2026-11-04）

- 改动：`crate/engine/fkv_task_write/src/lib.rs`（守卫条件 + 注释）；
- 新增回归测试 `fkv_task_write::write::older_delete_keeps_old_value_and_ttl_index`：
  1. 写 `a=A`（ts=100、expire_ts=1000）；
  2. 用 **ts=50 的 delete**：断言 `ttl_delete_sink` 为空且 `read_value` 仍是 `A`
     （旧条目存活，不许释放/清 TTL）；
  3. 再用 **ts=200 的 delete**：断言 sink 恰好 1 条、`read_value` 为 `None`。
- **反向验证**：把守卫临时改回旧写法，该测试 **FAIL**；改回修复版则 PASS
  （证明它真的覆盖了这个 bug，不是空断言）。
- `cargo nextest run -p fkv_task_write -p fkv_store -p fkv_task_scan -p fkv_task_read`
  → 140 passed；
- `cargo clippy -p fkv_task_write --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

结论：**通过**。

- 守卫已改为单条件：`fkv_task_write/src/lib.rs:695` `if batch.timestamp < iter.timestamp() { continue; }`（不分 op，:691-694 注释写明与 merge 同一判定），释放 overflow/large value（:698-704）与 TTL 删除（:714-720）都在守卫之内。
- 回归测试 `fkv_task_write/tests/write.rs:1272-1372` `older_delete_keeps_old_value_and_ttl_index`：ts=50 delete 后断言 sink 为空（:1330-1333）+ 旧值仍可读（:1334-1340）；ts=200 delete 后 sink=1（:1364）+ 值 None（:1365-1370）。断言真实覆盖该修复。
