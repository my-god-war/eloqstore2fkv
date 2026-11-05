# eng-75 大 value 写入 / 读取各复制一份实现：put_large 与 put_large_fixed 约 80 行同构，get_large / get_large_into / floor 重复元数据与 segment 读取

## 位置

`crate/engine/fkv_store/src/lib.rs`：

- `put_large` `:1821-1892` 与 `put_large_fixed` `:1896-1975`：约 80 行同构。
  两者只在“大 value 写到哪”一处不同：
  - plain：`write_large_value(&self.io, ..., &entry.value, &entry.metadata)`（`:1846`）
  - fixed：先把 value 拷进 `FixedBuffer`，再 `write_value_fixed_with_len(...)`（`:1914-1928`）
  其余逐项相同：`layout` 检查、`check_branch_segment_limit`、`segment_mapper.begin_batch`、
  遍历 batch、判断 `large_value_threshold`、`branch_segment_limit` 复查、`self.put`、commit/abort。
- `get_large_into_at` `:1988-2040` 与 `get_large_at` `:2092-2138`：查找 entry、TTL 过滤、
  非 large 值返回、`SegmentLayout` / `segment_mapper` 校验、`read_value*` 全部重复；
  只差“读进调用方 buffer”还是“分配 Vec 返回”。
  `floor` `:2044-2086` 又重复一遍同样的 entry 查找 + large value 分支 + segment 读取。
- 两个 put 路径都在循环内 `let mut entry = entry.clone()`（`:1837` / `:1914`）：
  对每个 entry 连 key/value/metadata 整体复制一份，大 value 写入时峰值内存翻倍。

## 现状

- 同一语义（“大 value 是 segment 编码，读时按 segment 映射还原”）在三个读接口里各写一遍；
  同一语义（“超过阈值的 upsert 先落 segment，再作为普通 entry 进 data page”）在两个写接口里各写一遍。
- 两份 put 的提交/回滚顺序必须保持一致（当前都是 `segment_mapper.begin_batch` → 写 segment →
  `put` → commit/abort）；任何一处改动（例如失败时释放已写 segment）都要同步两份，容易漏。
- `entry.clone()` 是纯搬运：只有真正走 large path 的 entry 需要修改 value/metadata，
  未命中的 entry 完全可以按引用传递；当前实现让每个 `PutLarge` 请求的整个 batch 多一次深拷贝。

## C++ 实现

C++ 明确把共享核心抽出来：
- `include/tasks/batch_write_task.h:176-195`：`WriteLargeValue(entry)` 只是分派，
  两条路径（`IoStringBuffer` / pinned contiguous）都转发到
  `WriteLargeValueSegments(actual_length, ptrs, buf_indices, metadata)`。
- 读取侧 `include/tasks/task.h:133-170`：`GetLargeValue` / `GetLargeValueContiguous`
  只是入口差异，segment 读取与 metadata 解析共用。

## 建议

1. 抽 `fn put_large_impl(&mut self, tbl, batch, sink: ValueSink<'_>)`，其中
   ```rust
   enum ValueSink<'a> {
     Owned,
     Fixed(&'a mut FixedBuffer),
   }
   ```
   `match sink` 只包住 `write_large_value` / `write_value_fixed_with_len` 一行；
   `put_large` / `put_large_fixed` 变成两个薄包装。枚举静态分派，不引入 dyn。
2. 读取侧抽 `fn resolve_large_value(&self, tbl, key, now_ms, dst: ValueDst<'_>)`，
   `ValueDst::Owned` 返回 `LargeValue`，`ValueDst::Buffer(&mut FixedBuffer)` 返回
   `LargeValueInto`；`get_large` / `get_large_into` / `floor` 共用 entry 查找与 TTL 过滤。
3. 去掉无条件 `entry.clone()`：遍历 `&[BatchEntry]` 时只对命中 large path 的 entry
   构造 owned 版本，其余用 `Cow`/引用传递；`write_li` 里存 `Cow<'_, BatchEntry>` 或
   `(index, Option<BatchEntry>)`，避免整批复制。
4. 为 `put_large_fixed` 的 `entry.value.len() > buf.len()` 分支补一个与 plain 路径一致的
   错误码测试，确保抽取后行为不分叉。

## 严重度

P2（约 200 行重复 + 大 value 写入峰值内存翻倍；无语义分叉）

## 核验记录（2026-11-04）

当前行号实读：`put_large` `:1821-1892`、`put_large_fixed` `:1896-1975`、
`get_large_into_at` `:1988-2040`、`floor` `:2044-2086`、`get_large_at` `:2092-2138`；
两处 `entry.clone()` 在 `:1837` / `:1914`。逐段对照确认两份 put 仅写 segment 的调用不同，
三份读仅“返回 owned Vec / 写入 FixedBuffer”不同。
C++ `WriteLargeValueSegments` 共享核心引用自 `include/tasks/batch_write_task.h:176-195` 实读。

## 审查结论（2026-11-04）

合理，全部采纳。实读核对（store 经 eng-64/68/76/core-22 改动后行号已漂移）：

- 写：`put_large`（原 `:1811`）与 `put_large_fixed`（原 `:1886`）仅 value 来源不同，其余骨架逐行同构。
- 读：`get_large_into_at` / `floor` / `get_large_at` 的 entry 查找、TTL、large 分支、segment 读取重复。
- `entry.clone()` 只在 `put_large` 的循环里（原 `:1827` / `:1904`），命中 large path 时整份大 value 多复制一次。

拆分：单一改动面（同一文件同一段落），合并为 1 个 todo 并已实现；不拆多文件以免互相冲突。

## 实现方案

- 写侧：私有 trait `LargeSink<I>`（`prepare` + `write`），`PlainSink` / `FixedSink` 两个零 dyn 实现；
  `put_large_impl` 承载开批、逐条落 segment、branch 段数复查、commit/abort；两个 pub 方法变薄包装。
- 读侧：私有 trait `ValueDst<I>`（关联类型 `Out`），`OwnedDst`（→ `LargeValue`）/
  `BufferDst`（→ `LargeValueInto`）；`Store::read_entry_at` 统一 entry 查找 + TTL 下推，
  `Store::resolve_value` 统一「非 large 直投 / large 走 segment」，`floor` 复用 `OwnedDst`。
- 去 clone：命中 large path 的 entry 用 `large_entry()` 只搬 key/时间戳并装入新的编码 content，
  不再复制原始大 value；未命中 entry 的值必小于阈值（或已是编码 content），clone 成本有界。
- 行为等价：错误码（`InvalidArgs` / `ResourceMissing` / `OutOfSpace`）、batch 回滚、
  `large_value`/`metadata.clear()`、fixed 尾部保留区不写出均保持不变。

## 验收

- `cargo nextest run -p fkv_store -p fkv_task_read -p fkv_task_write --no-fail-fast`：101 passed。
- `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings`：零警告。
- 新增 `large_value_paths_are_byte_equivalent`（阈值/段边界/多段逐字节对比 plain vs fixed、owned vs buffer、floor）。
- 新增 `put_large_fixed_rejects_value_larger_than_buffer`（buffer 不足报 `InvalidArgs`，整批回滚不落 segment）。

## DONE（2026-11-04）

改动文件：

- `crate/engine/fkv_store/src/lib.rs`：`LargeSink`/`PlainSink`/`FixedSink`/`large_entry`/`put_large_impl`；
  `ValueDst`/`OwnedDst`/`BufferDst`/`read_entry_at`/`resolve_value`；三条读路径与两条写路径去重。
- `crate/engine/fkv_store/tests/large_value.rs`：两个等价性 / 错误码测试。

遗留：

- `put_large_impl` 对 `segment_mapper_map.get_mut` 缺失仍返回 `ResourceMissing`（原为 `?` 直接返回，
  该分支在 `entry().or_insert` 之后不可达，现统一走 abort 路径，语义更严）。

## 复审（2026-11-05 r6b）

通过。

- 写侧统一：`LargeSink` trait（`crate/engine/fkv_store/src/lib.rs:2390`）+ `PlainSink`（`:2405`）/ `FixedSink`（`:2426`）；`put_large`（`:2472`）与 `put_large_fixed`（`:2577-2578`）均为 `put_large_impl`（`:2476`）薄包装。
- 读侧统一：`ValueDst`（`:2583`）+ `OwnedDst`（`:2600`）/ `BufferDst`（`:2628`）；`read_entry_at`（`:2666`）统一 entry 查找 + TTL，`resolve_value`（`:2684`）统一 large/非 large 分支，floor 复用 `OwnedDst`（`:2758-2764`）。
- 测试：`crate/engine/fkv_store/tests/large_value.rs:527 large_value_paths_are_byte_equivalent`、`:585 put_large_fixed_rejects_value_larger_than_buffer` 在盘。
