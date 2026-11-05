# r8e-02 P3：bench `--key-interval` 语义与 C++ 不一致——Rust 固定步长 `index*interval`，C++ 是逐条随机步进 `rand()%interval+1`，且该偏离未被 perf-74 / metrics.md 登记

## 现象（亲读，2026-11-05 05:55 版）

`crate/perf/fkv_bench/src/main.rs`：

- :613 `/// key 稀疏间隔：第 \`index\` 个 key 是 \`index * key_interval\`。`
- :743 / :748 `write_entry`：写 key = `shard_index(part, step, pos) * mix.key_interval`
  ——**均匀网格**（interval=4 时 key 恒为 0,4,8,12,…）。
- :979 读链、:1130-1131 scan 起点/终点同样按 `下标 × key_interval` 取 key。

C++ 对照（`benchmark/simple_bench.cpp`）：

- :111 / :141（`Writer` 构造与 `NextBatch`，两处同式）：

  ```cpp
  writing_key_ += FLAGS_load ? 1 : ((rand_gen() % key_interval) + 1);
  ```

  非 load 模式下每条写**随机步进 1..key_interval**（均匀分布，均值
  `(interval+1)/2`）；`--load` 模式恒 +1（顺序写，`key_interval` 完全不生效）。
- :114 / :144：delete 判定 `rand_gen() % del_ratio == 0`——按**随机选择**摘
  当前 writing_key_（可能从未写过），非位置确定性。

差异后果（同参数 `--key-interval 4 --del-ratio 4` 的跨实现对比）：

1. 键密度：Rust 恰好 `rows/interval` 个键、页填充率均匀；C++ 平均步长 2.5、
   键分布随机聚簇。树形状 / index 页命中率 / data 页紧凑度 / 压缩字典采样
   全部不同，`get_*` / `scan_*` p99 不可比。
2. 模式耦合：C++ `key_interval` 只在 churn 模式生效；Rust 在 Once（写一遍）
   模式同样应用步长。Once + interval=4 时 Rust 键空间是 `[0, rows*4)` 的
   稀疏网格，C++ `--load` 对应物是顺序满填。
3. delete 选择：Rust 位置确定性（每 N 位一个、删除 `N-1` 步前的键），C++ 随机
   25%（删除当前游标键）。速率同、选择分布不同——tombstone 空间分布的方差
   不同，影响 compaction / GC 压力的抖动形态。

## 登记缺口（本条立项依据）

- `perf-74`（reviewed，`issue/reviewed/perf-74-workload-param-semantics-mismatch.md`
  开发计划第 1 条）明确声称「第 index 个 key 写成 `index * key_interval`，
  **对齐 C++ key_interval**（C++ 用 4）」——对齐声明与 C++ 实际语义（随机步进）
  不符，属文档失实而非有意偏离。
- `rev-02`（reviewed）甄别小节亲核时已记录「`writing_key_` 每条目
  `+= rand()%key_interval + 1`（随机步长 1..4，非纯顺序推进）」，但其开发计划
  四点（batch 归属 / 分区键空间 / write-batchs / barrier）均未处置步长；
  DONE 亦未改步长、未登记。
- `doc/metrics.md` §6.1 / §6.2 登记了分区分片偏离、del-ratio 的分区内目标、
  test_secs / scan_bytes 口径，**没有** key_interval 步长偏离条目；§6.2 还写
  「跨 Rust / C++ 比较前先确认两侧参数一致」——同参数下负载形状仍不一致。
- 帮助文本 `main.rs:315`：「key 稀疏间隔；C++ simple_bench 用 4，默认 1」——
  把对齐语义写在帮助里，同属失实。

## 目标（二选一，裁定后另一侧同步清理）

1. **对齐**：`write_entry` 改为逐条随机步进——每条写流水持有 lane 种子派生的
   RNG，`pos += rng.usize(1..=key_interval)`（对齐 `rand()%key_interval+1`）；
   `key_interval == 1` 时退化为 +1（与现行默认行为、历史 perf.json 完全一致，
   不需要断代）。delete 判定同步改为 lane RNG 的 `rng.usize(0..del_ratio)==0`
   （或登记保留位置式）。步进引入随机性后，`live_key_num` / `is_live` /
   `live_after` 的闭式存活判定需重设计（记录每链实际写入集合或改用仿真重放），
   这是本选项的主要成本。
2. **登记偏离**：`doc/metrics.md` §6.1 增「key_interval 步长有意偏离」条目
   （固定网格 vs 随机步进、Once 模式差异、跨实现对比需 key_interval=1），
   并把 `main.rs:613` doc 与 :315 帮助文本的「对齐 C++」措辞改为如实描述；
   `perf-74` 的失实对齐声明在其文件追加更正记录。

## 验收

- 选对齐：`--key-interval 4` 时同一种子两次运行 key 序列逐字节可复现；
  `key_interval=1` 时现有测试（`write_batch_is_per_partition_entries` 等）
  bit-不变；存活断言在新模型下仍精确；clippy / nextest 全绿。
- 选登记：`grep -n "对齐 C++" crate/perf/fkv_bench/src/main.rs` 无 key_interval
  相关失实措辞；`doc/metrics.md` §6.1 出现步长偏离条目（含重开条件：
  实测跨实现 p99 对比出现键密度敏感的失真时改对齐）。

## 核验记录

- R8 独立审查（r8e，2026-11-05）亲读 `fkv_bench/src/main.rs` 全文（锚点
  :613/:743/:748/:979/:1130-1131 为 05:51 版读数）；C++ 亲读
  `simple_bench.cpp:95/:110-114/:130-132/:141-144`。
- 查重：perf-74 只裁 kv_size / key 编码 / 参数接线与 delete_target 语义，
  步长未裁；rev-02 记录了随机步进事实但四点计划均不覆盖；rev-01（kv_size）、
  rev-03（scan 页请求/随机起点）、rev-95（test_secs 窗口）与本条不同轴。
  `issue/` 全目录 grep `key_interval` 命中 perf-74 / rev-01 / rev-02 / rev-03
  / 00-r6-io-bench / run-dragon-report，均无步长处置。
- 范围注记：`fkv_bench` 归本路 scope（crate/perf/fkv_bench）；本条属清单 A
  「bench 参数与 C++ simple_bench 口径」+ 清单 D「文档声明 vs 代码事实」。


## DONE（2026-11-05，第二轮复审修复）

DONE（登记）：`key_interval` 网格 vs C++ 随机步进属 benchmark 模型差异，写入 `COMPAT_BASELINE`，不纳入数据面/格式等价。

## 复审（2026-11-05 v，lane B）

通过（选登记方案）。亲读证据：`COMPAT_BASELINE:64-67` 有
「`fkv_bench --key-interval` 是均匀网格…C++ `simple_bench` 是逐条随机步进
`rand() % interval + 1`…不纳入数据面正确性或格式等价」条目；
`doc/metrics.md:169-173` §6.1 同步登记（含「`interval = 1` 时严格可比」与
指向 `issue/done/r8e-02-*`）；失实措辞已清除——`crate/perf/fkv_bench/src/main.rs:609-612`
doc 如实描述两侧差异，帮助文本 `:318` 不再声称 key_interval 对齐 C++。
归档 `issue/reviewed/`。
