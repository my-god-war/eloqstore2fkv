# dead-08 `tool/fkv_ledger` 死代码：`ClassifySummary::changed()` 无调用者、`invalid` 字段恒为 0（从未被赋值）

- 领域：死代码 / 代码质量（issue 仓工具面；非 fkv 生产代码）
- 优先级：P4（纯清理，零行为影响，一行级删除；但现存无任何门禁覆盖）
- 置信度：高（逐符号全仓 grep + clippy 干净 + 手工审计；`fkv_ledger` lib+bin，pub 项不被 `dead_code` lint 报）

## 现象（issue 仓 HEAD，`tool/fkv_ledger`）

`tool/fkv_ledger/src/classify.rs` 有两处死代码：

```rust
// classify.rs:42-50
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassifySummary {
  pub mapped: usize,
  pub ignored: usize,
  pub unchanged: usize,
  pub skipped_terminal: usize,
  pub missing: usize,
  pub invalid: usize,      // ← 全仓从未被赋值
}

// classify.rs:62-64
impl ClassifySummary {
  // ...
  pub fn changed(&self) -> usize {   // ← 全仓零调用者
    self.mapped + self.ignored
  }
}
```

- `changed()`：`grep -rn '\.changed()' .`（全 issue 仓，含 `tool/fkv_ledger/{src,tests}`）
  **零命中**；同名不同物的 `ledger::changed_units` / `changed_unit_li` 不算。
- `invalid`：全仓仅出现在字段声明（`classify.rs:49`）与 `total()` 的求和项
  （`classify.rs:59`），**没有任何 `summary.invalid += 1` 之类的写入**
  （`apply` 只写 `missing`；非法行、非法 map target 都直接 `return Err`，fail-fast）。
  故 `invalid` 恒为 0，`main.rs:270-277` 打印汇总时也不含该字段
  （`classify: {total} rows, mapped/ignored/unchanged/skipped_terminal/missing`）。

## 根因

- 早期设计疑似「逐行非法计数」，后改为 fail-fast（`parse` 整份报错、`apply` 非法 target
  即 `Err`），`invalid` 失去生产者、`changed()` 失去消费者，二者均未随删。
- `tool/fkv_ledger/src/lib.rs` 全为 `pub mod`，`ClassifySummary` 是 pub API，`changed()`
  不会被 `-D warnings` / `dead_code` lint 命中；fkv 仓的 `bun sh/unused.js`
  （scip unused-pub + cargo-shear）只扫 fkv 仓，**不覆盖 issue 仓工具**。
  `.agents/skills/clippy/SKILL.md` 明确要求「清理不用的死代码」，此处游离在门禁之外。

## 建议（一行级机械修复）

- 删除 `ClassifySummary::changed()`；
- 删除 `invalid` 字段及其在 `total()` 的求和项（保持 `total()` = mapped+ignored+unchanged+
  skipped_terminal+missing 的现值语义）；
- 若确需「不中断的逐行非法计数」，则应反向改造：`apply` 把非法行计入 `invalid` 并让
  `main.rs` 打印——二选一，勿留悬空字段。

## 验收方式

```bash
cd /Users/z/git/db/kv/eloqstore2fkv
grep -rn '\.changed()' tool/fkv_ledger        # 期望 0
grep -rn 'invalid' tool/fkv_ledger/src/classify.rs   # 期望：仅剩必要项（不删则解释来源）
cargo clippy -p fkv_ledger --all-targets --all-features -- -D warnings   # 绿
cargo test -p fkv_ledger                      # 绿
```

## 处置记录

- 2026-11-08 第 38 轮结构 / 死代码域换面 spot check 发现（`tool/` 健康面，
  fkv 冻结 worktree `/tmp/r29/fkv` HEAD `6d3f11a` 取证；issue 仓工具未变）。

## 实现记录（2026-11-08）

- 修复（issue 仓 `tool/fkv_ledger`，无 fkv 代码改动）：删除 `ClassifySummary::changed()`
  （全仓零调用者）与从未被赋值的 `invalid` 字段（含其在 `total()` 中的求和项；
  `main.rs` 打印只用 mapped/ignored/unchanged/skipped_terminal/missing，
  `total()` 数值不变——`invalid` 恒 0）。
- 验证：`cargo +nightly clippy -p fkv_ledger --all-targets` 干净；
  `cargo run -p fkv_ledger -- classify --plan` 输出与改前一致
  （`543 rows, mapped 0, ignored 0, unchanged 543, skipped_terminal 0, missing 0`）；
  fmt exit 0。三账本复跑全绿。

## 复审记录（2026-11-08，第 39 轮结构域）

- 判定：**达标**，移入 reviewed/。`\.changed()` grep 0 命中；`invalid` 字段已删
  （`classify.rs:212` 残留的 `invalid` 为错误文案字符串，非字段）；
  clippy `-p fkv_ledger --all-targets` exit 0 零输出；`classify --plan` 输出
  与实现记录逐字一致。
