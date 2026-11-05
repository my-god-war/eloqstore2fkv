# perf-84 P2：`Store::scan*` 全家族整范围物化，`max_entries` / TTL 过滤双双不下推

来源：第 5 轮 store/write + read/gc 双 lane 独立发现，协调者亲验。

## 证据（2026-11-05 核验）

`crate/engine/fkv_store/src/lib.rs:937-981`（`scan_entry_opts`，`scan`/`scan_at`/
`scan_with_options`/`scan_entry*` 全部汇入）：

```rust
let entry_li = scan_entries_with(
  &self.io,
  ScanArgs {
    ...
    max_entries: 0,            // 硬编码 0 = 任务层不限量
    ttl_filter_now_ms: None,   // TTL 下推关闭
  },
  options,
).await?;
let mut result_li = Vec::new();
for entry in entry_li {
  if self.ttl_read_filter && is_expired(entry.expire_ts, now_ms) { continue; }
  result_li.push(KvEntry::from(entry));
  if max_entries != 0 && result_li.len() >= max_entries { break; }   // 事后截断
```

`ScanArgs` 明明支持两个下推旋钮；同文件 `scan_entry_page_at`（:853 注释
「过期条目不读 overflow、不占分页配额」）与 shard 分页路径
（`fkv_shard:1214` 走 `scan_page`）都是对的，唯独非分页入口没接上。

C++（`scan_task.cpp:323-346`）逐条限量 + 惰性物化 value + 字节级上限三重下推。

## 问题

`scan(begin, end, max_entries=10)` 读完整个叶子链：每条 entry（含过期条目）
的 overflow 链 IO + 解压全部发生后丢弃。内存 O(全区间)、IO 放大无界。
eng-52（scan 分页下推）/ eng-64/64a（点读 TTL 下推）修掉的同一病灶在本入口
的漏网。

## 目标

`scan_entry_opts` 把 `max_entries` 与 `ttl_filter_now_ms`（`ttl_read_filter`
开启时传 `now_ms`）透传给 `ScanArgs`；`KvEntry::from` 的转换在消费端保持。
顺带评估字节级上限（C++ `page_size`）是否需要——先记录差异，不强制。

## 验收

- 测试：区间 1000 条、`max_entries=10` + 中间夹过期条目 → 断言只物化必要页
  （可用 IO 计数器/failpoint 观察）；返回结果与现状一致。
- `cargo nextest run -p fkv_store -p fkv_sdk`。

## 依赖 / 重叠

- eng-52/eng-64 族（done）的补齐；perf-81a（done，返回类型）不相关。

## 核验记录

- 本人实读：`fkv_store/src/lib.rs:925-985`。
- 子代理摘录（未复核）：C++ 行号、`fkv_shard:1214`。

## 核验结论（2026-11-04）

issue 成立。实读 `crate/engine/fkv_store/src/lib.rs:942-989`：`scan_entry_opts`
仍传 `max_entries: 0` / `ttl_filter_now_ms: None`，事后过滤 + 截断。
perf-78a 只改了预取 future 形态、sto-62a 只改 shard 取消契约，均未触及此处语义。

## DONE（2026-11-04）

### 实现

- `Store::scan_entry_opts` 把 `max_entries` 与
  `ttl_filter_now_ms: self.ttl_read_filter.then_some(now_ms)` 直接透传给
  `ScanArgs`，删除消费端的事后 `is_expired` 过滤与 `len >= max_entries` 截断，
  改为一次 `map(KvEntry::from)` 单遍转换。
- `scan` / `scan_at` / `scan_opts` / `scan_with_options` / `scan_entry*` 全部经此入口，
  一并获得逐条限量与 TTL 下推。
- 清理 `fkv_task_write` 导入中不再使用的 `is_expired`。
- 扫描语义保持不变：结果集合、顺序、`begin/end` 开闭区间、停止条件、
  分页 `has_remaining` 口径均复用 `scan_page_with` 既有逻辑（与 `scan_entry_page_at` 同一路径）。

### 语义差异（已确认可接受）

- `LargeValueUnsupported`：旧实现会扫完整个区间，limit 之后的大 value 也会报错；
  下推后按 C++ `ScanTask` 逐条限量，达到 `max_entries` 即停止，limit 之后不再触碰。
  limit 之前的大 value 仍报错（`compat.rs::scan_rejects_large_value_but_truncate_still_works` 覆盖）。
- 字节级上限（C++ `page_size`）：本次不接入，`scan_entries_with` 仍用
  `PageOption::default()`（page_size=0 不限）。记录差异，留待需要时另开 issue。

### 测试

- 新增 `crate/engine/fkv_store/tests/scan_pushdown.rs`：
  - 1000 条区间、每 7 条夹一条过期项、`max_entries=10`：
    结果与全量扫描前 10 条一致，过期项不占配额；读页数 `limited*8 < full`
    且 `limited <= 8`（`MemStore::read_num` 计数）。
  - `ttl_read_filter=false`：过期项照常返回，仍按 limit 提前停止。
- 反向验证：临时还原 `max_entries: 0` / `ttl_filter_now_ms: None`，两个测试均失败。

### 验收结果

- `cargo check -p fkv_task_scan -p fkv_sdk` 通过。
- `cargo nextest run -p fkv_task_scan -p fkv_sdk -p fkv_store --no-fail-fast` 117/117 通过。
- `cargo clippy -p fkv_task_scan -p fkv_sdk --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:1004-1025`（`scan_entry_opts`）：`max_entries` 与
`ttl_filter_now_ms: self.ttl_read_filter.then_some(now_ms)` 直接透传 `ScanArgs`，
消费端只剩 `map(KvEntry::from)` 单遍转换，无事后过滤/截断。测试
`fkv_store/tests/scan_pushdown.rs:22-68`：1000 条区间 + `max_entries=10` + 每 7 条过期项，
断言结果与全量前 10 条一致（:56）且 `limited_reads*8 < full_reads`、`limited_reads <= 8`
（:59-66）；`:72-89`（关闭 TTL 过滤行为不变）。DONE 声称的字节级上限不接入
（`PageOption::default()`）与代码一致。
