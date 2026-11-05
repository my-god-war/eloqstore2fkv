# r7 滚动复审 · 第 4 组报告（2026-11-05）

范围：`issue/done/` 两项。逐条按 DONE 声称核对源码与测试，证据行号均亲读
当前工作区（`fkv_shard/src/lib.rs` mtime 04:33、`fkv_metrics` 04:32，内容与
声称相符，非在途冲突）。cargo 因并行会话占锁未运行，通过声称以代码 +
测试文本核对为准。

## 结论一览

| 条目 | 判定 | 去向 |
|---|---|---|
| [`12a`](../reviewed/12a-per-table-request-gaps.md) per-table 请求补齐 + FileGc/LocalGc 拆分 | 通过 | `reviewed/` |
| [`rev-01`](../reviewed/rev-01-bench-kv-size-semantics-and-default-divergence.md) bench `--kv-size` | 通过（含裁定重开在案记录） | `reviewed/` |
| 新立项 [`rt-02`](../new/rt-02-gc-mapping-doc-sync.md) Gc 映射文档滞后 | — | `new/` |

## 12a：通过

- **请求枚举穷尽**：`fkv_api` 25 变体含全部 8 个新请求，合并 `Request::Gc` /
  `Op::Gc` 全仓零残留；`fkv_shard` 的 `dispatch`(:2664-3003) /
  `dispatch_read`(:2153) / `op_of`(:1145) / `is_read_op`(:1426) /
  `is_write_op`(:1444) / `pri_of`(:86) / `skips_table_open`(:1637) /
  `prepare_table`(:1648) 全部接线，无 `_` 兜底，穷尽成立。
- **FileGc/LocalGc 落地形态**：C++ 亲读对照一致（`write_task.cpp:967/:1035`
  `RunFileGc` Cloud→`ExecuteCloudGC`/Local→`ExecuteLocalGC`、`RunLocalFileGc`
  无条件本地、`eloq_store.h:792/801` 均为 WriteRequest）。Rust 侧
  `GcScope::{File,Local}`（`fkv_task_gc/src/lib.rs:708`）共享引用集只分叉
  列举/删除；`local_gc` 跳过 manifest 剪枝（`fkv_store/src/lib.rs:3911-3918`）；
  云端本地路径覆写无 `remote.delete`（`fkv_cloud/src/lib.rs:2905-2969`）。
  语义在代码 doc-comment 层文档化充分。
- **每条新请求有测试**：cloud/store `local_gc.rs`、shard `lifecycle.rs:270-280`、
  `modes.rs:69-70`、`metrics.rs:127`（`Op::ALL.len()==21`）、`branch_archive.rs`
  `:212/:326`、`per_table_ext.rs:56`、`prewarm.rs:285/:226`、`standby.rs:231`、
  `archive_lifecycle.rs`，关键断言均抽读属实。
- **eng-84 边界清晰**：eng-84 残余只剩全局 `GlobalReply::name` 空目标；
  per-table `ListArchiveTags` 由 eng-84「仍未完成」第 3 条明确留给 12a，已交付。
- **残余**：`doc/metrics.md:28,40-41` 与 `doc/architecture/06-gaps.md:37` 仍写
  「Gc 合并 / fkv 无 list_object 请求」，与代码相反（DONE 自述、非隐瞒），
  原无 open issue 承接 → 立新 issue `rt-02`。

## rev-01：通过（注意：落地推翻了 perf-74 语义裁定）

- **代码属实**：`fkv_bench/src/main.rs:86` `DEFAULT_KV_SIZE=128`、帮助文本
  :264-270 由常量生成、`resolve_kv_size`(:752) 缺省 128 + `<=8` 拒绝、
  `value_len`(:767) = pair − 8、`:809` 写入用差值、`--value-size` 零残留；
  C++ 亲读 `simple_bench.cpp:24/:113/~:549` 与对照一致；断代在
  `main.rs:198-200` + `doc/metrics.md:143-148` §6 双登记。
- **测试属实**：`resolve_kv_size_pair_semantics`(:1276-1289) 断言缺省/非法回退/
  拒绝边界/value 推导，与 DONE 清单一致。
- **与简报预期不符处在案说明**：简报按「语义按 perf-74 保留」验收，实际 DONE
  明示推翻 perf-74「语义保留为文档差异」裁定，全量对齐 C++（pair 宽度、默认
  128）。理由（`main.rs:1` 立项目标 = 同命令行跨实现可比；三方默认不一致属
  实现缺陷）成立，落地自洽（默认/校验/文案/断代四件齐备），判有效裁定重开；
  perf-74 相关段落自此为历史记录。已写入该文件「复审（2026-11-05 r7）」小节。

## 新立项

- `issue/new/rt-02-gc-mapping-doc-sync.md`（P3）：`doc/metrics.md` §1 映射表与
  `doc/architecture/06-gaps.md` 的 GC 粒度行未随 FileGc/LocalGc 拆分更新
  （去重核实：无既有条目承接）。

## 目录状态

`done/` 已清空（本轮两条 mv 入 `reviewed/`）；`new/` 增 rt-02。
`00-index.md` 快照更新归看板维护方，未代改。
