# 待澄清问题

> `README.md` / `todo.md` 引用的清单。仓库整理时本文件缺失，2026-11-05 按
> `todo.md` 的 Q1 / Q5 / Q6 / Q7 条目重建；结论随对应 issue 更新。

## Q1：账本下一步先做 `verify` 还是 `scaffold`

- 现状：`map` + `classify` 已把 543 个 todo 分为 312 个真实剩余与 231 个范围外
  `ignored`；`scaffold`（按 unit 生成 Rust 空壳）尚未实现。
- 待定：优先为 312 个真实剩余生成骨架，还是先补 `verify` 的逐 unit 证据闭环。
- 跟踪：`issue/new/gap-01-ledger-todo-vs-scope-out-reconciliation.md`。

## Q5：`_cpp` 验证逐字节还是结构等价；哪些 crate 进 PR

- 现状：`crate/**/*_cpp` 共 39 个对照骨架；格式层（page / index / overflow /
  manifest / compress）已有逐字节双向语料，`mapping` / `store` 由 oracle 产
  结构化 payload 后按各自 schema 校验。
- 待定：引擎级 V3/V4 用逐字节还是结构等价；哪些 `_cpp` crate 进入常规 PR 门禁。

## Q6：`fkv_bench` 端到端 workload 是否复刻 `simple_bench`（已答）

- 结论：V6 不复刻全部 CLI，而是固定一组两侧可对齐的参数并写入
  `scripts/perf_gate.sh` / `doc/migration.md` §5 / `perf-baseline.json`：
  `write-read`、`rows=max_key=1000000`、`batch_size=100`、`partitions=1`、
  `read_per_part=4`、`kv_size=128`、`key_interval=4`、`del_ratio=4`、
  `test_secs=30`、`write_batchs=10000`、`scan_bytes=0`。
- C++ 侧 ini 只写 `store_path`，其余取 `KvOptions` 默认值
  （`data_append_mode=false` = PooledFilePages、O_DIRECT），Rust 侧
  `fkv_bench` 同样取 `KvOption::default()` 并固定 `with_direct_io(true)`；
  不再要求 `data_append_mode=true`。比值与 baseline 都在同一 runner 上产出，
  且 baseline / current 都记录 `data_append_mode` / `direct_io` / `bench_commit`
  （schema 2，见 `doc/perf-baseline.md`），跨模式比较会被 `compare` 拒绝。
- 键分布差异（Rust 均匀网格 vs C++ 随机步进）仍是已知限制，只用同 runner
  相对比值作门槛，不作跨实现绝对值承诺。

## Q7：`fkv_bench` 输出 JSON / `compare.yml` 的格式

- 现状：`fkv_bench --history` 继续使用 `PerfData { ..., runs: [...] }`；
  V6 门槛另用 `perf-baseline.json`（`schema: 2`，含 `workload` / `cpp` /
  `rust` / `ratios`，以及 `bench_commit` / `data_append_mode` / `direct_io`），
  由 `scripts/perf_gate.sh produce` 生成。schema 1 旧基线仍可读（跳过模式
  一致性检查并告警），但不再由 `produce` 产出。
- 待定：是否把 `perf-baseline.json` 合并回 web 看板的 `perf.json` 数据层，
  还是保持 release 资产 + 站点直接渲染 Markdown 的现状。
