# rev-48 文档滞后：eng-33a 跨分区写并发落地后，3 份文档 4 处仍写「写不并发」

来源：r7 滚动复审第 5 组复核 eng-33a 时发现的残余（eng-33a 的 DONE「剩余 /
风险」未自述文档同步需求，无既有条目承接）。

## 现状（2026-11-05 04:5x 亲读）

代码事实：eng-33a 已落地跨分区写真正并发（高优先 `Put` 组派发独立 compio
task，`max_write_concurrency` 为真派发额度；`fkv_shard/src/partition.rs` +
`lib.rs` `WriteFlight` / `spawn_put_group`；测试
`fkv_shard/tests/write_concurrency.rs` 4 条，见
`issue/reviewed/eng-33a-write-gate-concurrent-dispatch.md`）。

但以下 4 处文档仍停留在落地前，与代码相反：

1. `doc/architecture/06-gaps.md:10`（并发与调度表首行）：「读请求已 `spawn`
   并发…但写请求仍在 work loop 内逐条 `await`，跨分区写不重叠」/
   「`max_write_concurrency` 目前只有队列与指标语义，峰值写并发仍为 1」。
   该行应删除或移入文末「已收敛」记录。
2. `doc/architecture/02-execution-model.md:55-56`：「不同分区的写在**队列
   语义**上可排队，但 work loop 仍逐条 `await`，峰值 IO 并发仍为 1；跨分区
   真正并行是 eng-33a」。应改写为已落地的「写任务独立 spawn + `WriteGate`
   FIFO / 全局额度」模型（含读边界 `blocked_read_li`、控制请求先 drain 在飞写）。
3. `doc/migration.md:206`（M3 写路径）：「缺跨分区写并发与 merge 逐 entry
   clone 优化」——前者已落地（后者 eng-90 本轮亦已交付，改写时一并核对）。
4. `doc/migration.md:207`（M4 本地运行时）：「缺跨分区写并发、单 Store 多
   DS 托管」——跨分区写并发已落地，只剩单 Store 多 DS（sto-61b）。

失效链接：1 与 2 中的 `issue/todo/eng-33a-*` 链接已随本条归档指向
`issue/reviewed/`。

## 目标

- 4 处改写为落地后事实；06-gaps 的该行移入「已收敛」并指向
  `issue/reviewed/eng-33a-*`；顺带核对 `doc/metrics.md` §3 的
  `write_running` 口径描述（现为「在跑写任务数」，若仍写旧口径一并更新）。

## 验收

- `doc/` 内不再有「跨分区写不重叠」「峰值写并发仍为 1」「缺跨分区写并发」
  字样；06-gaps 并发表无与 eng-33a 相反的行；无指向 eng-33a 旧路径的链接。

## 去重

- `issue/todo/` + `issue/new/` 无其它条目承接（eng-33a 已归档；rt-02 范围仅
  Gc 映射文档；`00-index.md` 快照数字归看板维护方，不在本条范围）。
- 同文件内 eng-90 / rev-45 的 06-gaps 行若已随其归档更新则勿重复处理；
  本条只管「跨分区写并发」陈述。

严重度：P3（纯文档，代码与测试已一致）。

## 甄别小节（2026-11-05 05:02 复核，部分采纳）

代码事实核实通过：`crate/engine/fkv_shard/src/lib.rs:1940`
（`write_flight.push(spawn_put_group(...))`，高优先 Put 组派发独立 compio
task）、`WriteFlight`（`lib.rs:2161`）、`WriteGate` 按 `max_write_concurrency`
全局限额（`write_gate.rs`，0 = 不限）；`tests/write_concurrency.rs` 为 eng-33a
验收（`write_running` 采样观测跨分区并发 = 2、限额 1 时不超 1）。

立项时 4 处中 **2 处已被并行会话就地修复**（本条 04:58 立项后）：

- 06-gaps.md：04:59:11 更新——「写路径与内存」节已声明无未收敛缺口，
  eng-33a 已入「已收敛」（现 06-gaps.md:76-78）。原 :10 行已不存在。无需处理。
- 02-execution-model.md:53-56：05:00:34 改写为「写并发：不同分区按
  `max_write_concurrency` 真正并发 `spawn`，全局额度由 write gate 计数」，
  链接已指向 `issue/reviewed/eng-33a-*`。失效链接随之消除。无需处理。

失效链接：全 `doc/` 已 grep 无 `issue/todo/eng-33` 残留。metrics.md §3 的
`write_running` 已是「全局在跑写任务数」口径（doc/metrics.md:85），无需改。

残留（grep `doc/` 仅剩两行）：`doc/migration.md:206`（M3「缺跨分区写并发与
merge 逐 entry clone 优化」——eng-33a 与 eng-90 均已落地）与
`doc/migration.md:207`（M4「缺跨分区写并发、单 Store 多 DS 托管」）。

去重复核：`issue/todo/` 为空；rt-02（reviewed/）范围仅 Gc 映射文档；
`00-index.md` 无本条条目（归看板维护方）；无重复立项。

执行注意：06-gaps 与 02-execution-model 的修复与本条立项仅差 1-2 分钟，
疑为 eng-33a/eng-90 归档会话的收尾同步；**开工前须重查 `doc/migration.md`
是否也已被同一会话收敛，若已收敛则直接关闭本条。**

## 开发计划（纯文档，仅 `doc/migration.md`）

1. `doc/migration.md:206`（M3 行）：删「缺跨分区写并发与 merge 逐 entry
   clone 优化」，改为已落地陈述——跨分区写并发（eng-33a：`WriteFlight`
   并发派发 + `WriteGate` 全局额度）与 merge 单趟折叠零 clone（eng-90）
   均已交付；M3 当前无未收敛差异则照 06-gaps 口径写明。
2. `doc/migration.md:207`（M4 行）：删「缺跨分区写并发、」；「单 Store 多
   DS 托管」与 06-gaps「云与运行模式」行对齐（范围外有意偏离，sto-61b
   已归 done/），勿保留「缺」字口径。
3. 验收：`grep -rn '缺跨分区写并发\|跨分区写不重叠\|峰值写并发\|issue/todo/eng-33'
   doc/` 零命中；不动 `issue/new/00-index.md`。
4. 范围外仅观察不处理：同表 M2 行 eng-32c / rev-45 陈述亦滞后，归各自
   归档会话。

> 认领（2026-11-05 05:06，本会话子代理收尾中）。lane：doc/migration.md + 本 issue 文件。

## DONE（2026-11-05）

改写两行。开工前重查 `doc/migration.md:206/:207`：均未被并行会话收敛（仍为
「缺跨分区写并发」旧口径），未走直接关闭，按开发计划 1/2 执行：

- M3 行：删「缺跨分区写并发与 merge 逐 entry clone 优化」，改为已落地陈述
  （eng-33a：`WriteFlight` 并发派发 + `WriteGate` 全局额度；eng-90：merge
  单趟排序折叠零 clone），照 06-gaps 口径写明写路径无未收敛缺口。
- M4 行：删「缺跨分区写并发、」；「单 Store 多 DS 托管」按 06-gaps 口径改写
  为「范围外有意偏离（每进程一个 attach 身份）」并指向 sto-61b。
- M2 行未动（范围外仅观察不处理）。

引用终位核实：eng-33a 在 `issue/reviewed/`，eng-90 与 sto-61b 在
`issue/done/`（非 reviewed/），改写行按实际终位链接。

验收证据：`grep -rn '缺跨分区写并发\|跨分区写不重叠\|峰值写并发\|issue/todo/eng-33' doc/`
零命中（exit=1）。06-gaps 与 02-execution-model 已由并行会话收敛，未重复处理。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 亲读在盘文档：

- `doc/migration.md:206`（M3 行）已改为「跨分区写并发（eng-33a：`WriteFlight` 并发派发 +
  `WriteGate` 全局额度）与 merge 单趟排序折叠零 clone（eng-90）均已交付，写路径无未收敛
  缺口」，原「缺跨分区写并发与 merge 逐 entry clone 优化」已删。
- `doc/migration.md:207`（M4 行）已删「缺跨分区写并发、」，「单 Store 多 DS 托管」改为
  「范围外有意偏离（每进程一个 attach 身份，见 sto-61b）」。
- 引用终位正确（eng-33a 指 `issue/reviewed/`，eng-90/sto-61b 指 `issue/done/`）。
- 验收 grep 全 `doc/` 零命中（exit=1）；M2 行未动（范围外，含 eng-32c / rev-45 陈述已另行
  收敛）。纯文档改动。

> 注：同表 M3 行链接指向 `issue/done/eng-90-...`，本轮 eng-90 因在途未归档，链接暂有效；
> 若下轮 eng-90 改述或归档，需同步该链接终位。
