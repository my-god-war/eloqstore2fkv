# r9e-03 P3：r8e-02 修复残留——`main.rs:607-610` 仍写「对齐 C++ / 要对齐 C++ 就显式传 `--key-interval 4`」，与 COMPAT_BASELINE 新登记的偏离条目直接矛盾；perf-74 更正记录未追加

## 背景（R2 复核 r8e-02）

`issue/done/r8e-02-bench-key-interval-stride-diverges-from-cpp.md` 选了「登记偏离」
路线，DONE：登记进 `COMPAT_BASELINE`。已核实 `COMPAT_BASELINE:57-59` 条目在位
（「`fkv_bench --key-interval` 是均匀网格，C++ `simple_bench` 是逐条随机步进
`rand() % interval + 1`……跨实现 p99 与吞吐比值只能作趋势参考，不纳入数据面
正确性或格式等价」），C++ 侧步长原文已亲读复核
（`benchmark/simple_bench.cpp:112/:142` `writing_key_ += FLAGS_load ? 1 :
((rand_gen() % key_interval) + 1)`）。登记本体无异议，但验收清单的措辞清理
半数未做。

## 现象（亲读，2026-11-05 06:2x 版）

1. **`crate/perf/fkv_bench/src/main.rs:607-610` 与新登记直接矛盾**：

   ```rust
   /// 写路径口径，对齐 C++ `simple_bench` 的 `key_interval` / `del_ratio`。
   ///
   /// 默认值（间隔 1、不删）等于历史行为，保证旧 `perf.json` 可比；要对齐 C++ 就
   /// 显式传 `--key-interval 4 --del-ratio 4`。
   ```

   「要对齐 C++ 就显式传 `--key-interval 4`」正是 r8e-02 立项时 perf-74 犯过的
   失实声明：传 4 得到的是均匀网格，C++ 得到的是随机步进，同参数下键密度 /
   树形状不同。`grep -n "对齐 C++" main.rs` 的 key_interval 相关命中 :607 一处
   失实（:318 帮助文本「C++ simple_bench 用 4，默认 1」仅陈述参数值，无对齐
   声明，可保留；:613 `index * key_interval` 是事实描述，可保留）。r8e-02 验收
   明确要求该 grep 无 key_interval 相关失实措辞。

2. **`doc/metrics.md` §6.1 无步长偏离条目**：§6.1（:150-166 区域）登记了
   batch-size / write-batchs / del-ratio / 分区语义偏离，§6.2 :185 只写事实
   「key 字节 = 下标 × key_interval」；步长偏离（网格 vs 随机步进、Once 模式
   差异、跨实现对比需 `key_interval=1` 或按趋势参考）只在 COMPAT_BASELINE
   有一份，metrics.md 的 §6 口径页无对应条目——查 metrics.md 口径的读者看不到
   该偏离。（DONE 以 COMPAT_BASELINE 替代 metrics.md 属合理归口，本条只要求
   两侧不矛盾 + 口径页可发现，不强制重复全文。）

3. **`perf-74` 更正记录未追加**：r8e-02 验收要求「`perf-74` 的失实对齐声明在
   其文件追加更正记录」。`issue/reviewed/perf-74-workload-param-semantics-mismatch.md`
   末节仍是 r6 复审（grep `r8e|COMPAT_BASELINE|随机步进|网格` 零命中），其
   「对齐建议」与现状（登记偏离、不对齐）已不一致，无更正指向。

## 目标

1. `main.rs:607-610` 文档改为如实描述：删「对齐」与「要对齐 C++ 就显式传」，
   改为「网格模型与 C++ 随机步进不同，见 COMPAT_BASELINE key-interval 条目；
   跨实现 p99 对比仅在 `key_interval=1`（两侧均退化为顺序写）时严格可比」。
2. `doc/metrics.md` §6.1 增一行 key_interval 步长偏离条目（或引 COMPAT_BASELINE），
   §6.2 :185 处补「与 C++ 随机步进不同，作趋势参考」半句。
3. `perf-74` 文件末尾追加更正记录：原「key_interval 对齐 C++」声明按
   COMPAT_BASELINE 登记为有意偏离，指向 `issue/done/r8e-02-*.md`。

## 验收

- `grep -n "对齐 C++" crate/perf/fkv_bench/src/main.rs` 无 key_interval 相关
  失实措辞；metrics.md §6 出现步长偏离可发现入口；perf-74 有更正段。
- 纯文档改动，无代码行为变化；`cargo check -p fkv_bench` 通过。

## 核验记录

- R9 独立审查（r9e，2026-11-05）亲读 `fkv_bench/src/main.rs` 全文（:37-53
  模块文档、:315-336 帮助、:605-626 WriteMix、:739-752 write_entry、读/scan
  链）、`doc/metrics.md` §6、`COMPAT_BASELINE:57-59`、`perf-74` 全文；
  C++ `simple_bench.cpp:105-145` 亲读。
- 查重：r8e-02 DONE 只登记 COMPAT_BASELINE；rev-01/02/95 与步长不同轴
  （r8e-02 核验记录已查）；`00-r6-io-bench` 无步长处置。无承接条目。


## DONE（2026-11-05）

DONE：bench 注释改为网格 vs 随机步进如实描述；metrics §6.1 增偏离条目；perf-74 追加更正。

## 复审（2026-11-05 v）

**判定：通过，移入 reviewed/。** 亲读三处：

- `crate/perf/fkv_bench/src/main.rs:606-614`（`WriteMix` 文档）已改为「key 模型是均匀网格
  `index * key_interval`；C++ 是逐条随机步进 `rand() % key_interval + 1`……跨实现 p99 /
  吞吐比值只在 `key_interval = 1` 时严格可比」，原「要对齐 C++ 就显式传 `--key-interval 4`」
  失实措辞已删。其余 `对齐 C++` 命中均为参数命名/事实陈述，非 key_interval 失实声明。
- `doc/metrics.md:169-170`（§6.1）新增 key_interval 步长偏离条目（网格 vs 随机步进）；
  §6.2 :189 仍为事实描述「key 字节 = 下标 × key_interval」。
- `issue/reviewed/perf-74-workload-param-semantics-mismatch.md` 末尾新增
  `## 更正（2026-11-05，r8e-02 / r9e-03）` 段，明言原「`--key-interval 4 --del-ratio 4`
  即对齐 C++」不成立并指向 COMPAT_BASELINE 与 r8e-02。三项验收均达成；纯文档改动。
