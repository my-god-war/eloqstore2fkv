# eng-64a 点读 TTL 过滤下推进 read_entry：expire_ts 判定先于 overflow/解压

## 位置
- `crate/engine/fkv_task_read/src/lib.rs:103-166`（`read_entry`）
- `crate/engine/fkv_store/src/lib.rs:441-469`（`get_at`）、`:2104-2135`（`get_large_at`）、
  `:2001-2040`（`get_large_into_at`）、`:1491`（`clean_expired`，保持不过滤）

## 现状
`read_entry` 命中 data page 后直接 `read_overflow` + `decompress_value`，过期判定只能
由调用方在整条 IO 链之后做（`get_at:461`、`get_large_at:2117`、`get_large_into_at:2014`
三处重复）。过期大 value 点读付全价。

## 方案
1. `ReadCtx` 增加字段 `ttl_filter_now_ms: Option<u64>`（`read_entry` 不再加第 8 个参数）：
   在 `iter.seek(key)? && iter.key() == key` 之后、value 物化之前，
   `ttl_filter_now_ms.is_some_and(|now| is_expired(iter.expire_ts(), now))` 则返回
   `Ok(None)`。`is_expired` 用 `fkv_types::is_expired`（`fkv_task_read` 已有依赖，
   避免新增 crate 依赖 / 循环）。
   - `ReadCtx::default()` / `read_value` 的既有调用保持 `None`（原语义不变）。
2. `fkv_store` 三个点读入口传 `self.ttl_read_filter.then_some(now_ms)`（与 `scan_at:572`
   同一写法），删除读到 value 之后重复的 `if self.ttl_read_filter && is_expired(...)`；
   语义等价：过滤与否仍由 `ttl_read_filter` 决定。
3. `clean_expired` / `floor` 传 `None`（它们需要取到过期项的元数据/值）。

## 验收
- `cargo check -p fkv_store`
- `cargo nextest run -p fkv_store`（新增：过期 overflow value 点读的 `page_io().read_num()`
  等于未过期读的次数减去 overflow 链页数，且返回 `None`）
- `cargo clippy -p fkv_store --all-features --all-targets -- -D warnings` 零警告
- 语义不变：`ttl_read_filter=false` 仍返回过期值；`expire_ts == 0` 永不过期；
  tombstone 路径不受影响。

## DONE（2026-11-04）

### 实现要点
- `crate/engine/fkv_task_read/src/lib.rs`：
  - `ReadCtx` 增加 `ttl_filter_now_ms: Option<u64>` 字段（默认 `None`）。
  - `read_entry` 在 data page 命中 key 后、`read_overflow` / `decompress_value`
    之前判过期，命中直接 `Ok(None)`；判定用 `fkv_types::is_expired`
    （`expire_ts == 0` 永不过期）。
- `crate/engine/fkv_store/src/lib.rs`：
  - 新增 `read_ctx_at(tbl, now_ms)`：`ttl_filter_now_ms = ttl_read_filter.then_some(now_ms)`。
  - `get_at` / `get_large_at` / `get_large_into_at` 改用 `read_ctx_at`，删除三份
    读后重复过滤代码；`clean_expired` 继续用 `read_ctx`（`None`），保持能取到过期项。
  - 语义等价：过滤开关仍由 `ttl_read_filter` 决定，`get_at` 的 large value
    分支顺序（过期先于 `LargeValueUnsupported`）不变。

### 测试
- `crate/engine/fkv_store/tests/ttl.rs::expired_overflow_point_read_skips_overflow_chain`
  - 4096B overflow value + 过期时间戳：预热 index 缓存后，过期点读的
    `MemStore::read_num()` 增量与内联小 value 相同（不读 overflow 链）；
    未过期时仍拼回完整 4096B。

### 验证
- `cargo check -p fkv_store --all-targets` → 通过
- `cargo nextest run -p fkv_store` → 69 passed / 0 failed
- `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings` → 零警告
- `cargo clippy -p fkv_task_read --no-deps --all-features --all-targets -- -D warnings` → 零警告
- 注：不带 `--no-deps` 的 clippy 当前被并发改动中的
  `crate/core/fkv_mapping/src/lib.rs:273`（`useless_conversion`）挡住，与本次改动无关。

## 复审（2026-11-05 r6）

通过。逐项核验：

- `fkv_task_read/src/lib.rs:178`（`ReadCtx::ttl_filter_now_ms`，`ReadCtx::new` 默认 `None` :188）、
  `:285-289`（data page 命中后、`iter_value`（read_overflow + decompress）之前
  `expired(ctx, iter.expire_ts())` 命中即 `Ok(None)`），下推位置与方案一致。
- `fkv_store/src/lib.rs:598-626`：`read_ctx` 恒 `None`、`read_ctx_at` 传
  `ttl_read_filter.then_some(now_ms)`；`get_at`(:753)/`get_large*`(:2654/:2692) 走
  `read_ctx_at`；`clean_expired` 扫描仍 `ttl_filter_now_ms: None`(:2044)，过期项可取到。
- 测试 `fkv_store/tests/ttl.rs:133-176`：断言过期 overflow 点读的 `read_num` 增量与内联小
  value 相同（:168 `assert_eq!(expired_num, live_num)`），未过期仍拼回 4096B（:171-174）——
  真实覆盖「不读 overflow 链」。
