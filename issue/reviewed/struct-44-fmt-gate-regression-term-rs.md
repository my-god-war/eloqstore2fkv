# struct-44 `0b87909` 引入 fmt 门禁回归：term.rs 溢出断言行超 `fn_call_width` 一格，stable / nightly 双通道红

- 领域：结构 / 代码质量（fmt 门禁 × 修复验证流程缺口）
- 优先级：P3（零行为影响、一行机械修复；但门禁表在本轮 HEAD `0b87909`
  实测为红，收敛判定「门禁全表绿」的前提被打破）
- 置信度：高（双工具链 `--check` 实测复现 exit 1；违规行属 `0b87909`
  新增测试模块；第 29~31 轮记录均为「fmt 双通道 0」）

## 现象（fkv `0b87909`，worktree `/tmp/r29/fkv` 干净检出）

```text
$ cargo fmt --all -- --check ; echo $?          # → 1
$ cargo +nightly fmt --all -- --check ; echo $? # → 1（输出逐字节一致）
Diff in crate/io/fkv_io_compio/src/term.rs:128:
     assert_eq!(parse(b"5x").unwrap_err(), KvError::Corrupted);
     assert_eq!(parse(b"+5").unwrap_err(), KvError::Corrupted);
     // 溢出 u64：Corrupted。
-    assert_eq!(parse(b"18446744073709551616").unwrap_err(), KvError::Corrupted);
+    assert_eq!(
+      parse(b"18446744073709551616").unwrap_err(),
+      KvError::Corrupted
+    );
```

全仓唯一违规：`crate/io/fkv_io_compio/src/term.rs:131`（hunk 起始行号报 128）。

## 根因

- 行总宽 80 字符，**不是** `max_width=100` 触发：`assert_eq!` 宏实参
  `parse(b"18446744073709551616").unwrap_err(), KvError::Corrupted`
  宽 61 字符，超 rustfmt `fn_call_width`（默认 60）恰 1 格，rustfmt
  要求拆行。同函数 `:119` 的 `… .unwrap(), u64::MAX`（实参 48）不受影响。
- 该行属 `0b87909` 为 sem-70 新增的 `mod tests`（`parse_rejects_malformed_content`
  溢出断言）；`term.rs` 此前无测试模块，违规不可能来自更早提交
  （`--check` 按文件独立判定，HEAD 仅此文件被标）。
- 第 31 轮 sem-70 实现记录验证清单为「`fkv_io_compio` 全套绿（155 例）」
  ——跑了测试、漏了 `cargo fmt --check`；`6f79f44` 提交信息明确带过
  「fmt 干净」，`0b87909` 未复验。修复验证流程（struct 域门禁表）未把
  fmt 纳入逐提交必跑项是流程缺口。

## 影响

- 门禁表：本轮 HEAD fmt 双通道 exit 1（第 29/30/31 轮均为 0）；其余
  门禁（行数 / 分层 / clippy 含 absolute_paths / shear / unused 基线
  2 误报）全绿。
- 行为零影响：`cargo fmt` 重排仅缩进 / 换行，测试语义不变。

## 修复建议

`cargo fmt --all`（或手拆该断言为 4 行）后提交；无需改动任何逻辑。

## 验收方式

1. 新 HEAD 上 `cargo fmt --all -- --check` 与
   `cargo +nightly fmt --all -- --check` 双通道 exit 0；
2. `git diff` 仅 term.rs 该断言换行，无其它文件漂移；
3. 门禁全表复跑全绿（本条为第 32 轮结构域唯一新发现）。

## 伴随观察（不阻塞、不单独立项）

- `crate/engine/fkv_store/tests/page_cache.rs:1` 文件头注释仍写「写路径页
  缓存……覆盖任务的三条验收」，文件经 `6f79f44` 已扩展为读写共用（读路径
  三个小节各有准确小节头）；文件级 scope 措辞偏窄，可在下一次触碰该文件
  时顺手改为「读写共用页缓存」，不构成独立立项。
- `fkv_config/src/lib.rs` 998/1000（+4 来自 sem-69 回退行）：2 行余量为
  机械事实，本轮不立项；该文件后续增长建议落新模块（INI 解析拆分）而非
  续压主文件，触线即另立拆分项。

## 实现记录（2026-11-07）

- 修复 fkv commit：`1b064a4`（term.rs 溢出断言按 rustfmt 换行）。
- 复验：`cargo +nightly fmt --all -- --check` 与 stable 双通道 exit 0；根因
  确认为第 31 轮 sem-70 实现的验证清单漏跑 fmt（已在该记录勘误口径中
  记录）。伴随观察（page_cache.rs 文件头措辞偏窄 / fkv_config 998 行余量
  2 行）保留观察不立项，后者待触线时立拆分项。

## 复审记录（2026-11-07，第 33 轮结构域）

- 判定：**达标**，移入 reviewed/。`1b064a4` 纯格式 diff；HEAD `4c40a75`
  fmt 双通道 exit 0 复验。
