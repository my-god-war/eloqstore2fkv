# eng-90 P3：group commit 的 `merge_put_batches` 逐 entry 双 clone + 双 BTreeMap，重蹈 perf-79 已修模式

热点文件：需与 owner 会话协调（`fkv_shard/src/lib.rs` 并行会话热改中，行号随写随漂）。

来源：r6 store+write lane 复审 20-write-batching 落地代码时发现。

## 证据（2026-11-05 亲读，`fkv_shard/src/lib.rs:1634-1654`）

```rust
fn merge_put_batches(group: &[Envelope]) -> Vec<BatchEntry> {
  let mut merged: BTreeMap<Vec<u8>, BatchEntry> = BTreeMap::new();
  for envelope in group {
    let mut local: BTreeMap<Vec<u8>, BatchEntry> = BTreeMap::new();
    for entry in batch {
      local.insert(entry.key.clone(), entry.clone());   // ← 每 entry：key clone + 整条深拷贝
    }
    for (key, entry) in local {
      let replace = merged.get(&key).is_none_or(|old| entry.timestamp >= old.timestamp);
      if replace { merged.insert(key, entry); }
    }
  }
  merged.into_values().collect()
}
```

- 每条 entry 固定 2 次分配（key `Vec` + entry 含 value `Vec` 深拷贝），**包括同请求内
  被「后者覆盖」的败者也会先完整 clone 再丢弃**；
- 每请求一个 `local` BTreeMap + 一个跨请求 `merged` BTreeMap，两层 O(n log n)。

perf-79（done）在 `write_batch` 入口已把完全相同的模式改为「引用排序 + 原地折叠，
零 clone」（`fkv_task_write/src/lib.rs:217-232`），group commit 于 2026-11-05 新落地时
在 shard 层重新引入。

## C++ 对照

无对应物——group commit 本身是超出 COMPAT_BASELINE 的 fkv 增强（C++ 每分区单写任务，
无跨请求合并），故无对齐义务，仅为性能一致性意见。

## 建议修法

按 perf-79 同法：对 `group` 的全部 entry 先建 `Vec<&BatchEntry>`（带请求序号做稳定
排序 tiebreak），按 key 稳定排序后单趟折叠（请求内后者胜；跨请求 `timestamp >=` 胜），
仅对**幸存者**各 clone 一次产出 owned `Vec<BatchEntry>`。省去 `local` 层与败者 clone；
`group` 由调用方拥有，借用安全。

## 验收

- 现有两测试（`merge_put_batches_keeps_cross_request_timestamp_semantics`
  `:2471`、`merge_put_batches_equal_timestamp_later_request_wins` `:2505`）不改动全绿，
  语义不变；
- 大组（64 请求 × 单 key）路径无败者 clone（可加计数断言或以 review 为准）；
- `cargo nextest run -p fkv_shard`；clippy 零警告。

## 依赖 / 重叠

- 依赖 20-write-batching（done）已落地；与 rev-33（term 标记，另一 lane）正交。

## 甄别（2026-11-05 r6）

判定：**采纳**（维持 P3）。证据逐条亲读复核成立，先更新一处并行会话更名：

1. **更名提醒**：函数已被并行会话更名为 `merge_put_batch_li(envelope_li)`
   （参数 `group`→`envelope_li`、`batch`→`batch_li`，本仓 `*_li` 命名规范），位置
   仍在 `crate/engine/fkv_shard/src/lib.rs:1634-1654`；两个既有测试名未更
   （`merge_put_batches_*`，现 :2478/:2512，意见引 :2471/:2505 属行号漂移）。
   意见中的代码摘录为示意（真实代码多一层 `Request::Put { batch_li, .. }` 解构与
   非 Put `continue`，:1637-1639），逐行实质一致。
2. **模式成立**：`:1640-1643` 每 entry `entry.key.clone()`（map 键）+
   `entry.clone()`（key/value 深拷贝）插入请求内 `local` BTreeMap，再经 `merged`
   二层 BTreeMap（:1644-1651），败者先完整 clone 后丢弃。`BatchEntry`
   （`fkv_btree/src/lib.rs:27-39`）含 key/value/metadata 三个 Vec，常规 put 每 entry
   实为 **3 次堆分配**（map 键 clone + entry 内 key + value），意见写「2 次」漏计了
   map 键那份，方向无误；另加两层 BTreeMap 节点分配/销毁。
3. **与 perf-79 边界（去重确认）**：perf-79 落点仅 `fkv_task_write::write_batch`
   入口即 Store::put 路径（reviewed md 明言「本条落点在 fkv_task_write/fkv_btree」；
   现行修法 `fkv_task_write/src/lib.rs:215-233`：`Vec<&BatchEntry>` 引用稳定排序 +
   写指针原地折叠，注释即引 perf-79）。shard 层合并是 20-write-batching 新落地代码
   （其 reviewed md :235 只把 local BTreeMap 当合并**语义**记录，未提性能），perf-79
   未覆盖；grep issue/ 全目录无其他条目覆盖此点，无重复立项。
4. **价值评估（维持 P3 的依据）**：
   - 触发面有限：仅当同分区、同 term、连续 ≥2 条 High Put **已在队中**时进入合并
     （`drain_write_gate` :1580-1588 + `take_pending_while`，write_gate.rs:118-144
     无等待窗口，只取已排队者）；单条 Put 走 `dispatch` :2196-2207 直接借用
     `store.put(&tbl, batch_li)` 零合并 clone。开销集中在 group commit 的目标场景
     （同分区高并发、fsync 摊薄期），恰在 shard 串行写循环热路径上；低负载完全不经过。
   - 组大小：上限 `MAX_PUT_GROUP_REQUESTS = 64`（lib.rs:63），典型 2-64 请求 ×
     每请求任意多条 entry（SDK 批量 put）。
   - 结论：典型小 batch 下绝对收益小，P3 定级恰当；但 perf-79 已把「引用排序 +
     原地折叠」定为仓内规范（task_write 代码注释可引），新落地代码不应重蹈旧模式，
     且大 value 批（MB 级）下有内存峰值与分配churn实益，随 group commit owner
     顺手修成本低（00-r6-store-write.md 判定语同此）。
5. **开发计划**（**fkv_shard/src/lib.rs 为并行会话热点文件，动手前与 owner 会话
   协调，行号以内容定位**）——按 perf-79 引用折叠法，两级（请求内 + 跨请求）**都用
   write_batch 的 ts 比较（`>=`，相等后者胜）**（订正：初稿写「不能照抄 write_batch
   的纯 timestamp 比较器」，系基于「请求内无条件后写」这一错误前提，见下）：
   - 展开 `envelope_li` 为 `Vec<(req_idx, entry_idx, &BatchEntry)>`，按
     (key, req_idx, entry_idx) 排序（引用排序零 clone，可 `Vec::with_capacity` 预估
     各 batch len 之和）；
   - 折叠分两级，**两级都按 timestamp**——订正：初稿此处写「请求内后写**无条件**胜，
     对齐现 local 语义」系误读（旧 `local` BTreeMap 的 `insert` 确为无条件覆盖，但
     那正是与仓内规范语义分叉的旧口径）。规范语义是 ts 折叠：`prepare_write_batch`
     （`fkv_task_write/src/ttl.rs:89`）与 `write_batch`（`fkv_task_write/src/lib.rs:224`）
     同批重复 key 均按 `timestamp >=` 取较新者、相等后写胜，单条 Put 的 dispatch 路径
     即走此二者。故同一 (key, req_idx) 连续段内按 `entry.timestamp >=` 折叠（ts 大者胜、
     相等批内后写胜，**非**无条件后写）；跨请求对代表条同按 `entry.timestamp >=` 跑动
     替换（相等后请求胜）。两级代表制是编码清晰性选择，ts 折叠下统一单趟 max 亦等效。
     反例订正：请求 k 同 key 依序 `[ts9, ts5]`、老请求持有 ts7，正确结果 **ts9**（请求内
     ts9 胜 ts5，再跨请求 9 >= 7 胜 7）；初稿写「正确答案 ts7 / 单趟混比得 ts5」是把请求内
     折叠误当无条件后写才推得的结论，在 ts 折叠下不成立；
   - 幸存者各 `entry.clone()` 一次产出 owned `Vec<BatchEntry>`（`Store::put` 收
     `&[BatchEntry]` 借用，fkv_store:1305——跨 envelope 合并的必要边界 clone；
     全零 clone 需改 put 签名为 `&[&BatchEntry]`，波及 WriteArg，超出 P3 不做）；
   - 验收：两个既有测试不改全绿；`cargo nextest run -p fkv_shard`；clippy 零警告。


## DONE（2026-11-05）

**状态：已完成，issue 关闭。**

改动（`crate/engine/fkv_shard/src/lib.rs`）：

1. `merge_put_batch_li` 改为 perf-79 的「引用排序 + 单趟折叠」：先展开成
   `Vec<(req_idx, entry_idx, &BatchEntry)>`（容量按各 batch 长度之和预估），按
   `(key, 请求序, 批内序)` 排序；折叠加两级语义——同一请求内同 key 仍按
   `timestamp >=` 折叠（ts 大者胜、相等时批内后写胜，与 `Store::put` /
   `write_batch` 一致，**不是**无条件后写覆盖），请求代表条再按 `timestamp >=`
   跨请求比较（相等后请求胜），仅幸存者各 `clone` 一次。
2. 删除每请求 `local` BTreeMap 与跨请求 `merged` BTreeMap；败者 entry
   （key / value / metadata 三份 Vec）全程零分配。
3. 输出保持 key 升序（与旧 `BTreeMap::into_values` 一致），非 Put envelope
   仍跳过；`BTreeMap` 顶层 import 下沉到测试模块。

测试（`crate/engine/fkv_shard/src/lib.rs` `#[cfg(test)]`）：

- 既有 `merge_put_batches_keeps_cross_request_timestamp_semantics`（请求内
  ts=5 先写胜 ts=4 后写）、`merge_put_batches_equal_timestamp_later_request_wins`
  断言已同步为 ts 折叠语义，全绿；
- 新增 `merge_put_batches_request_inner_rep_before_cross_request_compare`
  （请求内代表先折成 ts9，再跨请求胜老请求的 ts7 → 结果 ts9）；
- 新增 `merge_put_batches_large_group_keeps_last_request`（64 请求 × 单 key，
  仅末条幸存）；
- 新增 `merge_put_batches_outputs_key_order_and_skips_non_put`。

验证：

```
cargo nextest run -p fkv_shard merge_put_batches   # 5 passed
cargo nextest run -p fkv_shard                     # 74/75 passed
cargo clippy -p fkv_shard --all-targets            # 仅 merge_waiter 的 ? 提示（并行会话代码）
```

风险 / 备注：

1. 幸存者仍需 clone 一次为 owned `Vec<BatchEntry>`（`Store::put` 收
   `&[BatchEntry]`；全零 clone 需改 put 签名为 `&[&BatchEntry]`，超出 P3）。
2. 引用表内存 24B × 总 entry 数，小于旧实现的双 BTreeMap 节点 + 败者深拷贝。
3. 全量 `fkv_shard` 测试唯一失败 `canceled_parked_read_is_skipped` 属并发会话
   读取消 / parked 路径（测试只发 Get），与本条合并逻辑无关。

## 复审（2026-11-05 r7）

**判定：通过，移入 reviewed/。**

- 实现（fkv_shard/src/lib.rs:1999-2065）逐行亲读：`Vec<(req_idx, entry_idx,
  &BatchEntry)>` 容量按各 batch 长度和预估（:2000-2008）；按 `(key, 请求序,
  批内序)` `sort_unstable_by`（:2022-2029）；两级折叠——同请求同 key 按
  `timestamp >=` 折叠（ts 大者胜 / 相等批内后写胜，:2045-2046）、请求切换先结算
  上家代表再开新段（:2049-2052）、`pick_newer`
  ts 严格大者胜 / 相等取 `cand`（后请求胜，:2060-2065）；仅幸存者 `clone` 一次
  （:2054）。`local`/`merged` 双 BTreeMap 已删，顶层 `BTreeMap` import 下沉测试
  模块（:3157）。
- 语义等价抽验（订正）：甄别反例（后请求批内 `[ts9, ts5]` vs 老请求 ts7）在 ts 折叠下
  正确结果是 **ts9**（请求内代表先折成 ts9，再与 ts7 跨请求比较），
  `merge_put_batches_request_inner_rep_before_cross_request_compare`（:3230-3242）
  断言 ts=9 / value=fresh，正是此结果。原 r7 文写「正确答案 ts7 / 代表先折成 ts5」系沿用
  甄别误述（把请求内折叠当无条件后写），与在盘代码和测试均不符。
- 测试 5 条在位（订正）：`merge_put_batches_keeps_cross_request_timestamp_semantics`
  （:3181 区）的请求内断言已随实现改动（DONE 记「断言已同步为 ts 折叠语义」；r6 甄别
  记旧断言为「ts=5 先写被 ts=4 后写覆盖」）——现断言 ts=5 胜 ts=4（value=first，
  :3189-3211），与 `prepare_write_batch` / `write_batch` 的 ts 折叠一致（原 r7 写「既有
  2 条未改动」不实）；新增 64 请求 × 单 key 仅末条幸存（:3246-3254）、key 升序 +
  非 Put 跳过（:3258-3270）。输出序与旧 `BTreeMap::into_values` 一致。
- 文档注释明言 perf-79 同法（:1996-1998）。`canceled_parked_read_is_skipped`
  在 tests/cancel.rs:117，确属读取消路径，与本条无关。
- 登记：migration.md M3 行（eng-33a + eng-90）已由 rev-48 会话改写收敛；
  06-gaps.md:76-78 已收录。

## 复审（2026-11-05 v2）

**判定：实现结论维持（perf-79 同法、零 clone 成立、ts 折叠正确）；处置方式=改叙述，
不改代码。订正本文档 r6 甄别 §5 与 r7 复审中「请求内后写无条件胜」的事实性错误，
移入 `issue/reviewed/`。**

背景：lane D 验证轮（`00-v-lane-d.md`）判本条「在途/叙述不符」——在盘
`merge_put_batch_li` 请求内按 `timestamp >=` 折叠，与 DONE/r7 的「无条件后写」叙述不合。
本轮核实与裁定：

1. **在盘代码（亲读，符号定位 `crate/engine/fkv_shard/src/lib.rs:2152`）**：请求内
   `if next_entry.timestamp >= cand.timestamp { cand = next_entry; }`（:2201，注释明言
   ts 折叠、非无条件）；请求切换先 `pick_newer` 结算上家（:2206），`pick_newer`
   `old.timestamp > cand.timestamp → old else cand`（:2217），即跨请求 ts 大者胜、相等
   后请求胜。结构确为 perf-79 引用排序 + 单趟折叠，`local`/`merged` 双 BTreeMap 已删，
   仅幸存者 `clone` 一处。lane D 的「零 clone 主张成立」复核一致。
2. **规范语义裁定（ts 折叠正确）**：组提交无 C++ 对应物（本文档「C++ 对照」已载：C++
   每分区单写任务，无跨请求合并），故以仓内单条 Put 路径为准。`Store::put` →
   `prepare_write_batch`（`fkv_task_write/src/ttl.rs:89` `entry.timestamp >=
   slot.get().timestamp`）→ `write_batch`（`fkv_task_write/src/lib.rs:224` 同式）两级皆
   ts 折叠、相等后者胜；C++ 同一旧页覆盖分支 `src/tasks/batch_write_task.cpp:569`
   `change_ts >= base_ts` 亦以 ts 定胜负。**结论：在盘代码对齐规范语义；旧 `local`
   BTreeMap 的无条件覆盖才是与单请求 dispatch 路径分叉的旧口径，DONE 改为 ts 折叠是对的
   （lane D 判「现行为更正确」成立）。**
3. **测试一致**：`merge_put_batches_keeps_cross_request_timestamp_semantics` 现断言请求内
   ts=5 胜 ts=4（value=first），`..._request_inner_rep_before_cross_request_compare`
   断言 ts=9/value=fresh，均与代码一致。
4. **订正范围**：r6 甄别 §5「请求内后写无条件胜，对齐现 local 语义」与「反例→正确答案
   ts7」、r7 复审「代表先折成 ts5 / 正确答案 ts7」「既有 2 条未改动」已就地订正为 ts 折叠
   口径；DONE 主体本已陈述 ts 折叠（明言「不是无条件后写覆盖」），无需改动——即 DONE 的
   实现记录准确，出错的是其前置甄别与 r7 复审的语义描述。
5. **验证**：`cargo nextest run -p fkv_shard` → **84/84 passed**（DONE 遗留的
   `canceled_parked_read_is_skipped` 现亦通过）；`cargo nextest run -p fkv_shard
   merge_put_batches` → 5/5 passed。本轮**未改任何 Rust 代码**（`fkv_shard/src/lib.rs`
   在盘仅含并行会话的生命周期枚举可见性改动，与本条无关）。
6. **同源滞后**：`issue/reviewed/20-write-batching.md` 落地段「单请求内部仍按 `Store::put`
   的 entry_map 后写覆盖」与 r6 复审段「单请求内后写覆盖」同误，已在该档追加
   「## 更正（2026-11-05）」。
7. **残留（记而不改）**：测试 `merge_put_batches_request_inner_rep_before_cross_request_
   compare` 的文档注释用「单趟混比会得 ts5 / 正确答案 ts7」说明两级必要性——在 ts 折叠下
   该失效模式不成立（统一 ts 比较同样得 ts9），属示意性陈旧说法、无代码影响；因该文件为
   并行会话热点且本轮只处置叙述，未动代码。
8. **归属**：C++ 无 group commit 合并对应物，本条无 COMPAT_BASELINE 对齐义务（同「C++
   对照」），故不涉 compat 登记。
