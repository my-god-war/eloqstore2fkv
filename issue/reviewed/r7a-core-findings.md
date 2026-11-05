# R7a 只读审查 findings（core / store / task）

范围：`crate/core/fkv_btree`、`crate/core/fkv_mapping`、`crate/engine/fkv_store`、
`fkv_task_read` / `fkv_task_scan` / `fkv_task_write` / `fkv_task_compact` / `fkv_task_gc`。

结论：VERDICT: FAIL（1 条高危正确性 + 1 条同源架构问题 + 3 条低危）。

验证（只读，未改代码）：

- `cargo check -p fkv_store -p fkv_task_write -p fkv_btree` 通过
- `cargo nextest run -p fkv_store -p fkv_task_write -p fkv_btree` 222 passed
- `cargo clippy -p fkv_store -p fkv_task_write -p fkv_btree --all-targets --all-features --no-deps -- -D warnings` 无告警

---

## 1. `IndexCache::clear` 复位 `base_gen = 0`，旧快照节点会被提拔进新 baseline

- 优先级：**P0（正确性）**
- 位置：`crate/core/fkv_btree/src/lib.rs:476-489`（复位在 487-488）；
  守卫在 `crate/core/fkv_btree/src/lib.rs:314-320`；
  触发点在 `crate/engine/fkv_store/src/lib.rs:1442-1447`（`settle_mapper_batch` 失败分支）

现象：

`insert` 只靠 `if generation < self.base_gen { return; }` 拒绝旧代条目。
`clear()` 把 `base_gen` 归 0 后该守卫对任意代际恒为假，等价于「接受所有代际的 insert」。

时序（单 shard，快照可跨写操作存活，见 `MappingSnapshot` / `SnapshotRegistry`）：

1. 读请求 A 持快照，`mapping_generation = 5`。
2. 写批次失败（OOM / manifest 写失败）→ `settle_mapper_batch(ok=false)`：
   mapper 代际回退到 6 之前的旧值，同时 `cache.clear()` 把 `base_gen` 置 0。
3. A 继续读某个已被上一次成功写改过的 index 页：`get(page_id, 5)` miss，
   于是按自己的旧快照读盘、`insert(page_id, node, 5)`；因 `5 < 0` 为假被接受，
   `gen_override = Some(5)` 并进了 `exception_li`。
4. 下一次成功写 `invalidate_index_cache` → `cache.retag(g_new)`：
   只逐条清 `exception_li` 的 override，不校验代际。
   第 3 步的旧节点被无条件下沉为 baseline 节点。
5. 新读者以 `gen == base_gen` 直接命中该旧节点：它的子页 id 可能已在本批被
   `free_page` 释放并复用 → 路由到错误页、漏读或读到别 key 的数据。

修法（择一）：

- `clear()` 不再改写 `base_gen`。abort 后 `base_gen` 本就等于回退后的代际
  （当前所有 `retag` 调用点都在提交成功之后，回滚路径不会推进 baseline），
  保留它才能继续拒绝 `generation < base_gen` 的旧快照回填。
- 若确实需要「清空后等价于新实例」，改成 `reset(generation)`：清条目 +
  把 `base_gen` 设成**当前 mapper 代际**，由 store 传入；不要归 0。

注意：`crate/engine/fkv_store/src/lib.rs:882`（`clear_index_cache`，`truncate` 用）
与 `:1446` 走同一问题路径，修复要一起覆盖；`truncate` 后逻辑页进空闲链，
被提拔的旧节点危害更大。

## 2. mapper 代际被当作缓存键，但它会重置 / 回退

- 优先级：**P1（架构，与 #1 同源）**
- 位置：`crate/core/fkv_mapping/src/lib.rs:632-643`（`empty` 设 `generation: 0`）、
  `:741-757`（`abort_batch` 回退代际）；
  `crate/engine/fkv_store/src/lib.rs:2107-2110`（reopen 后 `cache.retag(mapper.generation())`）

现象：

`PageMapper::with_tbl` / `read_view` 重建时 `generation` 恒为 0；`abort_batch`
把它回退。`install_replay_state` 复用缓存时 `retag(0)`，同样把 baseline 拉回 0。
于是「同代际 ⇒ 同映射」这个不变量在每个新 mapper 化身开始时不再成立，
`insert` 的拒绝条件与 `retag` 的提拔语义都建立在它之上。

当前 `retain_mapped` 会用 `file_page_id` 逐条过滤，能挡掉大部分 reopen 复用；
但新化身 + 旧快照（`gen_override` 较大）回填后仍能被后续 `retag` 提拔，
与 #1 是同一类污染。

修法：

给 `IndexCache` 引入 store 级单调 epoch（只增不减，进程内唯一），
`get` / `insert` / `retag` 全部用 epoch 作键；`mapping_generation` 只用于
「同一 mapper 化身内是否变过」。`clear()`/新实例的 `base_gen` 取当前 epoch，
永不归 0。需补一条「在途快照 + abort + 后续写」的缓存用例。

## 3. 根节点单子页上提在 leaf-index / data 根之间往返

- 优先级：**P2（写放大 / 空间放大）**
- 位置：`crate/engine/fkv_task_write/src/lib.rs:636-643`

现象：

上提条件只有 `frame.parent.is_none() && child_li.len() == 1`，不区分唯一子页是
index 还是 data page。

- 根是 leaf index、唯一子是 data page 时：删除最小 key 导致该页首键变化 →
  `child_li != frame.child_li` → 释放 leaf index 根，`root` 变回 data page。
- 紧接着的下一次写走 `route.data_page != NULL_PAGE` 分支，`build_new_index`
  又包一层 leaf index 根（新分配 1 页 + 1 个逻辑页 id）。

只要批内改到唯一 data 页的首键，就往返一次；append 分配下每批白烧一页，
直到 GC / compact 回收。功能上不错，但纯属多余。

修法：条件加 `&& !frame.leaf`——data 根与「只有一个 data 子的 leaf index 根」
语义等价，不必互换；只有单一子页是 index 页时才真正降低树高。

## 4. `InflightGuard::add_range_li` 每份 manifest 都重排累积链

- 优先级：**P3（GC 性能）**
- 位置：`crate/engine/fkv_task_gc/src/lib.rs:207-231`（`sort_chain` 在 229-230）

现象：

`add_range_li` 每被调用一次就对**整条已累积**的 `data_li` / `segment_li` 排序，
而它由 `apply_manifest` 在 manifest 循环里逐份调用（`manifest_retained_file_page_set_budget`
`crate/engine/fkv_task_gc/src/lib.rs:702-736`）。归档 / 分支 manifest 数 M、
每份区间链长 k 时总代价 O(M²·k·log(M·k))，GC 单轮随归档数平方增长。

修法：

- `add_range_li` 只 push（每份 `file_range_li` 自身已按上界有序）；
- 排序 + 去重挪到 `manifest_retained_file_page_set_budget` 循环结束之后做一次，
  或改用 k 路归并（每次合并一份短链，O(M·k·log k)）；
- `sort_chain` 保留给真正需要二分前的最后一次压实。

## 5. `peek_ttl_expire` 跳过坏 key，但出界的坏 key 永不被回收

- 优先级：**P3（健壮性 / 极小泄漏）**
- 位置：`crate/engine/fkv_store/src/lib.rs:2314-2360`（跳过循环在 2351-2358）；
  清理范围在 `:2384`（`expired_end = ttl_key(now+1, &[])`）

现象：

坏 TTL key 用 `key + 0` 前进跳过，避免 `next_expire_ts` 归零停摆（这部分是对的）。
但 `clean_expired` 只删 `[空, ttl_key(now+1))` 内的条目；排序在该上界之后的
坏 key（长度 < 8 且首字节大于 `now+1` 的高字节，例如 `0xff..`）既不进清理范围，
也不会被删除。结果是：

- 它永久留在 TTL 树里；
- 每次 `peek_ttl_expire` 都要为它多读一页（`page_entries = 1` 的扫描 + 跳过）。

仅在盘面损坏 / 旧格式残留时出现，量级很小，但属于「坏数据自愈」缺口。

修法（择一）：

- 跳过循环加计数上界（例如 `min(16, 剩余扫描预算)`），到上界即放弃本轮探测
  并返回 `None`，避免长尾；
- 让 `clean_expired` 在扫描结束后，对 `[expired_end, 首个有效 key)` 之间的坏 key
  补一轮删除（同样带上界），把坏条目真正清掉。

---

## 已核对、未发现缺陷的条目

- `expired_delete_ts`（`crate/engine/fkv_store/src/lib.rs:69-71`）：删除项不落页，
  时间戳只需压过同批被删条目本身；`max(now, entry_ts)` 在同批折叠里稳定胜出，
  不依赖注入的 `now_ms` 也不影响语义。
- `build_overflow_chain`（`crate/engine/fkv_task_write/src/lib.rs:1057-1130`）：
  逐页入 `PendingWrite`、超预算即 flush，峰值 = 值本体 + 预算 + 1 页 + 末页指针批，
  分段上界成立；`group_size < group_max` 与 `remaining > page_cap` 两个分支都验证过
  不会空转、不会漏字节。
- `put_large_impl` 段 journal 外置 TTL 清理（`:3005-3103`）：段批次只由外层结算，
  内层 `put_at_with_ttl(..., inline_ttl=false)` 只结算 data journal，
  `dispatch_ttl_clean` 推到外层结算之后，不产生嵌套回滚。
- `IndexCache` 记账 / Clock 逐出 / `retain_mapped` / 例外清单 O(例外数) `retag`：
  字节账本与 `Drop` 归还一致，硬上限由第二轮强制逐出保证。
- `scan_page_with_budget` 预取收割（`fkv_task_scan/src/lib.rs:320-330`）：
  `next_page == NULL_PAGE` 时预取是空操作、无在飞 IO；提前结束时吞错误、
  继续翻页时上抛，语义正确。
- `MappingTbl::resize/set/push` 的 `live_cnt` 增量与 `abort_batch` 回滚顺序一致，
  无下溢路径。
- 八个 crate 内无 `#[allow]`、无死代码 / 孤儿 pub API。


## 处置（2026-11-05）

本报告 findings 已在 r8/r9 与当前代码中修复；报告移入 `issue/reviewed/` 作为审查记录。
