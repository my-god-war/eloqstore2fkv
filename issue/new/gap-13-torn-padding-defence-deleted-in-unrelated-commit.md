# gap-13 撕裂 padding 防线被无关 commit 删除，且该 commit 是红的就已落地（P1，上游已由 0ba0ed5 修复）

## 来源

为 gap-10 写「撕裂 manifest 物化」用例时，用例先红后绿，定位到是**已提交代码**的回归，
而非用例本身的问题。本条记录事实与流程缺口，避免同类再次发生。

## 现象

fkv commit `4e247c2`（提交信息 `ci: fail fast when RustFS init hangs`）在改动
`.github/workflows/ci.yml` 的同时，**删除了** `crate/engine/fkv_task_reopen/src/lib.rs`
中 `split_manifest_with_len` 的撕裂 padding 分支：

```diff
     if next > bytes.len() {
       // body 完好但 padding 未落盘：视为未提交，丢弃本 record。
       break;
     }
     if corrupted {
       return Err(KvError::Corrupted);
     }
```

删除内容与提交信息（CI / RustFS）无关，形态上是被 `git add -A` 一并扫进来的散落改动。

## 证据

```
$ git diff b5073e1 4e247c2 --stat
 .github/workflows/ci.yml                | 6 +++++-
 crate/engine/fkv_task_reopen/src/lib.rs | 4 ----
```

该 commit **落地时就是红的**：它自己的用例当场失败 ——

```
$ cargo nextest run -p fkv_task_reopen
    FAIL [ 0.025s] fkv_task_reopen::reopen split_manifest_torn_tail_discards_last_record
     Summary [ 0.028s] 5 tests run: 4 passed, 1 failed
```

`crate/engine/fkv_task_reopen/tests/reopen.rs:76`（fmt-08：「torn padding / torn body
丢弃最后一条 record」）明确断言 `split_manifest(torn_padding).unwrap().len() == 1`，
删掉分支后返回 2。

后果链（不只影响该单元测试）：`valid_len` 会**超出文件长度** → `reset_manifest(size)`
的 `set_len(size)` 把文件撑长 → 下一次 `append_manifest` 落在零洞之后 → 再 reopen
解析到零字节 header → `Corrupted`（对应用户可见的数据丢失）。

## 处置

**上游已修复**：commit `0ba0ed5`（`ci: rebuild manifest tests and use named RustFS volume`，
12:45）把同样 4 行加了回去 —— `git show 0ba0ed5 -- crate/engine/fkv_task_reopen/src/lib.rs`
的 diff 与上面引用的删除逐行互为反向。本轮在写 store 级用例时也独立恢复过一次，
内容与 `0ba0ed5` **逐字节一致**，因此被其吸收、没有产生重复改动（`git diff b5073e1 --
crate/engine/fkv_task_reopen/src/lib.rs` 为空）。

结论：回归在代码层面已经不存在，本条剩余价值是**把探测器留在门禁里**（见下）。
本轮全量验证：

```
$ cargo nextest run --workspace --all-features
     Summary [ 5.947s] 1002 tests run: 1002 passed, 1 skipped
```

## 建议

1. 保留本轮新增的**store 级探测器** `crate/engine/fkv_store/tests/torn_manifest.rs`：
   它把同一缺陷映射到用户可见后果（恢复状态、盘上长度、后续 append 可读性），
   与 `reopen.rs` 的单元级断言互为补充，且**无 C++ 依赖、纯 rust**，可进 PR 门禁。
2. **提交前跑最低限度的一组**：`cargo nextest run -p fkv_store -p fkv_task_reopen`。

## 追加复核（2026-11-05）：第 2、3 条的前提有误，更正

复核 CI 与钩子后，原「覆盖缺口」的判断需要修正：

- **CI 本来就跑全工作区**：`.github/workflows/ci.yml:37`
  `cargo nextest run --workspace --all-features --no-tests=pass`，
  `linux-test.yml:44` 与 `test.sh` 同样是工作区级。**不存在「只跑子集而漏掉」**，
  故不需要新增 CI 门禁。
- **真正的空转在本地钩子**：`core.hooksPath=.husky/_`，`.husky/pre-commit` 为

  ```sh
  cargo +nightly fmt --check || true
  command -v codegraph >/dev/null 2>&1 && codegraph sync || true
  git add -u || true
  ```

  即 **fmt 检查被 `|| true` 吞掉、且完全不跑测试**；钩子只做 `git add -u`。
- **红 commit 能落地的实际原因是没推**：该批 commit 当时相对 `origin/main` 领先 6 个，
  CI 从未跑过它们（现已推送）。

⇒ 修正后的剩余动作只有两条，且都不需要动 CI：

1. 把 `.husky/pre-commit` 的 `cargo +nightly fmt --check || true` 改成**阻断式**
   （去掉 `|| true`）；是否再挂一个「受影响 crate 的 nextest」由维护者定代价。
2. 提交/推送前自觉跑一次工作区测试；守门责任在 CI，而 CI 只在**推送后**才介入。

## 验收

- `split_manifest_torn_tail_discards_last_record` 与
  `torn_manifest::torn_manifest_tail_recovers_last_complete_record` 同时为绿；
- 提交前检查清单包含 `-p fkv_task_reopen -p fkv_store`；
- 后续同类「无关 commit 顺带删除防线」能被 `git diff <prev> <new> --stat` 审阅时一眼看到
  （建议 review 时对与主题不符的文件改动强制要求说明）。
