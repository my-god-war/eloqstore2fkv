# perf-21 fkv_bench 延迟直方图 p999/p9999 排名公式千分位/百分位刻度错用：p999/p9999 自引入起恒等于最后一个非空桶（≈max），从未输出真实千分位

- 领域：性能（bench 计量正确性 × perf_gate 延迟门输入）
- 优先级：P3（仅 bench 工具侧指标，不影响 fkv 引擎与盘面；但 p999/p9999
  是 gate +30% 延迟回归检查的分位输入，实际比较的是 max——噪声更大，
  方向仍单调，历史上双侧同错故比对自洽）
- 置信度：高（算法逐行 + 仿真复现 + 历史 artifact 三值相等佐证；发现于
  第 32 轮 `6f79f44` 页缓存验证——读 lane 全内存化后 max 涨到窗口时长，
  使该缺陷第一次显性暴露为 p999=4,992,802µs）

## 现象（fkv `0b87909`，`crate/perf/fkv_bench/src/hist.rs:83-99`）

`LatencyHist::percentile_us` 的目标排名：

```rust
let rank = (self.count as u128 * percent as u128)
  .div_ceil(100)
  .min(u128::from(self.count));
```

`LAT_PERCENT_LI`（hist.rs:13-18）里的 `999` / `9999` 是**千分位/万分位
刻度**，但排名公式除以 **100**（百分位刻度）：

- `percent = 999`：rank = `count × 999 / 100 = 9.99 × count ≥ count`，
  被 `.min(count)` 钳到 **count（最后一个样本）**；
- `percent = 9999`：同理钳到 count。

于是 p999/p9999 的桶游走永远走到**最后一个非空桶**才满足
`cumulative ≥ rank`，返回值 = 该桶上界钳到 `max_us`
（溢出区分支 hist.rs:91-96 `((LAT_LINEAR_US) << (index + 1)).min(self.max_us)`）。
即 **p999 ≡ p9999 ≡ max（或最后非空桶上界）**，p50/p90/p95/p99 正常。

## 证据

1. **仿真复现**（按 hist.rs 逻辑 1:1 Python 复刻）：464 万样本 @1-4µs +
   单个 4,992,802µs 样本 → p50=3 / p90=4 / p99=4 全对，p999=p9999=
   4,992,802（=max）。真实千分位应仍为 4µs。
2. **历史 artifact 三值相等**：第 21 轮 `ro4.txt` `get_p999_us` =
   `get_p9999_us` = `get_max_us` = 575µs（scan 族 1676/13547 同形态）；
   PRE（c5e8fbc）实测 256 / 208µs、第 32 轮 REL（0b87909）所有 read /
   scan / fullscan 族同样 p999=p9999=max。**该缺陷早于 `6f79f44`**，
   历史各轮的 p999/p9999 数字实为 max。
3. **avg 佐证**：REL read-only pass1 avg=4.24µs——若真有 0.1% 样本
   ≥4.99s，avg 至少 5ms；故 p999=4,992,802µs 不可能是真实千分位，
   只能是排名公式把 max 当分位返回。

## 影响

- `perf_gate.sh` 的 +30% 延迟（20% 容差）门在 `*_p999_us` / `*_p9999_us`
  维度实际比较 max：单样本长尾（如 perf-22 的窗口长样本）会假红/假绿，
  只因两侧同错而方向大体自洽。
- 与 C++ `LatencyRecorder` 对拍的 p999/p9999 口径失真（比较的是两侧
  各自的 max）。

## 修法建议

排名按刻度归一：`(50,100) (90,100) (95,100) (99,100) (999,1000)
(9999,10000)` 分母入表；或 `LAT_PERCENT_LI` 直接存万分位统一除 10000。
修后补一条锁定测试：N 个 1µs 样本 + 1 个大样本 → p999 必须落在小值桶。

（发现轮：第 32 轮性能域；连带观测 perf-22 是该缺陷显性化的触发者。）

## 实现记录（2026-11-07）

- 修复 fkv commit：`a0b076c`（fkv_bench）。
- 改法：`LAT_PERCENT_LI` 与两个 `percentile_us`（LatencyHist / WindowHist）
  统一万分位刻度（p50=5_000 … p9999=9_999，rank 除 10_000）；既有 5 条
  hist 用例同步改刻度（期望值不变：100 样本下 p50/p99/p100 仍落 50/99/100
  µs 桶）；新增锁定测试 `percentile_permille_scale_is_not_clamped_to_max`
  （100 万 × 3µs + 1 × 10s：p999/p9999 必须落 3µs 桶）。
- 验证：变异（除数还原 100）→ 锁定测试红，还原后绿；fkv_bench 58/58。

## 复审记录（2026-11-07，第 33 轮性能域 + 结构域双复核）

- 判定：**达标**，移入 reviewed/。万分位刻度数学自算成立（100 样本
  p50/p99 落桶）；锁定测试在套件；两域各自亲手变异复核均红（除数还原
  100 → `percentile_permille_scale_is_not_clamped_to_max` /
  `latency_hist_linear_percentiles` 双红）；实测大样本 p999=261-294µs 与
  max 明显分离（修复前恒相等）；58/58。附注勘误：gate 的 SKIP_SUFFIXES
  本就跳过 p999/p9999/max 维度，记录中「gate 实际比较 max」的影响面比
  登记窄（该维度从不参与门槛），随修复彻底消失。
