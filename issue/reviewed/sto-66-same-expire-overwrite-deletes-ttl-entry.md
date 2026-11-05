# sto-66 P1：相同 expire_ts 覆盖把 TTL 索引项整体删掉，过期条目永不物理清理

来源：第 5 轮 store/write lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

1. TTL upsert 的时间戳恒为 0——`crate/engine/fkv_task_write/src/ttl.rs:61-63`：

```rust
} else if entry.op == WriteOp::Upsert && entry.expire_ts != 0 {
  let key = ttl_key(entry.expire_ts, &entry.key);
  ttl_map.insert(key.clone(), BatchEntry::upsert(key, Vec::new(), 0));
}
```

2. merge 收集的旧 TTL 删除项用 `batch.timestamp`——
   `crate/engine/fkv_task_write/src/lib.rs:682-688`：
   `sink.push(BatchEntry::delete(key, batch.timestamp));`
3. 组装顺序 upsert 在前、delete 在后——`crate/engine/fkv_store/src/lib.rs:1344`：
   `ttl_li.extend(ttl_delete_li);`
4. `write_batch` 同 key 折叠「`cur.timestamp >= kept` 后者胜」
   （`fkv_task_write/src/lib.rs:215-227`）→ delete(ts>0) 恒胜 upsert(ts=0)。

同一 key 以**相同绝对 expire_ts** 再次 upsert 时（如反复刷新「固定 12:00 到期」），
旧页 merge 出 `delete ttl_key(E,k)`，与新批的 `upsert ttl_key(E,k)` 同 key →
折叠保留 delete → TTL 树条目消失。

C++ 有显式守卫（`batch_write_task.cpp:598-607`）：

```cpp
// update expire timestamp only if it is changed.
if (base_expire != expire_ts)
{
    UpdateTTL(base_expire, base_key, WriteOp::Delete);
    UpdateTTL(expire_ts, new_key, WriteOp::Upsert);
}
```

## 问题

到期后 `clean_expired` 扫不到该 key → 主树 entry（含 overflow 链）永久滞留，
`next_expire` 失真；读路径 `ttl_read_filter` 掩盖症状，空间无界泄漏。

## 目标

`free_replaced_values` 的 TTL 删除项只在 `iter.expire_ts() != batch.expire_ts()`
时生成（对齐 C++ `base_expire != expire_ts` 守卫；delete 分支不需要，因为
delete 本身会删主树 entry，TTL 项必删）。注意 sto-65 修完后本守卫位于
ts 检查之内。

## 验收

- 测试：`put(k, expire=1000)` → 再 `put(k, expire=1000)`（同 expire）→
  TTL 树仍有条目；到期 `clean_expired` 后主树 entry 被物理删除。
- `cargo nextest run -p fkv_task_write -p fkv_store`。

## 依赖 / 重叠

- 依赖 sto-65 先落（同一函数、同一段守卫重构）；TTL 折叠时序属 eng-50 已定语义。

## 核验记录

- 本人实读：`ttl.rs` 全文、`fkv_task_write/src/lib.rs:205-235/640-700`、
  `fkv_store/src/lib.rs:1265-1350`、`batch_write_task.cpp:598-607`。

## 审查结论（2026-11-04）

**成立（P1，TTL 索引丢失 → 空间无界泄漏）**，已修：

- TTL 索引 upsert 的时间戳恒为 0（`fkv_task_write/src/ttl.rs`），而旧 TTL 删除项用
  `batch.timestamp`（>0）→ 同 key 折叠（`cur.timestamp >= kept` 后者胜）里 delete 必胜；
- `fkv_store` 组装顺序是 upsert 在前、`ttl_delete_li` 在后
  （`crate/engine/fkv_store/src/lib.rs`），于是「相同 expire_ts 的覆盖」会把刚写进去的
  TTL 索引项删掉 → 到期后 `clean_expired` 扫不到 → 主树条目（含 overflow 链）永久滞留。
- C++ 有显式守卫：`if (base_expire != expire_ts)` 才 `Delete` 旧项 / `Upsert` 新项
  （`batch_write_task.cpp:598-607`）。

修复：旧 TTL 删除项只在 **expire_ts 真的变了** 时才推——
`new_expire = if batch.op.is_delete() { 0 } else { batch.expire_ts }`，
条件加 `iter.expire_ts() != new_expire`（删除时 new_expire=0，必然不等，行为不变）。

## DONE（2026-11-04）

- 改动：`crate/engine/fkv_task_write/src/lib.rs`（旧 TTL 删除项的触发条件 + 注释）；
- 新增 `fkv_task_write::write::same_expire_overwrite_keeps_ttl_index_entry`：
  1. `a=A`（ts=100, expire=1000）；
  2. 相同 `expire_ts=1000` 覆盖（ts=200）：`ttl_delete_sink` 必须为空、值变 `B`；
  3. `expire_ts=2000` 覆盖（ts=300）：sink 恰好 1 条。
- **反向验证**：去掉 `!= new_expire` 条件后该测试 **FAIL**，加回则 PASS。
- `cargo nextest run -p fkv_task_write` 32 passed；
  `-p fkv_task_write -p fkv_store -p fkv_task_scan -p fkv_task_read` 140 passed；
- `cargo clippy -p fkv_task_write --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

结论：**通过**。

- 守卫已落地：`fkv_task_write/src/lib.rs:709-713` `new_expire = if batch.op.is_delete() { 0 } else { batch.expire_ts }`；`:714-719` 仅 `iter.expire_ts() != 0 && iter.expire_ts() != new_expire` 才推 `delete ttl_key`（对齐 C++ `base_expire != expire_ts`）。
- 回归测试 `fkv_task_write/tests/write.rs:1377-1470` `same_expire_overwrite_keeps_ttl_index_entry`：同 expire=1000 覆盖后 sink 为空（:1432-1435）且值变 B（:1436-1441）；expire 变 2000 后 sink=1（:1468）。断言真实覆盖。
- 与 sto-65 同函数，两条守卫顺序（先 ts 后 expire）与 md 记录一致。
