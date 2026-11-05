# 33 P1：孤儿 crate —— 实现完整但无生产调用者

## 现象

以下 crate 有实现和单测，但**没有任何生产路径依赖**：

| crate | 状态 |
|-------|------|
| `fkv_cache` | slot / LRU / pin / swizzle 完整，孤儿（另见 `13-cache-rootmeta.md`） |
| `fkv_root` | `RootMeta` + `RootMetaMgr` 完整，孤儿 |
| `fkv_cloud` | 完整，未接入 shard/runtime（见 `10`、`12`） |
| `fkv_standby` | 完整，未接入（见 `12`） |
| `fkv_task_prewarm` | 完整，孤儿 |

另外 `fkv_gc::should_compact` 从不被调用（`crate/engine/fkv_gc/src/lib.rs:35`）。

## C++ 对照

C++ 里这些都有明确消费者：`PageManager` 进读路径，`RootMetaMgr` 管每表 RootMeta，`CloudStoreMgr` / `StandbyStoreMgr` 由 `StoreMode` 装配，`PrewarmTask` 在 cloud 打开时触发。

## 影响

- 自测通过但生产路径完全没走，等于未验证。
- `fkv_root` 的 LRU 每次 touch 都 `VecDeque::retain` + 重算 `used_bytes` O(n)（`fkv_root/src/lib.rs:192-196、225-230`），一旦接线就是热点。
- 违反 `rust_review`「零死代码」纪律。

## 建议

1. 逐个接线：`fkv_cache`→读路径（`02`）、`fkv_root`→字典 + root 缓存、`fkv_cloud`/`fkv_standby`/`prewarm`→runtime（`12`）。
2. 接线前把 O(n) 实现改成 O(1)（LRU 双向链 + 增量字节计数）。
3. 暂时接不上的在 ledger 记 blocked，别停在「完整但没人用」。

## 验收

- 生产依赖图里无孤儿 crate（或已显式 deferred）。

## 审查结论（2026-11-04）

### 总结论

**部分采纳**。核心事实成立：5 个 crate 在生产依赖图中确实无消费者。但存在 2 处硬错误与 3 处归口重复，必须改写后再执行：

1. `fkv_gc::should_compact` **不是死代码**：`crate/engine/fkv_gc/src/lib.rs:68` 在 `pick_compact_file` 内调用，`pick_compact_file` 由 `crate/engine/fkv_store/src/lib.rs:2162` 生产调用。原「从不被调用」判断错误。
   真正只有测试消费的是 `file_amplification` 与 `unreferenced_file_page_li`（全仓仅 `fkv_gc/tests/gc.rs` 命中）。
2. `fkv_root` 的 O(n) 描述不准确：`touch` 有「已是最新直接返回」快路径（`lib.rs:222-224`）；`used_bytes` 只在 `evict_if_needed` 入口重算一次，插入/删除走增量。仍需修的是非尾部命中时的 `VecDeque::retain` + `remove` 的 O(n)。
   `fkv_cache` 早已是 `prev/next` 双向链（`fkv_cache/src/lib.rs:20-24`），「接线前改双向链」只适用 `fkv_root`。
3. 引用 `13-cache-rootmeta.md`、`10` 不存在实体文件，实际归口是 `core-13`、`core-14`（已移入 `issue/todo/`）、`12`（`issue/new/`）。
   附带观察：`fkv_standby/src/lib.rs` 正在被并发改动（io-10 的 blocking async 修复），但只是内部实现修正，依赖图仍未接线。

### 逐条核验

| 项 | 原文声明 | 核验证据 | 判定 |
|---|---|---|---|
| `fkv_cache` | 无生产依赖 | 符号 `PageCache` 仅命中 `fkv_cache/src` 与自身 tests；全仓 `Cargo.toml` 无任何 crate 依赖它（`fkv_cache_cpp` 仅同名对照） | 属实 |
| `fkv_root` | 无生产依赖 | `RootMetaMgr` 仅命中自身 src/tests；`fkv_store/src/lib.rs:671` 只是注释提到 `RootMeta`，无 `use fkv_root` | 属实 |
| `fkv_cloud` | 未接 shard/runtime | 唯一真实依赖者 `fkv_task_prewarm`（自身孤儿）；`fkv_task_reopen` 仅在 `[dev-dependencies]`，其 `src/` 零 `CloudStore`；`fkv_shard/src/lib.rs:124` 硬编码 `CompioStore::new` | 属实，本轮工作树无源码改动 |
| `fkv_standby` | 完整，未接入 | 任何 `Cargo.toml` 均不依赖它；导出 `StandbyService`/`StandbyConfig`，仅自身 tests 消费；`config` 已解析 `enable_local_standby`/`standby_*`，但 shard 不读 | 属实 |
| `fkv_task_prewarm` | 孤儿 | 无任何 crate 依赖；`prewarm_table`/`prewarm_tables` 仅自身 src 互调 + tests；`fkv_cloud::prewarm_*` 亦无人调 | 属实 |
| `fkv_gc::should_compact` | 从不被调用 | `lib.rs:68` 内部调用 → `pick_compact_file` → `fkv_store:2162` | **不成立** |
| `fkv_gc::file_amplification` | 未提及 | 仅 `fkv_gc/tests/gc.rs:19-20` | 新增发现：真死码 |
| `fkv_gc::unreferenced_file_page_li` | 未提及 | 仅 `fkv_gc/tests/gc.rs` 导入 | 新增发现：真死码 |

### 归口去重

- `fkv_cache` 全部内容已被 `core-13-page-cache-and-swizzle-tag-unwired.md` 覆盖（含接线/删除二选一决策），本 issue 不得重复立项。
- `fkv_root` 的接线与 `first_unflushed` 语义已被 `core-14-first-unflushed-fp-id-never-updated-root-meta-unwired.md` 覆盖；O(1) LRU 改写作为 core-14 的附带项。
- `fkv_cloud` / `fkv_standby` / `fkv_task_prewarm` 已被 `12-runtime-modes-api.md` 覆盖（shard IO backend 枚举 + `Request::Prewarm`）；`fkv_standby` 的缺陷另有 `issue/todo/io-10-...`。
- 因此本 issue 收窄为**依赖图守卫 + 死码清理**，不再承担接线实现。

### 开发计划（采纳后）

1. 删除死码：`fkv_gc::file_amplification`、`fkv_gc::unreferenced_file_page_li`，同步删 `crate/engine/fkv_gc/tests/gc.rs` 中对应用例；保留 `should_compact`（生产在用）。
2. 加依赖图守卫：新增脚本（放 `tool/` 或 `sh/`），用 `cargo metadata` 构建正向图，判定「非 dev、非自身、非 `_cpp` 对照」的依赖边；对 `fkv_cache`/`fkv_root`/`fkv_cloud`/`fkv_standby`/`fkv_task_prewarm` 设显式白名单并标注归口 issue，白名单外的新孤儿直接失败。
3. ledger 记账：把 5 个 crate 在 `tool/fkv_ledger` 的 unit 状态标 `blocked`，`owner` 指向 core-13 / core-14 / 12，避免「完整但没人用」静默存在。
4. 接线实现分别跟进 core-13、core-14、12；本 issue 验收改为「守卫脚本通过 + 白名单每项都有归口 issue」。

### 采纳边界

若评审认为「孤儿 crate 已全部有 owner issue」，可退化为拒绝并把守卫脚本并入 `12` 的验收项；但死码 `file_amplification`/`unreferenced_file_page_li` 需另找归属，否则继续堆积。

## 二次核验（2026-11-04）

前一次结论的方向对，但有 3 条事实已过期或写错，本轮按实际代码重判：

| 项 | 上次结论 | 本轮核验 |
|---|---|---|
| `fkv_cache` | 「完整实现，孤儿」 | **crate 不存在**。`crate/core/`、`crate/io/` 无此目录，`git log -- crate/core/fkv_cache` 为空，`cargo metadata` 成员表也没有它。残留的是 `doc/architecture/01-overview.md:16`、`doc/architecture/06-gaps.md:47`、`doc/progress.md:30`、`todo.md:174/176/199/200` 的文档引用。 |
| `core-13` / `core-14` | 「已移入 issue/todo/」 | **不存在**。`find issue -name '*13*' -o -name '*14*'` 无匹配，之前据此做的归口去重不成立。`fkv_root` 实际无归口。 |
| `fkv_cloud` | 未接 shard/runtime | 属实。唯一生产消费者是 `fkv_task_prewarm`（`Cargo.toml:11` 普通依赖），而 prewarm 自身零消费者；`fkv_task_reopen` 的 `fkv_cloud` 在 `[dev-dependencies]`。 |
| `fkv_standby` / `fkv_task_prewarm` / `fkv_root` | 孤儿 | 属实，`cargo metadata` 非 dev 边为空。 |
| `fkv_gc::should_compact` | 上次已纠正为生产在用 | 保留，`pick_compact_file` → `fkv_store` 调用链仍在。 |
| `fkv_gc::file_amplification` / `unreferenced_file_page_li` | 真死码 | 属实，本轮已删。 |

另：`fkv_api` / `fkv_metrics` / `fkv_object` / `fkv_shard` / `fkv_runtime` 在生产依赖图里也是不可达的
（CLI 尚未经 `fkv_runtime` 启动 shard），属「运行时栈整体未接线」，全部归 `12`，不在本条重复立项。

## 本轮落地

1. 删除死码：`fkv_gc::file_amplification`、`fkv_gc::unreferenced_file_page_li`；
   `fkv_gc/tests/gc.rs` 改为只测 `retained_file_page_set` 与 `should_compact`（新增 `factor == 0` 分支）。
2. 新增守卫：`tool/orphan.sh` + `tool/orphan_allow.tsv`。
   - 只算 `kind == null` 的生产依赖边，排除 `*_cpp` 对照；顶层入口 `fkv`/`fkv_sdk`/`fkv_testkit`/`fkv_bench`/`fkv_cli`/`fkv_ledger`。
   - 反向叶剪枝：**直连孤儿**（零生产消费者）失败；**传递孤儿**只提示，避免未接线运行时栈互相带出噪音。
   - 白名单 owner 必须能在 `issue/` 下找到文件前缀；已接线的白名单项报「过期」。
   - 正负样例实测：去掉 `fkv_root` 行 → 退出 2；给 `fkv_store` 加行 → 报过期退出 2。
3. `.github/workflows/ci.yml` 的 `test` job 在 clippy 后新增 `./tool/orphan.sh`。

## 归口与拆分

- `issue/todo/33a-fkv-root-wiring.md`：`fkv_root` 接线或删除二选一，含 `touch`/`remove` 的 O(n) `VecDeque::retain` 改写要求。
- `issue/todo/33b-orphan-guard-ci.md`：守卫接入 CI（本轮已加，验收仍需故意造孤儿验证）+ 白名单随归口 issue 收缩。
- `fkv_cloud` / `fkv_standby` / `fkv_task_prewarm` / `fkv_runtime` / `fkv_object_s3` → `12-runtime-modes-api.md`，白名单已标注。
- 取消上次计划里的「ledger 标 blocked」：`tool/fkv_ledger` 的库固定在 `.data/motedb.mote` 且 gitignore，无法随仓库传播。
- `fkv_io_mem` / `fkv_io_sim` 是测试后端，白名单标 `test-only`。
- 未处理：`fkv_cache` 的文档残留（`doc/architecture/*`、`doc/progress.md`、`todo.md`）不属于本 issue 的 crate 依赖图范围，留给文档清理。

本条验收改为：`./tool/orphan.sh` 退出 0，且白名单每项都有可解析 owner。

## DONE（2026-11-04）

决策：**不删、不接线（不在本 lane 范围），全部显式 deferred + 守卫通过**。

### 本轮核验

- 生产依赖图孤儿（`./tool/orphan.sh`，只算非 dev / 非 `_cpp` 边）：与白名单一致，7 项。
- 守卫 CI 接线在位：`.github/workflows/ci.yml:29` 跑 `./tool/orphan.sh`。
- 正负样例实测：追加 `fkv_store`（已接线）+ 不存在 owner 的假白名单行 → 守卫报「白名单已过期」
  与「owner 无对应文件」并退出 2；还原后退出 0。孤儿检测与过期检测均生效。
- 死码确认已清：`fkv_gc::file_amplification`、`fkv_gc::unreferenced_file_page_li` 全仓无定义/引用；
  `should_compact` 保留且被 `pick_compact_file`（`fkv_gc/src/lib.rs:41`）生产调用。

### 本轮修复（唯一失败项）—— 后被事实推翻并纠正

`fkv_standby` 的白名单 owner `12b-standby-runtime-wiring` 已归档，触发守卫「owner 已归档」并退出 2。
我一度新增 `issue/todo/12d-standby-runtime-wiring.md` 承接该归口并改 owner——但该条前提有误：
`fkv_standby` **其实已被 12b 接线**（`fkv_shard`/`fkv_runtime` 有生产依赖、`ShardConfig.standby`
注入 `Request::Reopen`、`fkv_runtime::backend_of` 接受 standby、`fkv_runtime/tests/standby.rs`
端到端通过）。

最终处置（dragon 复审，见 `issue/done/12d-standby-runtime-wiring.md` 的 `## 复审`）：

- `tool/orphan_allow.tsv` **删除** `fkv_standby` 行（crate 已有生产消费者）。
- `12d` 作为 stale 归口归档到 `issue/done/`（附复审证据）。

### 验收

`./tool/orphan.sh` 退出 0（白名单 **6** 项，owner 全部可解析且非归档）：
`fkv_cloud→12c`、`fkv_task_prewarm→12a`、`fkv_runtime→12c`、`fkv_object_s3→12c`、
`fkv_io_mem/fkv_io_sim→test-only`。`fkv_standby` 已接线，只出现在「传递孤儿」提示里
（其上游 `fkv_shard`/`fkv_runtime` 尚未被 facade 接线，归 `12c`）。

### 未处理（超出本 lane / 本 issue 范围，留档）

- `fkv_cache` 的文档残留（`doc/architecture/*`、`doc/progress.md`、`todo.md`）仍是死引用，
  留待文档清理，不在 crate 依赖图范围内。
- `fkv_api` / `fkv_metrics` / `fkv_object` / `fkv_shard` 为传递孤儿（消费者自身也孤儿），
  随 `12c` 的运行时整体接线一并消失，守卫只提示不失败。
- 已归档 `issue/done/12b-*` 与 `issue/todo/sto-62-*.md` 内对 `issue/todo/12b-*` 的引用已过期，
  归口改指 12b 的 DONE；本条不改其它 lane 的 issue 正文。

## 复审（2026-11-05 r6b）

通过。

- `tool/orphan.sh`（可执行）与 `tool/orphan_allow.tsv` 在盘；白名单 6 项与 DONE 声称一致
  （fkv_cloud→12c、fkv_task_prewarm→12a、fkv_runtime→12c、fkv_object_s3→12c、
  fkv_io_mem/fkv_io_sim→test-only），无 `fkv_standby`/`fkv_root` 行。
- CI 接线：`.github/workflows/ci.yml:29` `- run: ./tool/orphan.sh`。
- 死码已清：`crate/engine/fkv_gc/src/lib.rs` 仅 `:7 should_compact`（被 `:55 pick_compact_file`
  生产调用）；`file_amplification` / `unreferenced_file_page_li` 全仓无定义。
- 拆分件 `33a`（fkv_root 删除）同批复核通过；`33b`（orphan-guard CI）已在 `issue/reviewed/`。
