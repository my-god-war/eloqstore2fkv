# gap-05 V6 性能门槛缺 C++ 基线资产（当前只告警，不阻断）（P2）

## 来源

dragon 的剩余工作盘点 + `questions.md` Q6 的自述。

## 证据（2026-11-05 实测）

- `questions.md` Q6：「C++ oracle 同 runner 相对比值：`perf.yml` 已接 release 二进制与
  `perf-baseline` 资产；**当前仓库无 cpp 基线资产，缺失时只告警**
  （设 `PERF_GATE_REQUIRE_BASELINE=1` 可转为阻断）」。
- `doc/migration.md` §5：**V6 未系统化**（「bench ratio 仅有流水线」）。
- 环境事实（`doc/progress.md` 范围块）：C++ 与 Rust 的 manifest framing 用 OS 页大小，
  跨页大小主机不可互读 → 性能对比也必须在**同一台机器**上跑两侧。

## 问题

现在「性能没退化」这件事**没有任何守门**：`perf.yml` 缺基线资产时只告警，任何人把
`put/get/scan` 的吞吐做掉一半，CI 也是绿的。加上跨实现比值本身受
`fkv_bench --key-interval` 与 C++ `simple_bench` 随机步进的**键分布差异**影响
（已在 `doc/progress.md` 登记为「只能作趋势参考」），所以更需要一个**同 runner、
同 workload 定义**的固定基线，而不是每次现算。

## 目标

1. 产出并入库 `perf-baseline` 资产：在指定 runner 上跑 C++（`simple_bench`）与 Rust
   （`fkv_bench`）的固定 workload 组合，记录**绝对值 + 同 runner 相对比值**。
2. 把比值门槛接成**可阻断**（`PERF_GATE_REQUIRE_BASELINE=1` 的路径先跑通，
   再决定是否设为默认）。
3. workload 定义统一：`--kv-size`、`--write-batchs`、`--test-secs`、键分布口径都要在
   两侧可对齐（随机步进 vs 均匀网格的差异必须显式归一，不能靠「都是 8 字节键」糊过去）。

## 验收

- 仓内存在可用的基线资产（或 CI 能自动产出并缓存），且缺失时**报错**而不是只告警；
- 人为把 Rust 侧某挡位性能调慢 30% 时，门槛作业必须变红（反向验证）；
- `doc/migration.md` §5 的 V6 描述更新为「有同 runner 基线 + 门槛」。

## 阻塞 / 依赖

**外部前置（硬）**：一台可同时跑 C++ 与 Rust 的 **Linux（4 KiB 页）NVMe 机器**，
用于产基线；本机（Apple Silicon）不适合作为基线来源（页大小不同 + 负载噪声）。

## 核验记录

- 实测：`questions.md` Q6 原文；`doc/migration.md` §5 V6 段；
  `doc/progress.md` 里 `--key-interval` 差异的登记条目。

## 复审（2026-11-05）

原描述引用的 `perf.yml` 在两个仓都不存在；`PERF_GATE_REQUIRE_BASELINE` 只出现在
issue markdown 里。不是“缺基线只告警”，而是没有任何门槛和基线资产。
需要新写可阻断的 perf gate（见 gap-05 目标）。
