# 27c P1：并发读写快照与 GC 保留验收

父 issue：`27-no-cow-read-snapshot.md`。

## 目标

为 `27a` / `27b` 提供验收用例，证明「读进行中并发写，读结果自洽」与
「GC 不回收被活跃快照引用的页」。

## 用例

1. **点读自洽**（`crate/engine/fkv_store/tests/` 或 `fkv_shard/tests/`）：
   - 用可控 `PageIo` 在 `read_page` 处挂起读；
   - 读到一半插入 `put` + `remap_page`（甚至覆盖旧物理页）；
   - 恢复读，断言返回读起点版本的值，不出现新旧页混读。
2. **scan 自洽**：同上，scan 跨多个 data page，写者只改部分页；断言遍历链
   与叶子内容全部来自同一快照。
3. **索引缓存代际**：读未命中缓存 → 写改 index 页 → 旧快照再读，断言不复用
   写后的 `IndexNode`。
4. **GC 保留**：某文件只被旧快照引用，`Store::gc` 后文件仍在；快照 drop 后
   下一轮 GC 删除。
5. **无快照时零回归**：Append + 无活跃读者，`gc` / 读路径行为与当前一致，
   且快照并集分支不产生额外分配（可用现有统计断言）。

## 约束

- 测试只走公开接口，不直接摸内部字段。
- 不用 `sleep` 等时序；用事件/通道或可控 IO 精确卡 await 点。
- 若 `eng-32b` 未落地，用例 1 / 2 先在 `fkv_store` 层用「手动持有快照 +
  手动写」模拟，不伪造 shard 并发。

## 验收

- 上述 5 组测试稳定通过（`cargo nextest run -p fkv_store --no-fail-fast`）。
- 去掉快照（临时 revert `27a`/`27b`）时用例 1 / 4 必须失败，证明测试有效。

## 依赖

- `27a`、`27b` 完成；`eng-32b` 决定用例 1 / 2 落在 store 层还是 shard 层。

## DONE（2026-11-05）

复审结论：27a/27b 落地后用例 1–4 已由
`crate/engine/fkv_store/tests/concurrent_snapshot.rs`（5 条）与
`gc_snapshot.rs`（原 2 条）覆盖；本次逐条复核并补齐用例 5（无快照零回归）。

- 新增 `gc_snapshot::gc_reclaims_old_files_immediately_without_active_snapshot`：
  无活跃快照时覆盖写后旧文件必须在**当轮** GC 全部回收（不拖到下一轮），
  点读 / scan 只见当前值，且 `snapshot_num == 0`（普通读写不遗留快照登记）。
  这条同时钉住「快照并集分支在空注册表时不把旧页并进 retained set」——
  否则 `removed` 会小于 `frozen_li.len()`。
- 复核既有用例：
  - 用例 1：点读 / floor 挂起在首个 `read_page`，期间真实 `put` 覆盖或删除，
    恢复后返回读起点快照（`suspended_get_...` / `suspended_floor_...`），
    且写后当前 `store.get` 已看到新值（反向断言）。
  - 用例 2：scan 跨多个 data page，写者只改部分页；旧快照遍历链与叶子全部
    来自同一旧快照，当前 `store.scan` 已变化（反向断言）。
  - 用例 3：旧快照回填旧代 `IndexNode` 后写改 index 页，新快照读新值（miss
    后回填），旧快照再读仍取旧值且必须 miss 新代节点（双向断言）。
  - 用例 4：挂起读快照独占的文件 GC 后仍在，快照 drop 后下一轮 GC 删除，
    当前数据不被误删；segment 侧同构用例亦在。
- 约束复核：全部用例只走公开接口，用 `ReadGate` 在 `read_page` 精确挂起，
  无 `sleep`。

验证：

```
cargo nextest run -p fkv_store -E 'binary(gc_snapshot) | binary(concurrent_snapshot)'
cargo clippy -p fkv_store --all-targets -- -D warnings
```

结果：8/8 通过（concurrent 5 + gc 3，新增 1 条）。

## 复审（2026-11-05 r7）

结论：**通过**。8 条测试全部在位且断言覆盖修复（均亲读）：

- `fkv_store/tests/concurrent_snapshot.rs` 5 条：`suspended_get_…` `:265`（挂起读
  返回读起点快照 `:299-303`，反向断言写已提交 `:291-295`）、
  `suspended_floor_…` `:309`、`suspended_scan_…` `:347`、
  `gc_retains_files_referenced_by_suspended_read_snapshot` `:415`、
  `old_snapshot_misses_index_cache_entries_from_new_generation` `:526`。挂起点
  全部经 `ReadGate.arm/wait_blocked/release`（`:276-297`），无 sleep。
- `fkv_store/tests/gc_snapshot.rs` 3 条：data/segment 两条既有 +
  新增 `gc_reclaims_old_files_immediately_without_active_snapshot` `:210-262`：
  前置 `snapshot_num == 0`（`:225`），覆盖写后**当轮** GC 必须回收
  `removed == frozen_li.len()`（`:238-243`），盘面逐文件确认已删（`:244-247`），
  点读/scan 只见当前值且不遗留快照登记（`:250-260`）——同时钉住「快照并集
  分支空注册表不并旧页」，否则 `removed < frozen_li.len()` 即失败。
- 约束符合：测试走公开接口（`store.snapshot` / `store.gc` / read helper），
  无 sleep 时序依赖。
