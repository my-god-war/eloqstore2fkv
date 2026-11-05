# rt-02 文档滞后：`Gc → file_gc + local_gc` 映射表未随 FileGc/LocalGc 拆分更新

来源：r7 滚动复审第 4 组复核 `12a` 时发现的残余（12a 的 DONE 已自述「需在文档
lane 同步」，但无 open issue 承接，本条补登记）。

## 现状（2026-11-05 04:4x 亲读）

代码已是拆分后的事实源：

- `fkv_metrics`：`Op::FileGc => "file_gc"` / `Op::LocalGc => "local_gc"` 一对一
  （`crate/foundation/fkv_metrics/src/lib.rs:109-110`），`Op::Gc` 已删除；
  `Op::ListObject` / `Op::ListStandbyPartition` 已映射 C++ 标签（`:123-124`）；
  `Op::ALL.len() == 21`（`fkv_metrics/tests/metrics.rs:127` 钉死）。
- `fkv_api`：`Request::Gc` 已删，只有 `Request::{FileGc, LocalGc}`
  （`crate/engine/fkv_api/src/lib.rs:65-76`）。

但两处文档仍停留在拆分前：

1. `doc/metrics.md:28`：映射表行 `` | `Gc` | `file_gc` + `local_gc` | fkv 合并；
   对账时 2 个标签求和 | `` ——已失实，应为 `FileGc → file_gc`、
   `LocalGc → local_gc` 两行一对一。
2. `doc/metrics.md:40-41`：`` | （无） | `list_object` | fkv 无对应请求 |`` 与
   `list_standby_partition` 同款两行——`Op::ListObject` / `Op::ListStandbyPartition`
   已存在并映射，行应删除或改为同义映射。
3. `doc/architecture/06-gaps.md:37`：「`FileGc` / `LocalGc` 仍合并为单条 `Gc`」
   ——与代码相反；该行引用的 `12a` 链接也指向 `issue/todo/`（12a 已复审归档）。

## 目标

- `doc/metrics.md` §1 映射表与 `Op::cpp_request_type_li()` 重新逐行对表；
- `doc/architecture/06-gaps.md` 删除或改写 GC 粒度 gap 行（拆分已落地）；
- 顺带核对表中 `Shutdown`（无）与新增 `Prewarm` / `ListArchiveTags`（无 C++ 标签）
  行是否需要补（`fkv_metrics/src/lib.rs:121-122`）。

## 验收

- 文档表格与 `op_enum!` 宏展开一一对应，无「合并求和」残留行；
- `doc/architecture/06-gaps.md` 不再声称 FileGc/LocalGc 未拆分。

严重度：P3（纯文档，代码与测试已一致）。

## 甄别（2026-11-05 立项复核，采纳）

逐条亲读核实，原意见全部属实，无重复承接：

- 代码事实源确认：`fkv_metrics/src/lib.rs:109-110` 为 `FileGc => "file_gc"` /
  `LocalGc => "local_gc"` 一对一，`Op::Gc` 已删；`:121-124` 新增
  `Prewarm` / `ListArchiveTags`（无 C++ 标签）与 `ListObject` / 
  `ListStandbyPartition`（一对一映射）。`fkv_api/src/lib.rs` 仅存
  `Request::{FileGc, LocalGc}`（全仓 grep 无 `Request::Gc`）；
  `fkv_shard/src/lib.rs:1154-1156` 分发亦为一对一。`GcScope` 只存在于
  引擎内层（`fkv_task_gc` 定义、`fkv_store/src/lib.rs:3771-3791`
  `gc_with_scope` 使用），不是请求面概念，文档无需提及。
- `doc/metrics.md:28`（`Gc | file_gc + local_gc | fkv 合并`）与 `:40-41`
  （`list_object` / `list_standby_partition` 两行「fkv 无对应请求」）确认失实。
- `doc/architecture/06-gaps.md:37`「仍合并为单条 `Gc`」确认失实；其链接指向
  `issue/todo/12a-per-table-request-gaps.md`，而 12a 实际已归档在
  `issue/reviewed/`（链接也失效）。
- 去重：`issue/todo/` + `issue/new/` 无其他 issue 承接本项。`eng-84` 只管
  global_* API 接线与 facade（改 `doc/migration.md`），不碰 metrics.md §1
  与 06-gaps 的 Gc 行；`rev-37` / `rev-46` 与本题无关。
- 甄别补充两处原意见未列出的同类失实（见开发计划第 6、7 条）。

## 开发计划（纯文档改写，共 7 处）

1. `doc/metrics.md:28`：删 `Gc` 合并行，改为两行 `| FileGc | file_gc | |`、
   `| LocalGc | local_gc | |`（对齐 `lib.rs:109-110`）。
2. `doc/metrics.md:40-41`：删 `（无）| list_object` / `（无）| list_standby_partition`
   两行（已一对一映射，`lib.rs:123-124`）。
3. `doc/metrics.md` §1 表补 4 行：`Prewarm`（无 C++ 标签）、
   `ListArchiveTags`（无 C++ 标签）、`ListObject → list_object`、
   `ListStandbyPartition → list_standby_partition`（`lib.rs:121-124`）。
4. `doc/metrics.md:51-52` 对账规则：合并求和仅剩 `Drop` 一例，措辞相应收窄；
   「无 C++ 标签单独列出」的对象由 `Shutdown` 扩为 `Shutdown` /
   `Prewarm` / `ListArchiveTags`。
5. `doc/metrics.md:72`：「每 shard：16 个 `Op`」→ 21（`Op::ALL.len() == 21`，
   `fkv_metrics/tests/metrics.rs:127` 钉死）。
6. `doc/architecture/06-gaps.md:37`：整行删除——GC 粒度差异已不存在，行内剩余
   信息（per-table 控制面已接线）不再是缺口；如需留痕，指向
   `issue/reviewed/12a-per-table-request-gaps.md`（原 `todo/` 链接已随归档失效）。
7. `doc/architecture/02-execution-model.md:27`：低优先 lane 清单里的 `Gc`
   改为 `FileGc / LocalGc`（甄别时发现的同源残留）。

验收即原意见验收标准 + 第 5、7 条：metrics.md 全表与 `op_enum!` 宏展开
一一对应、无「合并求和」残留（Drop 除外，其合并仍属实）、Op 计数 21；
06-gaps 不再声称 FileGc/LocalGc 未拆分；02-execution-model 无裸 `Gc`。

## DONE（2026-11-05）

按开发计划 7 条全部落地，另修一处同源残留（`doc/migration.md` M4）：

1. `doc/metrics.md` §1：`Gc | file_gc + local_gc` 合并行删除，改为
   `FileGc → file_gc` / `LocalGc → local_gc` 两行一对一。
2. `doc/metrics.md` §1：删除 `（无）| list_object` / `（无）| list_standby_partition`
   两行；按 `op_enum!` 顺序补齐 `Prewarm` / `ListArchiveTags`（无 C++ 标签）与
   `ListObject → list_object` / `ListStandbyPartition → list_standby_partition`。
3. `doc/metrics.md` §1 补充事实：全表 21 行与 `op_enum!` 一一对应；
   对账求和只剩 `Drop`（`drop` + `drop_table`），无标签的
   `Shutdown` / `Prewarm` / `ListArchiveTags` 单独列出不参与对比。
4. `doc/metrics.md` §3：「每 shard：16 个 `Op`」→ 21。
5. `doc/architecture/06-gaps.md`：「请求面与生产入口」表两行（GC 粒度 /
   `global_*` 空目标）删除，改为「均已收敛」并指向文末新增的 2 条已收敛记录
   （`12a` / `eng-84a` / `eng-84`），链接指向 `issue/reviewed/` 与
   `issue/done/`，不再使用失效的 `issue/todo/12a-*`。
6. `doc/architecture/02-execution-model.md` 低优先 lane：`Gc` →
   `FileGc / LocalGc`。
7. `doc/migration.md` M4：删除「per-table 控制面已接，余 FileGc/LocalGc 粒度」，
   改为「per-table 控制面与 `FileGc` / `LocalGc` 请求已接线，余跨
   `(branch, term)` 的 GC 粒度」（与 `rev-39` 一致）。

验证：

- 机械比对 `op_enum!` 变体序列与 `doc/metrics.md` 表格首列：21 行全等
  （脚本 diff 为空）。
- 全仓 `grep 'Request::Gc|Op::Gc\b'` 无命中；`doc/` 无「16 个 `Op`」/
  「余 FileGc」/「合并为单条」残留。
- 纯文档改动，无代码/测试变更；未跑 workspace 全量测试（按并发纪律只跑
  targeted，eng-84 侧 `fkv_runtime` 用例 11 passed）。

## 复审（2026-11-05 r7）

**判定：通过。** 7 处逐条亲读复核，另做了两项独立验证（行号为复审时点读数）：

1. `doc/metrics.md:28-29`：`FileGc → file_gc` / `LocalGc → local_gc` 两行
   一对一，旧 `Gc | file_gc + local_gc` 合并行已删。✓
2. `:40-44`：`（无）| list_object` / `（无）| list_standby_partition`「fkv
   无对应请求」行已删；`Prewarm`（无标签）/ `ListArchiveTags`（无标签）/
   `ListObject → list_object` / `ListStandbyPartition → list_standby_partition`
   四行按 `op_enum!` 顺序补齐。✓
3. `:54-59`：对账规则收窄为「只有 `Drop` 合并求和」；无标签的
   `Shutdown` / `Prewarm` / `ListArchiveTags` 单独列出不参与对比；
   「上表与 `op_enum!` 一一对应，共 21 个 `Op`」。✓
4. `:79`：「每 shard：21 个 `Op`」，旧「16 个」无残留。✓
5. `doc/architecture/06-gaps.md:35-36`：请求面 / 入口缺口表改为「均已收敛，
   见文末」；`:46+` 新增已收敛记录（per-table 控制面与 FileGc/LocalGc、
   `global_*` 控制面与空目标回执），链接指向 `issue/reviewed/` /
   `issue/done/`，旧 `issue/todo/12a-*` 失效链接清除。✓
6. `doc/architecture/02-execution-model.md:27`：低优先 lane 清单已为
   `FileGc / LocalGc / …`，无裸 `Gc`。✓
7. `doc/migration.md:207`（M4）：「per-table 控制面与 `FileGc` / `LocalGc`
   请求已接线，余跨 `(branch, term)` 的 GC 粒度」，与 rev-39 口径一致。✓

独立验证：

- **表 ↔ 代码全等**：`op_enum!` 变体序列（`fkv_metrics/src/lib.rs:112-133`，
  21 个）与 `doc/metrics.md` §1 表格首列（`:24-44`，21 行）逐行人工比对，
  名称与顺序全等；`Request::Gc` / `Op::Gc` 在 `crate/` 全仓 grep 零命中。
- **残留 grep**：`doc/` 内「16 个 `Op`」「余 FileGc」「合并为单条」
  「file_gc` + `local_gc」全部零命中。

纯文档改动，无需跑测试。**邻带发现（不属本条范围）**：eng-33a 落地后
`06-gaps.md` / `02-execution-model.md` / `migration.md` 另有 4 处「跨分区写
不并发」滞后陈述，见 `issue/new/rev-48`。

