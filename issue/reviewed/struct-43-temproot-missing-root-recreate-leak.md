# struct-43 TempRoot 逃逸口 + 运行时重建「缺失根」→ 每轮全量测试泄漏临时目录

- 领域：测试基建（结构 / 测试质量域；struct-08/14 TempRoot 收敛的遗留面——
  两轮只治了「手写样板重复」，未治 `into_path` 逃逸口与运行时重建的组合）
- 优先级：P3（无生产行为影响；每轮全量测试在宿主 `$TMPDIR` 净积累 2 个
  垃圾目录，长期累积，本轮实测已积 107 个）
- 置信度：高（代码链逐行可证 + 本轮在 fkv `c5e8fbc` 两遍全量实测复现，
  第二遍前后 105 → 107，恰为 2 个用例各泄 1 个）

## 现象（fkv `c5e8fbc`）

`crate/engine/fkv_runtime/tests/store_path.rs` 两个「缺失根」用例的构造手法：

- `:205` `global_archive_missing_root_is_empty_success`
  - `:207` `TempRoot::new("runtime_store_path_missing").into_path()`
  - `:208` `fs::remove_dir_all(&missing).unwrap()`
- `:233` `global_archive_missing_secondary_root_scans_readable_root`
  - `:236` / `:237` 同上

意图是「先占位拿唯一路径、再删掉，得到一个不存在的 store 根」。但
`into_path`（`fkv_testkit/src/lib.rs:85-89`）用 `mem::forget` 放弃了
TempRoot 的 Drop 清理责任，此路径从此无人认领。

随后用例内 `attach_shard(0, "main", 1)` 走 term 递增：
`crate/io/fkv_io_compio/src/term.rs:56-57` 的 `upsert` 首行
`create_dir_all(root)` ——「缺失」根被运行时**重建**并写入
`CURRENT_TERM_main_0`（实测泄漏目录内恰为该 1 字节文件）。用例结束，
路径无人清理 → 泄漏。

## 复现证据（本轮第 31 轮实测）

```text
$ ls ${TMPDIR} | grep -c '^fkv_runtime_store_path_missing'   # 全量两遍前（run1 后）：105
# cargo nextest run --workspace --exclude fkv_nvme_test --all-features  # 第二遍
$ ls ${TMPDIR} | grep -c '^fkv_runtime_store_path_missing'   # 之后：107
```

另：标签计数 `runtime_store_path_missing 107 / shutdown_race 3 /
read_concurrency 2`。其中 shutdown_race / read_concurrency 的 5 个
stray 时间戳均早于本会话两遍 `c5e8fbc` 全量（未复现，疑为并行会话
WIP 或历史超时残留），不计入本条；fkv_bench 的 stray 观察到延迟自清，
亦不计入。

## 影响

- 测试语义不受影响（两用例照常通过；「缺失根当空根」的断言对象是
  global_archive 扫描，重建发生在 attach 阶段）。
- 宿主机 `$TMPDIR` 无界积累：每遍全量 +2 目录 × 每目录 1 个 1 字节
  `CURRENT_TERM_main_0`。CI 一次性 runner 无感，开发机长期累积
  （本机已 107 个，时间跨度自当日 04:22 起）。
- 与 struct-08（43 文件手写样板收敛）/ struct-14（7 内联残留收尾）
  的治理口径构成盲区：两轮普查键是「手写 temp_root 样板」，未覆盖
  「合法 TempRoot API 的逃逸口 + 被测对象重建」组合。

## 修复建议

不必「先建再删」：缺失路径用**从未创建**的 TempRoot 子路径即可——

```rust
let runtime_root = TempRoot::new("runtime_store_path_cfg");
let missing = runtime_root.join("missing");   // 不存在，且 Drop 递归清理兜底
```

`into_path` + 显式 `remove_dir_all` 手法整体删除；运行时若在其下重建
`missing/CURRENT_TERM_main_0`，也随 `runtime_root` 的 Drop 一并回收。
`into_path` 保留给「删除责任显式移交」的真实场景（当前全仓其余调用点
仅 testkit 自测一处，语义正确）。

## 验收方式

1. 上述两用例改写后通过，`grep -rn 'runtime_store_path_missing'`
   不再出现 `into_path`；
2. 全量两遍前后 `$TMPDIR` 内 `fkv_runtime_store_path_missing_*`
   计数不变；
3. `sh/unused.js`、门禁全表不受影响。

## 实现记录（2026-11-07）

- 修复 fkv commit：`0b87909`。`fkv_runtime/tests/store_path.rs` 两处
  「缺失根」改用活 TempRoot 下从未创建的子路径
  （`runtime_root.path().join("missing")`；第二处不挂在 root_a 下，避免
  被根扫描当作分区目录），attach 经 term upsert 的 `create_dir_all` 重建
  后随 runtime_root 的 Drop 清理；`into_path()` + `remove_dir_all` 手法
  删除，`fs` 导入仍被权限用例使用故保留。
- 验证：`store_path` 8/8 passed；泄漏计数两轮前后 107 → 107（修复前每轮
  净泄 2 目录）；`fkv_runtime` 115 passed / 1 skipped（合并 `6f79f44` 后）。

## 复审记录（2026-11-07，第 32 轮结构域）

- 判定：**达标**，移入 reviewed/。修复与记录逐条一致；泄漏实测
  `global_archive_missing` 两用例连跑两遍 107 → 107 → 107（零增长，
  存量 107 为修复前历史）；全仓不再出现该手法（仅注释提及）。
