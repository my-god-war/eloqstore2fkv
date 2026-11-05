# eng-64 点读 TTL 过滤在 value 物化之后：过期 overflow/压缩 key 白付整条 IO 链 + 解压 CPU（eng-52 只下推了 scan）

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:448-463`（`get_at`：`read_entry(...).await?` 已完成 overflow 链读取 + 解压，之后才 `if self.ttl_read_filter && is_expired(...) return Ok(None)`，`:461`）；对照同文件 `:2004-2006`（`get_large_into_at` 过滤在 `read_value_fixed` **之前**，重 IO 的 segment 读不发生）—— 同仓两条路径行为不一致
- C++: `src/tasks/read_task.cpp:31-82`（`LocateAndProcess` 无过期判断，`expire_ts` 仅作出参透传 `:80`）；`read_task.h:30-57`（Read 用 `extract_metadata`/`large_value` 参数把元数据判定与 value 物化解耦）

## 现状
`read_entry` 一次性物化 entry.value；过期判定只能在其后。过期大 value 点读付出全部 IO 后丢弃。

## C++ 实现
元数据（expire_ts）与 value 物化是两个可选阶段，调用方可先判过期再决定是否取 value。

## 差异与风险
过期 key 的点读成本 = 活 key 成本；TTL 语义上「读不到」却付了「读到」的价。与 eng-52（scan 下推）相邻但入口不同，非重复。

## 对齐建议
- `read_entry` 提供两段式：先返回元数据（expire_ts/is_large_value/overflow 指针），调用方判定后再物化 value（对齐 C++ Read 的参数化解耦）；
- 最小改法：`read_entry` 内对 overflow entry 先读首 overflow 页取 expire_ts 判定？不可行（expire_ts 在 data 页 entry 头）—— 实际上 expire_ts 在 entry 编码头即可取得，物化前过滤只需把「解码 entry 头」与「读 overflow/解压」拆开。

## 严重度
P2（性能 + 两条 get 路径行为不一致）

## 核验记录（2026-11-04）
`get_at`（`:461`）与 `get_large_into_at`（`:2004`）过滤位置对照实读确认；C++ 无过期判断引用自子代理摘录。

## 审查结论（2026-11-04）

### 成立
- 行号复核：`get_at` 实际 `crate/engine/fkv_store/src/lib.rs:441-469`，过滤在 `:461`；
  `get_large_into_at` 实际过滤在 `:2014`（issue 写的 `:2004-2006` 已漂移 8 行左右）。
- 事实确认：`ttl_read_filter` 默认 `true`（`lib.rs:165`），所以热路径默认受影响。
- 成本确认：`read_entry`（`crate/engine/fkv_task_read/src/lib.rs:103`）在
  `DataPageIter` 命中后无条件执行 `read_overflow`（整条 overflow 链 IO）+
  `decompress_value`（解压 CPU），之后调用方才判过期。过期大 value 白付全链成本，
  结论不夸大。
- 同类入口一致性问题确凿：`get_at`(:461) / `get_large_at`(:2117) /
  `get_large_into_at`(:2014) 三处重复同一段「读后过滤」，且都与 scan 的
  `ttl_filter_now_ms` 下推（`fkv_task_scan/src/lib.rs:207`）实现风格不一致。

### 修正
- issue 建议的「两段式 read_entry（先返元数据再物化）」过重：调用方需要重新持有
  页缓冲、再决定是否补读 overflow / 解压，等于把 `read_entry` 内部状态机外泄。
- 实际最小且与 eng-52 对齐的做法：把 `expire_ts` 判定**下推进 `read_entry`**，
  在定位到 data page entry、拿到 `iter.expire_ts()` 之后、`read_overflow` /
  `decompress_value` 之前返回 `None`。语义与现有事后过滤等价（只读 `expire_ts`
  与 entry 头），`expire_ts == 0`（永不过期）与 tombstone 语义不变。
- 该改动必须落在 `crate/engine/fkv_task_read/src/lib.rs`（约 3 行 + 签名），
  无法只靠 `fkv_store/src/lib.rs` 完成；调用点仅 5 处，改动面很小。

### 结论
合理，落 `issue/todo/eng-64-point-read-ttl-pushdown.md`。

## 复审（2026-11-05 r6）

通过。本文件为问题原始记录，结论指向 eng-64a（pushdown），其实现与测试已按当前源码复核：
`fkv_task_read/src/lib.rs:287-289`（`read_entry` 在 `iter_value` 即 overflow/解压之前判过期返回
`None`）、`fkv_store/src/lib.rs:616-626`（`read_ctx_at`）。详见同目录
`eng-64-point-read-ttl-pushdown.md` 的 r6 复审。
