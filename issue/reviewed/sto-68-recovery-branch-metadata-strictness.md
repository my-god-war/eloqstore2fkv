# sto-68 P3：recovery 分支元数据段比 C++ 严格：空段/缺段拒整个 manifest

来源：第 5 轮 read/gc lane 侦查（子代理摘录；触发面低，边界对齐）。

## 证据

`crate/engine/fkv_recovery/src/lib.rs:160-162`（apply_log）/`:117`（from_snapshot）：

```rust
if cur.remaining() >= 4 {
  self.branch_metadata = cur.branch_metadata()?;   // len==0 → deserialize(&[]) → Err(EndOfFile)
}
...
let branch_metadata = cur.branch_metadata()?;      // from_snapshot：无段直接 Err
```

C++（`replayer.cpp:261-266` log / `:191-206` snapshot）：

```cpp
if (branch_meta_len > 0 && remaining.size() >= branch_meta_len) { ... }
// 否则静默跳过，保留旧 metadata 继续 replay
```

另 `:119` mapping 段用 `remaining() > 0` 而 C++ 用 `>= 4`（1-3 字节残留
Rust 报错、C++ 忽略）。

## 问题

- `branch_meta_len == 0` 的 log record：C++ 保留旧值继续，Rust 整条 replay
  失败 → 表不可开。
- 无 branch 段的旧格式 snapshot：C++ 默认值接受，Rust 直接 Err。
- 现实触发需手造/旧数据（双方 writer 恒写非空段），故 P3；但按盘面兼容口径
  应对齐或书面记录。

## 目标

- len==0 / 段缺失：保留旧值（或默认值）继续，对齐 C++。
- mapping 段长度判定改 `>= 4`（对齐 C++ 的最小 varint/长度前缀语义），或记录
  有意偏离（严格性是 fkv 的防损坏选择）。

## 验收

- 测试：空 branch 段的 log record 与无 branch 段的 snapshot 均可 replay；
  1-3 字节尾残留的行为与决策一致。
- `cargo nextest run -p fkv_recovery`。

## 依赖 / 重叠

- sto-51/51a（撕裂尾自愈，done）不同点（那是尾损坏，这是合法空段）。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上。

## DONE（2026-11-04）

- 复核 C++（`/Users/z/git/db/kv/eloqstore/src/replayer.cpp`）：
  - snapshot `:188-206` / log `:261-274`：branch 段只在 `remaining >= 4 && len > 0 && remaining >= len` 时解析，否则保留旧值。
  - snapshot `:208` / log `:292`：segment 段判定同为 `remaining >= 4`。
  - 结论：issue 证据成立，Rust 原实现确实更严（`len==0`、声明长度越界、无段 snapshot 全部 Err）。
- `crate/engine/fkv_recovery/src/lib.rs`
  - 删除 `Cursor::branch_metadata`，新增 `branch_metadata_opt() -> Result<Option<_>>`：
    不足 4B / `len == 0` / `len > remaining` 三种情况只消费 4B 长度前缀并返回 `None`，
    与 C++ 一致；负载存在但解析失败仍报错（C++ 同样 `LOG(FATAL)`）。
  - `from_snapshot`：branch 段缺失时用 `main`/`term=0` 默认值继续（C++ 为空名默认值，
    fkv 选 main 以免 GC key 变空；不影响盘面字节）。
  - `from_snapshot` segment 段判定 `remaining() > 0` → `>= U32_BYTES`，1-3 字节尾残留按缺段处理。
  - `apply_log`：branch 段用 `branch_metadata_opt`，空段 / 越界段保留旧 metadata。
  - 新增常量 `U32_BYTES` 取代裸 `4`。
- 有意保留的严格点：log segment delta 段（以及 snapshot mapping 段）声明长度超过剩余字节时
  `take` 报 `EndOfFile`，C++ 用 `substr` 静默截断。这是防损坏选择，不在本 issue 对齐范围内，
  现状记录于此。
- `crate/engine/fkv_recovery/tests/recovery.rs` 新增 4 个用例：
  - `snapshot_without_branch_section_replays`
  - `snapshot_tolerates_short_tail`（1-3 字节尾残留）
  - `log_with_empty_branch_section_keeps_old_metadata`
  - `log_with_oversized_branch_section_keeps_old_metadata`

验证：

- `cargo check -p fkv_recovery -p fkv_task_reopen -p fkv_store` → 通过。
- `cargo nextest run -p fkv_recovery -p fkv_task_reopen -p fkv_store --no-fail-fast` → 124 passed。
- `cargo clippy -p fkv_recovery -p fkv_task_reopen -p fkv_store --no-deps --all-features --all-targets -- -D warnings` → 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_recovery/src/lib.rs:48`（`const U32_BYTES`）、`:87-96`
（`branch_metadata_opt`：不足 4B / `len==0` / `len > remaining` 只消费前缀返回 `None`，
负载解析失败仍报错）、`:133-136`（snapshot 缺 branch 段用 main/term=0 默认）、
`:140/:184`（segment 段判定 `>= U32_BYTES`，1-3 字节尾残留按缺段处理）、
`:181-183`（apply_log 空段保留旧 metadata）。测试
`fkv_recovery/tests/recovery.rs:230/:245/:260/:290` 四用例齐全（:103 还断言缺段不改
已有 branch metadata）。「有意保留的严格点」（声明长度越界仍报 EndOfFile）在 DONE 记录。
