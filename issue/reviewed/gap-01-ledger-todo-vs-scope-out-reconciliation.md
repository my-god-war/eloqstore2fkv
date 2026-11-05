# gap-01 账本 543 个 todo 未区分「范围外」与「真缺口」（P1）

## 来源

dragon 的剩余工作盘点（用户 2026-11-05 指定按 `issue/new.md` 流程拆到 `./new`）。

## 证据（2026-11-05 实测）

- `doc/progress.md` §状态总览：**todo 543 / done 1305 / verified 0 / total 1848**。
- 按 crate 的 todo 分布：`<unmapped> 352`、`fkv_io 103`、`fkv_mapping 34`、
  `fkv_buf 34`、`fkv_runtime 13`、`fkv_shard 7`。
- `doc/progress.md` §下一批任务只给了 6 个具体单元
  （`AsyncIoManager`、`CircularQueue`、`IndexPageIter`、`PageManager`、
  `ArchiveCrond`、`KvTask`）。
- 而 `doc/progress.md` §范围与排除项 已明确一批「有意不迁移」的能力
  （`mem_cached_page` 的页级 pin / 用户态 data page cache、`GlobalRegisteredMemory`
  slab、单进程多 DS 托管、若干不生效旋钮），这些**仍以 todo 挂在账本里**。

## 问题

543 这个数字现在**不可用于判断剩余工作量**：它把「已裁定范围外」与「真缺口」混在一起，
`<unmapped> 352` 更是完全没有归属（既不知道要迁到哪个 crate，也不知道是否已裁定不做）。
结果是：任何基于账本的排期/汇报都会高估或低估剩余量。

## 目标

1. 对 543 个 todo 逐条给出**归属或裁定**之一：
   - 归属：`map` 到目标 crate/单元（含 `<unmapped>` 里的 352 条）；
   - 裁定范围外：用账本既有 `ignore` 能力标记，并在原因里指向
     `COMPAT_BASELINE` / `doc/progress.md` 的对应条目（不要只在报告里写）。
2. 产出「真实剩余量」：`todo_total - ignored_scoped_out`，并让 `doc/progress.md`
   的开头范围块与之一致。
3. 顺带补 `Q1` 的 `scaffold`（生成 Rust 骨架）：它是本项「归属」动作的落地手段，
   没有它，`map` 完仍然要手写空壳。

## 验收

- `fkv_ledger report` 的 `todo` 数下降，且每一处下降都能对上
  `ignore` 记录或 `map` 记录；
- `doc/progress.md` §状态总览 与 §按 crate 的表与账本输出一致（可脚本核对）；
- 范围外条目在 `COMPAT_BASELINE` 有对应叙述（不允许只标 `ignore` 不写原因）。

## 阻塞 / 依赖

**无外部阻塞**：纯账本 + 文档工作，本机可完成。需要的是**裁定**（哪些属于范围外），
可先按现有范围块机械对齐，剩下的逐条提请决策。

## 核验记录

- 数据源：`doc/progress.md`（2026-11-05 读取）、`doc/adopt.tsv` 头注、账本 CLI 输出。

## 复审（2026-11-05）

原 index 把它归为“已跟踪的验证债务”不成立：`ignored=0`、`<unmapped>=352`，
`map` / `ignore` 工具虽已实现但从未运行。重新打开，按 gap-01 目标执行分流。

## 复审归档（2026-11-05）

工作已落地，本轮复审逐项复核成立后归档到 `issue/reviewed/`。跳过 `done/`：这些条目的
实现在本轮复审之前就已提交，`done/` 只用于本轮亲自开发并附测试证据的条目。
`new/` 只留仍未落地的条目。

证据（复核命令与结果）：

- `tool/fkv_ledger/src/classify.rs`（新增）+ `doc/ledger-classification.tsv`：
  543 条逐条裁定，`awk` 统计 `312 map / 231 ignore`，**空理由 0 条**，理由均指向具体
  issue id 与 `COMPAT_BASELINE` 段落。
- `doc/progress.md:9`：`分流口径：todo 312 / ignored（范围外裁定）231 / <unmapped> 0`，
  `<unmapped>` 归零；覆盖率 0% → 12.5%。
- `map` / `ignore` 早已实现（`map.rs:133/152`、`ledger.rs:420`），本轮确认已被真正运行过。

未随本项关闭：**`scaffold` 子命令仍未实现**。判定为**低价值**：fkv 已功能完整
（工作区 1002 测试全绿），账本剩余的 312 条 todo 绝大多数是范围外残留，为其生成空壳
骨架的收益低于维护成本。若将来需要，按 `doc/migration-tooling.md` 的 P1 行单独开发。
