# r8c-02 P3：rev-37 方案 A 落地后，05 恢复文档 reopen 段与 fkv_config 两字段注释仍是旧语义，DONE 自认残留且无归口 issue

## 现象（亲读）

rev-37（`issue/done/rev-37-auto-reopen-retry-policy-divergence.md`）按方案 A 落地：
driver 单次尝试失败即连坐、外部 `Reopen` 无同表 driver 时直接回执
`ResourceMissing`、`auto_reopen_retry_times` 语义改为「每读请求轮数」（与 C++
同名同义）。DONE「残留」段自认两处登记未做（受 lane 文件范围限制），且
`issue/new` / `issue/todo` 均无条目承接：

1. `doc/architecture/05-persistence-and-recovery.md:57-60` 仍写：
   「重试策略由 `fkv_shard::ReopenQueue` 按 `AutoReopenOption` 延迟重试（云模式
   manifest 延迟出现），其它错误立即返回」——描述的是落地前「driver 多轮延迟
   重试」的旧策略，与现行为（单次尝试 + 每读请求预算 + 外部 Reopen fail-fast）
   相反；也未登记「与 C++ 单次延迟尝试即连坐对齐」的决策。同文件 :44-46 的
   torn tail 段不受影响。
2. `crate/foundation/fkv_config/src/lib.rs:281` / `:283`：
   `auto_reopen_retry_times: u8 = 10` / `auto_reopen_pending_time_us: u64 =
   10_000_000` 两字段**无任何注释**。rev-37 开发计划第 1 条明确要求
   「`fkv_config` 两个 `auto_reopen_*` 字段注释与 INI 文档标明与 C++ 同名同义
   （fkv=每读请求预算）」——未兑现。相邻字段（`max_processing_time_us` :285、
   `yield_budget_us` :289、`data_append_mode` :270）都有语义注释，风格断档。
3. 对照面：`doc/architecture/06-gaps.md:81-82` 与 `issue/new/00-index.md` 的
   rev-37 行已是新语义——同一仓内两份文档对同一行为各执一词。

## C++ 对照

`src/storage/shard.cpp`：`OnTaskFinished`（:1382-1396）auto_reopen 分支按
`req->AutoReopenRetry() && reopen_retry_remaining_ > 0` 消耗**每读请求**预算；
driver 失败 `CompleteReopenWaiters`（:382-401）立即连坐；
`OnReceivedReq`（:593-606）外部 Reopen 类型请求带 reopen 标志直接 `InvalidArgs`。
`include/kv_options.h:422/429` 同名配置 `auto_reopen_retry_times = 10` /
`auto_reopen_pending_time_us = 10'000'000` 即每读请求预算——rev-37 甄别段
已逐行核实，落地后 Rust 与之同义。

## 目标

1. 重写 `05-persistence-and-recovery.md` reopen 段：单次 driver 尝试、每读请求
   `reopen_retry` 预算、外部 `Reopen` 无 driver 即回执 / 有 driver 挂 waiter
   （eng-39a 决策），并注明与 C++ 的对齐关系。
2. `fkv_config` 两字段补注释：语义 = 每读请求重试轮数 / 挂起间隔，
   与 C++ `kv_options.h:422/429` 同名同义（rev-37 方案 A 后）。
3. 全仓 grep `auto_reopen` 复查一遍文档引用，消除 06-gaps 与 05 的表述分叉。

## 验收

- 三处文本与 `fkv_shard/src/lib.rs` 现行为（`ReopenQueue` / `run_due_reopen` /
  `take_reopen_retry`）逐句一致；`05-persistence` 与 `06-gaps` 无矛盾表述。
- rev-37 的 DONE「残留」段可关闭（在该 md 复审小节登记）。

## 核验记录

- 亲读 `issue/done/rev-37-*.md` 全文（DONE 残留段：仅允许改 fkv_shard /
  fkv_runtime / fkv_task_reopen / fkv_standby，文档登记留给后续维护者）。
- 亲读 `doc/architecture/05-persistence-and-recovery.md:39-90`、
  `doc/architecture/06-gaps.md:81-85`、`crate/foundation/fkv_config/src/lib.rs:270-292`。
- `grep -rn "auto_reopen" doc/` 仅 06-gaps 一处为新语义，05 与其余文档 0 命中；
  `grep -rn "auto_reopen" fixtures/` 无 INI 语料引用，INI 面无单独文档页。
- 查重：`issue/new`（本轮前仅 00-index）、`issue/todo`（空）、
  `issue/reviewed/` 无承接条目；r7 各组报告未涉及 rev-37 文档残留。


## DONE（2026-11-05，第二轮复审修复）

DONE：05 恢复文档改为“driver 单次尝试 + 每读请求预算”；`auto_reopen_*` 两字段补语义注释。

## 复审（2026-11-05 v）

通过，归档。亲读：

- `doc/architecture/05-persistence-and-recovery.md:57-61`：已改为「`ReopenQueue`
  只做**单次**延迟尝试，失败即连坐全部 waiter（对齐 C++ `CompleteReopenWaiters`）；
  `auto_reopen_retry_times` 是**每读请求**预算……外部 `Reopen` 无同表 driver 时
  立即回 `ResourceMissing`，不 park」，与落地后行为一致。
- `crate/foundation/fkv_config/src/lib.rs:306-307`（每读请求轮数）与 :310-311
  （单轮延迟，对齐 C++ `auto_reopen_pending_time_us`）两字段注释已补。
- 分叉消除：`grep -rn auto_reopen doc/` 仅 05（:59）；`06-gaps.md:81-83` 与之
  同口径（rev-37/38/46 三行），无矛盾表述。
