# sem-74 `KvError::LargeValueUnsupported` 的 `Display` 文案单复数与 C++ 不一致（且索引「仅 `NotRunning` 文案不同」断言证伪）

- 领域：语义 / 错误面（`KvError` 全枚举 × C++ `error.h` 的映射完整性，含 `Display` 文案面）
- 优先级：P3
- 置信度：高（两侧 `as_str` / `ErrorString` 逐条自动对拍，唯一新增差异；索引断言可证伪）

## 现象

`fkv_error::KvError` 的 `Display` 委托 `as_str()`，逐条与 C++ `ErrorString` 对照，
**20 个变体中有 2 个文案不同**，其中一条此前从未登记：

| 变体 | Rust `as_str()` | C++ `ErrorString` |
|---|---|---|
| `NotRunning` | `"store is not running"` | `"EloqStore is not running"` |
| `LargeValueUnsupported` | `"Operation does not support very large value"` | `"Operation does not support very large **values**"` |

`NotRunning` 的差异已在 `issue/new/00-index.md:54` 记作已知
（「仅 `NotRunning` 文案不同，无行为差异」）；但 `LargeValueUnsupported` 的
**单复数差异（`value` vs `values`）未被任何账本登记**，且直接**证伪**索引
第 54 行「仅 `NotRunning` 文案不同」的断言。

## 证据

- Rust：`crate/foundation/fkv_error/src/lib.rs:35`
  `Self::NotRunning => "store is not running",`
  与 `:54` `Self::LargeValueUnsupported => "Operation does not support very large value",`
- C++：`eloqstore/include/error.h:52` `return "EloqStore is not running";`
  与 `:82` `return "Operation does not support very large values";`
- 全枚举对拍（脚本逐条 diff）：变体数与顺序 20/20 一致、`IsRetryableErr` /
  `is_retryable` 一致（均为 `OpenFileLimit` / `Busy` / `TryAgain`）；**唯一
  差异即上表两条**。
- 索引断言：`issue/new/00-index.md:54`「（仅 `NotRunning` 文案不同，无行为差异）」。
- 现有锁定测试 `crate/foundation/fkv_error/tests/error.rs` 的 `display_is_stable`
  只断言 `NoError` 与 `ExpiredTerm` 两条，**未覆盖** `LargeValueUnsupported`，
  故该差异不会被现有测试捕获。
- 该文案会经 `Display` 出现在用户可见面：`fkv_cli` 的
  `report_global`（`crate/facade/fkv_cli/src/main.rs:160`）以
  `eprintln!("{action} failed table={tbl} error={err}")` 打印；
  `Store::get` 命中 large value 时返回 `LargeValueUnsupported`
  （`fkv_store/src/read.rs`，见 COMPAT_BASELINE:370），CLI 的 `--mode` 各路径都会
  把它打到 stderr。

## 影响

- 仅 `Display` / 日志 / CLI stderr 的**文案面**差异，不改错误枚举值、可重试集合
  与任何行为；不涉及盘面格式与可见数据。
- 但索引把「仅 `NotRunning` 文案不同」当作已收敛结论，实际有第二条差异——
  该断言过强（与第 38 轮 sem-73 修正的「偏离仅限根不存在」同类问题）。
- 若差分器按 C++ stderr / 日志文案对拍错误字符串，`LargeValueUnsupported` 会被
  报成失败。

## 建议

（登记型，二选一）

1. 直接对齐 C++：把 `crate/foundation/fkv_error/src/lib.rs:54` 改为
   `"Operation does not support very large values"`（单字符级修正，无行为影响），
   并在 `display_is_stable` 增补该条断言钉住；`NotRunning` 是否一并对齐
   （`"EloqStore is not running"`）由维护者决定——它已被记为已知差异。
2. 若保留 fkv 文案：把 `LargeValueUnsupported` 与 `NotRunning` 一并登记进
   `COMPAT_BASELINE`「错误枚举」条目，并把 `issue/new/00-index.md:54` 的
   「仅 `NotRunning`」改为「`NotRunning` / `LargeValueUnsupported` 两条文案不同」，
   去掉过强断言。

倾向方案 1 的第一个变体（对齐 C++，消除差异）——文案面本就以 C++ 为 oracle，
单复数修正零成本。

## 验收方式

```bash
cd /tmp/r29/fkv
cargo test -p fkv_error            # display_is_stable（补断言后应覆盖两条）
grep -n "very large value" crate/foundation/fkv_error/src/lib.rs   # 对齐后应为复数
```

（本轮已跑：`fkv_error` 全套 39+5 例、`fkv_config` 全套全绿。）

## 实现记录（2026-11-08）

- 修复 fkv commit **`d097a4b`**：`fkv_error/src/lib.rs` 的
  `LargeValueUnsupported` 文案由「…very large value」改为与 C++
  `include/error.h:82` 逐字一致的「…very large value**s**」；
  `fkv_error/tests/error.rs` 的 `display_is_stable` 补该条断言（此前只钉
  NoError / ExpiredTerm 两条）。
- 附注：对齐后 `issue/new/00-index.md:54`「仅 `NotRunning` 文案不同」的断言
  **重新成立**（唯一剩余文案差异仍是 `NotRunning`，属已登记的有意差异）。
- 验证：`cargo nextest run -p fkv_error --all-features` 2/2 passed；fmt 绿。

## 复审记录（2026-11-08，第 40 轮结构域）

- 判定：**达标**，移入 reviewed/。文案与 C++ `include/error.h:82` 逐字一致；
  `display_is_stable` 含新断言；`fkv_error` + `fkv_config` 46/46 passed；
  索引「仅 NotRunning 文案不同」断言重新成立。
