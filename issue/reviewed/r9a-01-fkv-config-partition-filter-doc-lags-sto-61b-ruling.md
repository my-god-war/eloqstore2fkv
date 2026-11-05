# r9a-01 P3：fkv_config 模块文档 partition_filter 仍写「随 sto-61 落地」，与 sto-61b「范围外不做 1:1」最终裁定相悖

## 现象（亲读）

`crate/foundation/fkv_config/src/lib.rs:11-13`（「未迁移字段（决策登记）」清单第 3 条）：

```rust
//! - `partition_filter`：属「一个 Store 托管多个 data store shard」，随 sto-61
//!   的 `DsShardId` 路由一并落地；当前 runtime 无 per-request shard 概念，
//!   先引入回调字段只会成为死代码。
```

「随 sto-61 …一并落地」是 sto-57c 登记时的前瞻叙事（`issue/reviewed/sto-57c-non-migrated-options-registry.md:11`）。
sto-61 家族实际收口与此不同：

- `issue/done/sto-61b-multi-ds-shard-hosting-and-request-routing.md` DONE：「多 DS 托管按**宿主集成范围外**关闭，登记基线」。
- `COMPAT_BASELINE`（工作区版）：「单进程托管多个 data store shard（per-request `ds_shard` / `partition_filter` / detach fanout，sto-61b）**不做 1:1**……请求路由仍按 `partition_id % shard_num`，与 C++ 未设 filter 时的默认路由一致……判定与重开条件见 issue/done/sto-61b-…」。
- `doc/architecture/06-gaps.md:24`：同一裁定，标记「**范围外（有意偏离）**」。

即 `partition_filter` 的最终裁定是「不做 1:1、范围外」，而 `fkv_config` 模块文档（被
`validate` 的告警文案 :494/:524 与 COMPAT_BASELINE 明文指为「决策登记」载体）仍停留在
「将随 sto-61 落地」的旧计划口径——读到该注释的人会误以为此字段计划落地。

连带（scope 外，随本条提示，修不修由 runtime 归口裁定）：`crate/engine/fkv_runtime/src/lib.rs:2312`
`list_standby_partition` doc「多 DS 的分区过滤归 `sto-61b`」——语义是「移交待办」，
实际 sto-61b 已按范围外关闭，同样滞后。

## C++ 对照

不涉盘面。C++ 侧 `partition_filter` 是 `KvOptions` 的宿主回调字段（`include/kv_options.h`，
`std::function`），非 INI 键；fkv 不迁移该字段本身无 INI 兼容问题（sto-57/rev-40 已裁定），
本条纯为 Rust 侧决策登记文档滞后。

## 目标

1. `fkv_config` 模块文档该条改为最终口径：「不做 1:1，宿主集成范围外（sto-61b 裁定）；
   请求路由按 `partition_id % shard_num`，与 C++ 未设 filter 的默认一致；判定与重开条件见
   `COMPAT_BASELINE` / `issue/done/sto-61b-…`」。
2. 提示 `fkv_runtime/src/lib.rs:2312`「归 `sto-61b`」改为「sto-61b 已按范围外关闭」口径
   （本条验收不强制，需 runtime 归口确认）。

## 验收

- `grep -rn "随 sto-61" crate/foundation/fkv_config` 零命中；模块文档与
  `COMPAT_BASELINE` / `doc/architecture/06-gaps.md:24` 三处口径一致。
- runtime 归口确认 `:2312` 注释口径（或明确不改的理由）。
- `./sh/clippy.sh`、`./test.sh` 通过（纯注释改动）。

## 核验记录

- 亲读 `fkv_config/src/lib.rs:1-40`（模块文档）与 `:488-528`（validate 告警文案引用「决策登记见模块文档」）。
- 亲读 `COMPAT_BASELINE` 全文、`doc/architecture/06-gaps.md:24`、`issue/done/sto-61b-…` 的 DONE 段。
- 查重：r6 catchup / r7 group1-5 / r8 全部 done 文件无本条；`sto-57c` 是登记来源而非本条的重复
  （本条主张的是 sto-61b 关闭后登记未回写）；`rev-48`（eng-33a 文档滞后）、`r8d-01`（write_gate 文档滞后）
  同类不同点。
- 锚点 mtime：`fkv_config/src/lib.rs` 05:57、`fkv_runtime/src/lib.rs` 06:17（2026-11-05），均在 2 分钟
  并行写入区之外；`fkv_runtime` 属在途改动活跃区，行号以内容锚点为准。


## DONE（2026-11-05，第三轮复审修复）

DONE：`fkv_config` 与 runtime doc 改为 sto-61b 最终裁定（范围外，不做 1:1）。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 亲读在盘文档：

- `crate/foundation/fkv_config/src/lib.rs:11-13`：「未迁移字段（决策登记）」中
  `partition_filter` 已改为「属……宿主集成，**不做 1:1**（sto-61b 裁定范围外，见
  `COMPAT_BASELINE`）；请求路由按 `partition_id % shard_num`，与 C++ 未设 filter 时的
  默认一致」——与立项旧口径「随 sto-61 落地」相反，目标 1 达成。
- 验收 grep `随 sto-61` 于 `crate/foundation/fkv_config` 零命中（exit=1）。
- 连带项 `crate/engine/fkv_runtime/src/lib.rs:2468`（`list_standby_partition` doc）已改为
  「多 DS 托管与 `partition_filter` 已按范围外关闭（sto-61b）」，目标 2 达成。
- 三处口径一致：`doc/architecture/06-gaps.md:24`「不做 1:1 | 范围外（有意偏离）」、
  `COMPAT_BASELINE:20-27`「不做 1:1……请求路由仍按 `partition_id % shard_num`」、
  本条模块文档。纯注释改动。
