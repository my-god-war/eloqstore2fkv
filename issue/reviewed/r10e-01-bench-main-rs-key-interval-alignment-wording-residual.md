# r10e-01 P3：r9e-03 修复残留——`main.rs:385` 仍写「对齐 C++ 时显式传 4/4」，与 COMPAT_BASELINE 登记的有意偏离直接矛盾，r9e-03 的验收 grep 未真正清净

## 背景（R3 复核 r9e-03）

`issue/reviewed/r9e-03-bench-key-interval-alignment-wording-contradicts-baseline.md`
（R2 立项，并行会话修复后移入 reviewed）的修复本体已核实一半到位：

- `crate/perf/fkv_bench/src/main.rs:606-614`（`WriteMix` 文档）已改为如实描述
  「key 模型是**均匀网格**；C++ 是逐条随机步进；跨实现 p99 / 吞吐比值只在
  `key_interval = 1` 时严格可比」——原「要对齐 C++ 就显式传 `--key-interval 4`」
  已从该处删除。
- `doc/metrics.md` §6.1 已增 key_interval 步长偏离条目；`perf-74` 已追加更正段。

## 现象（亲读，2026-11-05 08:0x 版）

r9e-03 验收第 1 条明文要求：

> `grep -n "对齐 C++" crate/perf/fkv_bench/src/main.rs` 无 key_interval 相关失实措辞。

但 `main.rs:385` 仍保留同义失实措辞：

```rust
  // 写路径口径：默认「间隔 1、不删」＝历史行为，对齐 C++ 时显式传 4/4。
  let key_interval = arg_match
    .get_one::<String>("key-interval")
```

「对齐 C++ 时显式传 4/4」与被删掉的 `:607-610`「要对齐 C++ 就显式传
`--key-interval 4 --del-ratio 4`」是同一句声明，只是把 flag 名省略成数值。
按 r8e-02 的裁定：传 `--key-interval 4 --del-ratio 4` 得到的是均匀网格，
C++ 得到的是随机步进，同参数下键密度 / 树形状不同——**同参数并不对齐**。
该行是 `key_interval` / `del_ratio` 解析处的注释，属 key_interval 相关失实声明，
`grep` 未清净。

修复只改了 `:607-610` 一处，漏掉了 `:385` 这句近乎逐字同义的幸存副本。

## 目标

1. `main.rs:385` 注释改为如实描述，例如：「写路径口径：默认「间隔 1、不删」
   ＝历史行为；key 为均匀网格，与 C++ 随机步进不同（见 `COMPAT_BASELINE`
   key-interval 条目），跨实现比值仅在 `key_interval = 1` 时严格可比。」
   删去「对齐 C++ 时显式传 4/4」。
2. 复查同文件是否还有其它 key_interval / del_ratio 相关的「对齐 C++」失实措辞
   （本次全 `grep` 仅 `:385` 一处残留；`:318` 帮助文本「C++ simple_bench 用 4」
   仅陈述参数值、无对齐声明，可保留）。

## 验收

- `grep -n "对齐 C++" crate/perf/fkv_bench/src/main.rs` 无 key_interval /
  del_ratio 相关失实措辞（`:385` 命中清零）。
- 纯文档改动，无代码行为变化；`cargo check -p fkv_bench` 通过。

## 核验记录

- R3 独立审查（r10e，2026-11-05）亲读 `main.rs:355-400`（`key_interval` /
  `del_ratio` 解析段）与 `:600-626`（`WriteMix`）、`:300-340`（帮助文本）；
  `COMPAT_BASELINE` key-interval 条目、`doc/metrics.md` §6.1、`perf-74` 更正段。
- 查重：`grep -rn "显式传\|:385" issue/` 无承接条目；r9e-03 文件仅涉 `:607-610`。
  并行会话的 r9e-03 复审（`issue/reviewed/r9e-03-*.md` 末节）以一句「其余
  `对齐 C++` 命中均为参数命名/事实陈述」整体放过，未逐行核 `:385`；本条为逐行
  核对后唯一不成立的残留。

## DONE（2026-11-05，协调者直接收口）

`crate/perf/fkv_bench/src/main.rs:385` 注释改为偏离登记口径（固定步长有意偏离，
指向 COMPAT_BASELINE），grep 验收「对齐 C++ 时显式传 4/4」零命中。
`cargo check -p fkv_bench` 通过（注释级改动）。

## 复审（2026-11-05 v2）

**判定：通过，移入 `issue/reviewed/`。** 亲读核实：

- `crate/perf/fkv_bench/src/main.rs:385-386` 已改为如实描述：「写路径口径：默认
  「间隔 1、不删」＝历史行为；C++ simple_bench 用随机步进（`rand()%interval+1`），
  本仓固定步长属有意偏离，见 COMPAT_BASELINE 登记」——原「对齐 C++ 时显式传 4/4」
  已删，且不再出现任何「同参数即对齐」的声明。`grep -n "对齐 C++ 时\|显式传\|4/4"` 零命中；
  `grep -n "对齐 C++"` 剩余命中均为参数命名 / 事实陈述（含 `:318` 帮助文本「C++ simple_bench
  用 4」，仅陈述参数值，符合本条豁免）。
- 与 `COMPAT_BASELINE:64-66`（key-interval 条目：网格 vs `rand() % interval + 1`，
  跨实现比值仅作趋势参考）、`doc/metrics.md` §6.1 `:169-174`（同口径偏离条目）表述一致，
  三处互指、无矛盾。本条确为 r9e-03 复审「其余 `对齐 C++` 命中均为参数命名/事实陈述」
  一句整体放过所漏的残余，且是该文件内唯一幸存副本。
- `cargo check -p fkv_bench` 通过（exit 0，纯注释级改动，无代码行为变化）。
- 查重：`grep -rn "r10e-01" issue/` 无其它承接条目。mtime 08:13，复审时 10:18，
  非在途。
