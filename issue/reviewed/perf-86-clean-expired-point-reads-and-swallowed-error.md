# perf-86 P2：`clean_expired` 每条过期 key 一次全树点读；内联清理错误被 `let _` 吞掉

来源：第 5 轮 store/write lane 侦查（子代理摘录 + 协调者结构复核）。

## 证据

`crate/engine/fkv_store/src/lib.rs:1912-1932` 一带：TTL 树扫出过期 key 后逐条

```rust
if let Some(data_entry) = read_entry(&self.io, tbl, root_id, mapper, user_key, ...)
  .await?
  && data_entry.expire_ts == expire_ts
```

每条 = 一次独立 root→leaf 下降（index cache 缓解但不消）。C++
`CleanExpiredKeys`（`batch_write_task.cpp:2043-2076`）TTL 顺序扫描直接构造
`data_batch` / `ttl_batch` 两个 batch，零点读。

另：`fkv_store/src/lib.rs:1261` 附近 `put_at` 内联清理：

```rust
} else {
  let _ = self.maybe_clean_expired(tbl, now_ms).await;   // 吞掉全部错误（含 Corrupted）
}
```

## 问题

1. N 条过期 = N 次树下降；大批过期时清理延迟放大。
2. Rust 的 `expire_ts == expire_ts` 复核在 C++ 由写路径 TTL 维护精确性保证，
   不需要逐 key 点读；且复核失败（不删主树）时 TTL 项照样删，语义与 C++ 出入。
3. 内联清理静默失败无日志，Corrupted 被吞。

## 目标

- 对齐 C++：TTL 扫描时按 user_key 排序的扫描序直接构造删除批（主树批 + TTL 批
  一起提交）；`expire_ts` 复核若保留，失败分支至少 log::warn 并维持 C++ 语义
  （TTL 项与主树项同删同留）。
- `maybe_clean_expired` 错误不吞：失败记 log（warn），返回值维持触发语义。

## 验收

- 测试：过期 N 条的清理只做常数次树遍历（failpoint 计数）；注入 Corrupted
  时有 warn 日志且 put 主流程不受影响。
- `cargo nextest run -p fkv_store`。

## 依赖 / 重叠

- sto-65/66 改 TTL/释放语义，先落它们再动本条（同函数族）；
  14/TTL 有界清理（done）是入口与触发面。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上；开发时验证。

## 审查结论（dragon 复核，2026-11-04）

**部分成立：性能项成立、埋错项成立，「语义与 C++ 出入」这条要改判。**

- **成立（性能）**：`clean_expired`（`fkv_store/src/lib.rs:1896-2000`）先用一次
  TTL 范围扫描拿到 `entry_li`（`max_entries: 0`，无上界），随后**逐条**
  `read_entry(&self.io, tbl, root_id, mapper, user_key, ...)` 复核主树
  （`:1982-1998`）—— N 条过期 = N 次 root→leaf 下降。C++ 侧确认零下降：
  `BatchWriteTask::CleanExpiredKeys`（`src/tasks/batch_write_task.cpp:2016-2078`）
  直接由扫描序构造 `ttl_batch` + `data_batch`。
- **成立（埋错）**：`put_at` 尾部 `let _ = self.maybe_clean_expired(tbl, now_ms).await;`
  （`:1272`，非原报告写的 1261）—— 失败（含 `Corrupted`）无任何痕迹。
- **改判（原报告第 2 条）**：Rust 的 `expire_ts` 复核**不是**「应与 C++ 对齐去掉的
  多余动作」，而是防御 C++ 不防御的那一类不变量破坏 —— 正是 sto-65/66 刚修的那两种
  （旧 ts delete 释放存活值、同 expire 覆盖删 TTL 索引）。C++ 能省掉复核是因为它
  假定 TTL 项与主树项严格一一对应；一旦该不变量被破坏，C++ 会按过期 TTL 项删掉一个
  `expire_ts` 更新的**存活** key，Rust 不会。另外「复核不过仍删 TTL 项」也是对的
  （陈旧 TTL 项本来就该清）。**判定：保留复核，改判为有意偏离并写入
  `doc/migration.md` §8.1；不要「对齐 C++」把复核删掉。**

## 阻塞（留 todo）

`fkv_store/src/lib.rs` 是并行会话热点（本轮多次抓到它正在改），代码改动交 owner。
落地清单（按收益/风险排序）：

1. `:1272` 改为 `if let Err(err) = ... { log::warn!(...) }`：保持「清理失败不影响写
   成功」的语义，但不再静默；`Corrupted` 必须可见。
2. 逐条复核的开销用「一次批量读」摊掉：当前无批量读接口（`read_entry_li` /
   `get_batch` 均不存在），两条路选一 ——
   (a) 低风险：给 `clean_expired` 的单次调用加过期条数上界（`max_entries` 取有限值 +
   下轮继续，`next_expire` 已有时序语义可直接复用），把尾延迟封顶；
   (b) 结构性：新增按叶子一次下降的批量读（`user_key` 来自 TTL 扫描序，已排序）。
   **先按 `fkv_bench` 出数再决定**（仓库已有 perf 看板），避免为未观测的收益改热路径。
3. 测试：注入 `Corrupted` 时有 warn 且 `put` 仍 Ok；上界生效时一次清理的树下降次数
   有常数上界（failpoint/计数）。

## DONE（2026-11-05）

### 改动

- `crate/engine/fkv_store/src/lib.rs`
  - `put_at` 内联清理：`let _ = self.maybe_clean_expired(...)` 改为
    `if let Err(err) = ... { log::warn!(...) }`。清理失败仍不回滚、不改变写成功
    语义；`Corrupted` 等错误现在可见。
  - `clean_expired`：TTL 过期范围扫描加单次上界 `CLEAN_EXPIRED_MAX_ENTRIES = 256`
    （`max_entries`），不再一次全量逐条点读。扫满上界时 `next_expire_ts` 压到
    `now_ms.max(1)`，由写路径 / `drain_pending_ttl` 下一轮继续；未扫满时仍 peek
    区间外第一条真实 `expire_ts`。单次清理的树下降次数因此有常数上界。
  - `expire_ts` 逐条复核**保留**：perf-86 复审确认这是有意偏离 C++，用于防御
    sto-65/66 修复的「TTL 索引与主树 `expire_ts` 不一致」不变量破坏；未删复核。
- `crate/engine/fkv_store/Cargo.toml`：`cargo add log`（0.4.35，std），供 warn 使用。
- `crate/engine/fkv_store/tests/clean_expired_bound.rs`（新增）
  - `inline_clean_error_is_warned_and_put_still_succeeds`：TTL 树根读取注入
    `Corrupted`，断言 `put_at` 仍 Ok、写入已提交、过期项保留、warn 含表名与
    `Disk data corrupted`，故障解除后下一次写入继续清理。
  - `clean_expired_is_bounded_and_continues_next_round`：259 条过期项首次只清
    256 条、`next_expire_ts == now`，第二轮清剩余 3 条并归零，验证上界与续清。

### 测试 / 检查

- `cargo nextest run -p fkv_store`：113 passed / 0 failed（含新增 2 条；libtest
  并行线程会共享 failpoint 全局状态，故按仓库约定用 nextest）。
- `cargo test -p fkv_store --test ttl` 等 TTL 相关二进制：8 passed。
- `cargo clippy -p fkv_store --all-targets -- -D warnings -A deprecated -A dead_code`：
  通过（`-A` 仅用于放行并发会话在 `fkv_metrics` / `fkv_io_compio` 引入的无关
  警告；fkv_store 本身零警告）。
- 未采用结构性批量读方案 (b)：按低风险方案 (a) 先封顶单次尾延迟，未为未观测收益
  改热路径；`fkv_bench` 未跑。

### 残留

- `doc/migration.md §8.1` 的「保留 `expire_ts` 复核」偏离记录不在本子任务允许的
  改动范围（仅 `fkv_store` 与必要测试），需文档 owner 补记。
- 超过 256 条的积压按轮次清理；无后续写入/ drain 时物理回收会延后（读侧 TTL 过滤
  不受影响）。


## 复审（2026-11-05 r6）

通过。`fkv_store/src/lib.rs:46`（`pub const CLEAN_EXPIRED_MAX_ENTRIES: usize = 256`）、
`:2041`（TTL 扫描 `max_entries` 取该上界）、`:2051-2054`（扫满上界 `next_expire_ts =
now_ms.max(1)`，未扫满 peek 真实下一条）、`:1339-1342`（内联清理失败 `log::warn!`，
写成功语义不变）；`expire_ts` 逐条复核保留（有意偏离 C++，:2005 注释）。测试
`fkv_store/tests/clean_expired_bound.rs:134`（注入 Corrupted：put 仍 Ok、写入已提交、
warn 含表名与 "Disk data corrupted"、故障解除后续清）与 `:197`（259 条过期：首轮 256 +
`next_expire_ts==100`，次轮 3 条归零）断言完整。
