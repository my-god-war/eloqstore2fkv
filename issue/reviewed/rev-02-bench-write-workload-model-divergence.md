# rev-02 bench 写负载模型与 simple_bench 不一致：batch_size 归属、分区键空间、写入量、轮间 barrier

## 位置

- Rust：`/Users/z/git/db/kv/fkv/crate/perf/fkv_bench/src/main.rs:799-829`
  （写主循环：`while start < rows`，`main.rs:806` 按 `index % partitions` 把一批
  `batch_size` 条切给各分区，`main.rs:824` 每轮 `join_all` 收齐全部分区 put 再进下一轮）、
  `main.rs:623-625`（`effective_rows = rows.min(max_key)`）、`main.rs:314-318`（batch 默认 100）
- C++：`/Users/z/git/db/kv/eloqstore/benchmark/simple_bench.cpp:25-28`
  （`batch_size=8192`/`write_batchs=32768`/`partitions=128`/`max_key=1000000`）、
  `simple_bench.cpp:100-157`（每个 `Writer(id)` 对应一个分区，从 `writing_key_=0`
  顺序推进，超过 `max_key` 回绕 0）、`simple_bench.cpp:181-232`
  （每分区 1 个在飞请求，完成即 `NextBatch` 重发，循环 `write_batchs` 次收割）

## 现状

Rust 把 `--batch-size` 当作**整轮所有分区合计**的 key 数：一轮内每个分区只收到约
`batch_size / partitions` 条；每轮 `join_all` 收齐后才开始下一轮（分区轮间 barrier）。
写入总量 = `rows`（缺省 = `max_key`）个不同 key，**各写一次**，无重复覆盖量参数；
分区是**分片**语义：全局 key 空间 `[0, rows)` 按 `index % partitions` 均分，
每分区承载 `rows / partitions` 个 key。

## C++ 实现

- `batch_size` 是**单分区单请求**的 KV 条数：每个 `Writer` 的请求始终携带
  `batch_size` 条 entry（`simple_bench.cpp:104-129`）。
- 每个分区的 key 空间都是完整的 `[0, max_key)`：所有 `Writer` 各自从 0 递增、
  超过 `max_key` 回绕（`simple_bench.cpp:110-133`），分区之间是**同键空间的多份
  全量数据集**，不是分片。
- 写入总量 = `write_batchs × batch_size`，与 `max_key` 无关：默认参数下每分区
  约重写自己的键空间 268 遍，配合 `del_ratio=4` 的 25% delete，持续制造覆盖写、
  tombstone、compaction/GC 压力。
- 无轮间 barrier：每个 writer 完成即重发，各分区独立流水，任意时刻恰有
  `partitions` 个写请求在飞。

## 差异与风险

1. **单请求尺寸差 `partitions` 倍**：`--batch-size 8192 --partitions 128` 时
   C++ 每请求 8192 条、Rust 每请求约 64 条。group commit 合并规模、`max_write_batch_pages`
   批量页 IO、manifest 记录大小都随请求尺寸非线性变化，put 吞吐/分位不可比。
2. **每分区数据量差 `partitions` 倍**：同 `--max-key 1000000 --partitions 128`，
   C++ 每分区 100 万 key（总 1.28 亿 KV），Rust 每分区约 7800 key（总 100 万）。
   树深、文件数、index cache 命中率、点读冷读比例全部不同，`get_*` 分位跨实现失真。
3. **无覆盖写 churn**：Rust 写一遍就进读阶段；C++ 默认反复覆盖 + 删除。写吞吐、
   空间放大、GC 触发行为不是同一种负载。
4. **轮间 barrier**：Rust 每轮等最慢分区，快分区空转；C++ 各 writer 自由流水。
   异构分区延迟下写 p99 与吞吐口径不同。

## 对齐建议

- 把 `batch_size` 定义为单分区单请求条数（每轮每分区 put `batch_size` 条），
  或改名 `--batch-entries-per-part`；
- 增加 `--write-batchs`（每分区请求次数，缺省可仍按 `rows` 一遍）复刻覆盖写负载，
  并在文档里把「分区=同键空间全量副本 vs 分片」的选择显式化、与 C++ 对齐或标注有意偏离；
- 写阶段改为每分区一条独立流水线（完成即补发），去掉轮间 `join_all` barrier。

严重度：P2

## 核验记录

- 基线：eloqstore `82b774c`。
- 逐行对读 `simple_bench.cpp:87-281`（Writer/WriteLoop）与
  `fkv_bench/main.rs:799-839`；确认 C++ 每请求 `batch_size` 条、键空间按分区全量、
  `write_batchs` 与 `max_key` 解耦；确认 Rust 按 `index % partitions` 分片、
  每轮 barrier、`rows` 受 `effective_rows` 封顶。
- 读阶段链数 `partitions × read_per_part` 与 C++ `num_readers` 一致（`main.rs:666-683` vs
  `simple_bench.cpp:304`），该项**已确认对齐**，不在本 issue 范围。
- 区分：属「实现但口径不正确」——功能（写吞吐/分位）都在，参数语义与负载形状不对齐；
  非未实现。

## 甄别（2026-11-05 r7）

**结论：采纳（P2 维持）。** 四点差异事实全部亲核属实（一处算术勘误，见下），
均未被既有裁定覆盖，非重复立项。处置：第 1、3、4 点立为开发项（对齐），
第 2 点按 perf-74 哲学保留分片语义并文案登记（意见自身也给了「对齐**或**标注
有意偏离」两个选项，此处取后者，理由见分点裁定）。

### 核实（均亲读当前代码）

- 时效性：`main.rs` mtime 03:37（perf-95 落地）早于本意见 04:04，意见针对
  当前代码。逐行复核：`main.rs:799-829` `while start < rows`、`:800`
  `end = (start + batch_size).min(rows)`、`:806` `index % partitions` 切批
  （一轮 `batch_size` 条摊给 `partitions` 个 put，单请求 ≈ `batch_size/partitions`
  条）、`:824` 每轮 `join_all` 收齐再进下一轮；`effective_rows` 接线在 `:356`
  （`rows = min(rows, max_key)`）；batch 默认 100（`:314-318`）、partitions
  默认 1（`:332-334`）。全部属实。
- C++ `simple_bench.cpp`：`:25-28` 默认 8192/32768/128/1000000 属实；
  `:100-157` 每 `Writer(id)` 绑独立分区表、每请求恒 `batch_size` 条 entry、
  `writing_key_` 每条目 `+= rand()%key_interval + 1`（**随机步长 1..4**，
  非纯顺序推进，意见表述略简化但不影响结论）、超 `max_key` 回绕 0；
  `:114/:144` `rand()%del_ratio==0` → 25% delete；`:181-232` 全部 writer 满
  在飞，`finished` 队列收割即 `NextBatch` 补发，无轮间全局同步点。属实。
- **勘误**：「每分区约重写自己的键空间 268 遍」算术不准。C++ 循环计数
  `write_batchs` 是**全系统总批数**（`:186` 的 `i` 计每次 dequeue，跨全部
  writer）：总条数 32768×8192 ≈ 2.68 亿；÷128 分区 = 每分区约 210 万条，
  ×平均步长 2.5 ≈ 每分区键空间约 5 遍。268 = 总条数÷`max_key`，未除分区数。
  定性结论（写入量与 `max_key` 解耦、大量覆盖 + delete churn）不受影响。
- CI 口径（`perf.yml:35-37`）：只传 `--rows 20000 --batch-size 100
  --workload write-read`，`partitions` 走默认 1——**CI 下第 1、4 点不触发**
  （partitions=1 时 batch 两语义恒等、barrier 退化为串行本就如此）；分歧只在
  显式多分区对齐跑法咬人。这约束修法优先级与验收方式，也是 P2 而非 P1 的依据。
- `PerfData` 已记录 `partitions` 字段（`main.rs:189` 起）：改 batch 语义只影响
  `partitions>1` 的行，历史行自带口径标签，不污染曲线。

### 与相邻条目边界（去重）

- **perf-74**（reviewed）：裁定范围 = kv_size 语义（保留 value 口径+文案）、
  key 编码、`--key-interval`/`--del-ratio`、workload 枚举、**batch-size 默认值
  100 保留**（理由：改默认断 perf.json 曲线；「对齐 C++ 靠显式传参」）。
  未裁定 batch-size 的**归属语义**；且其「显式传参即可对齐」的逃生门在本点
  失效——`--batch-size 8192 --partitions 128` 下 Rust 单请求仍只 ~64 条，
  传什么参数都差 `partitions` 倍。这是与 rev-01 翻案拒绝的本质区别：第 1 点
  非翻案，是新面。perf-74 已注「多分区 delete 目标可能跨分区」仅是分片语义
  的一个症状，未裁定分片 vs 全量副本本身。
- **perf-73**（reviewed）：裁定并发度（`partitions` 个 put 在飞）；每轮
  `join_all` 是其落地形态，但「轮间 barrier vs 完成即补发」未被裁定保留，
  第 4 点是对其落地的细化，非重复。
- **perf-95**（reviewed）：裁定「写阶段次数驱动、不入 test_secs 时间窗」；
  未裁定写入量公式（`rows` 一遍 vs `write_batchs×batch_size`），第 3 点不重叠。
- **perf-71**（reviewed）：冷读/随机 key/键编码已落地；第 2 点谈每分区数据集
  尺寸（树深/文件数/cache 命中），不同轴。
- **rev-01**（todo）：kv-size 帮助文本，无交集。rev-03（scan 阶段）另案。
- 全 `issue/` grep `write_batchs|覆盖写|键空间|回绕|轮间` 除本条零命中——
  非重复立项。

### 分点裁定与开发计划

1. **batch_size 归属 → 对齐（开发项）**。改为「单分区单请求条数」：每轮每
   分区发 `batch_size` 条。`partitions=1`（CI 与全部历史默认跑法）下两语义
   恒等、行为零变化；help 文案改为「单分区单请求条数（对齐 C++ `batch_size`）」。
   不采纳改名 `--batch-entries-per-part`：`partitions=1` 等价性使现有名可直接
   换语义，避免参数面膨胀。
2. **分区键空间 → 保留分片 + 登记（perf-74 哲学）**。理由：(a) 追平 C++
   默认 = 每分区 100 万 key、全系统 1.28 亿 KV（128× 磁盘与时长），CI
   `PERF_ROWS=20000` 小负载下无意义；(b) Rust 读断言（delete 位 `get==None`、
   scan 条数 == `live_key_num`）与 `ReadLane` 模型建立在分片之上，改全量副本
   是正确性口径重设计；(c) perf-74 已有「`partitions=1` 才与 C++ 严格等价」的
   登记先例。落地：help/`WriteMix` 文档 + `doc/metrics.md` §6 注明「分区 =
   全局键空间分片；C++ 为每分区全量 [0,max_key) 副本，每分区数据量差
   `partitions` 倍，跨侧比较需 `partitions=1` 或显式换算」。
3. **写入量/churn → 新增 `--write-batchs`（开发项，key-interval/del-ratio
   同款）**。语义按 C++（**全系统总批数**，非每分区批数，避免发明第三种
   口径）：`0`（默认）= 现状 `rows` 一遍；`>0` 时写阶段改为总批数驱动，写入
   条数 = `write_batchs × batch_size`，与 `rows`/`max_key` 解耦，配合
   `--del-ratio`/`--key-interval` 复刻覆盖写 + tombstone churn；
   `PerfData` 增 `write_batchs` 字段（`#[serde(default)]`）进 history。
   CI 不开（默认 0），给 write 吞吐 gate 用真覆盖负载留出能力。
4. **轮间 barrier → 对齐（开发项，与 1 同一改造面）**。写阶段改每分区独立
   流水：每分区持续持有 1 个在飞 put，完成即取本分区下一批（C++ `:186-199`
   同构），去掉整轮 `join_all`；保留「同分区内顺序不变」（分区间本无顺序
   约束）。与 1 一并改，同一循环结构。

   开发顺序：1+4 一并（同一循环重写）→ 3 追加参数 → 2 纯文案。
   验收：既有 `partitions=1` 测试全部 bit-不变；`--partitions 4
   --batch-size 8192` 下单 put 条数断言 = 8192；`--write-batchs N` 写入总
   条数 == `N × batch_size`；clippy/nextest 全绿；`doc/metrics.md` §6 与
   help 三处口径一致。

> 让渡记录（2026-11-05）：并行会话落地，本会话核验通过。

## DONE（2026-11-05）

**状态：已完成，issue 关闭。**

改动（`crate/perf/fkv_bench/src/main.rs`）：

1. `--batch-size` 语义改为**单分区单请求条数**（对齐 C++ `batch_size`）：
   每分区一条 `WriteLane`，`next_batch` 在分区自有序列上取 `batch_size` 条；
   `partitions=1` 与旧行为逐请求等价。
2. 去掉轮间 `join_all` barrier：`join_all` 直接驱动每个分区的独立流水，
   完成即取下一批（对齐 C++ `WriteLoop` 完成即补发）。
3. 新增 `--write-batchs N`（u64→usize，缺省 0）：`N > 0` 时写阶段按**全系统**
   总批数驱动，写入条数 = `N × batch_size`，超过分区键数按分区回绕覆盖；
   共享 `AtomicU64` 预算动态认领，不预先均分。`0` = 把 `--rows` 写一遍。
   `PerfData` 增 `write_batchs`（`#[serde(default)]`）。
4. delete 目标改按**分区内序列位**回退 `del_ratio - 1`，tombstone 不再跨分区
   （`partitions > 1` 旧实现会删不存在的 key）。
5. 存活判定改为序列位口径并修正尾部：`is_live(pos, count, r)` /
   `live_key_num(count, r)`（闭式）/ `live_after(written, count, pos, r)`；
   覆盖写后读侧断言仍精确。

测试：

- 新增 `write_batch_is_per_partition_entries`（`partitions 4 / batch 8192`
  单请求 8192 条 + key 属分区）、`write_lane_wraps_within_partition`、
  `write_entries_stay_in_partition`、`write_lanes_cover_rows`、
  `claim_batch_budget_is_global`、`live_after_covers_partial_passes`、
  `churn_and_scan_bytes_smoke`；`partition_key_num_covers_rows` 等既有测试保留。
- `cargo test -p fkv_bench` 34 passed；`cargo clippy -p fkv_bench --all-targets`
  无警告；`cargo +nightly fmt -p fkv_bench`。
- 冒烟：`--rows 32768 --batch-size 8192 --partitions 4 --workload write-only`
  → 4 个 batch（`put_batch_s × batch_size == put_kv_s`），运行时断言通过；
  `--rows 2000 --batch-size 100`（partitions=1）仍为 20 batch。
- `doc/metrics.md` §6 已更新（写口径、分区语义、churn、断代）。

风险 / 有意偏离：

- 分区仍是全局键空间分片（C++ 为每分区全量副本），跨实现严格对等需
  `--partitions 1`，已在 help 与 metrics.md §6.1 登记。
- `partitions > 1` 时 `put_batch_s` 数量会变少、单批变大（本轮修复目标本身）；
  `partitions=1` 不变。
- Once 模式每分区末批可能不足 `batch_size` 条（写满 `rows` 一遍的约束）。

## 复审（2026-11-05 r7）

**判定：通过。** 逐条按 DONE 核对源码与测试（`fkv_bench/src/main.rs` mtime
04:52 有并行会话在途痕迹，但本轮声称内容全部在位且稳定，行号为复审时点读数）：

- **batch 归属单分区属实**：`WriteLane::next_batch` 在分区自有序列取恰好
  `len` 条、越界回绕（`main.rs:788-798`）；`write_batch_is_per_partition_entries`
  断言 `part 2 / step 4 / batch 8192` 每请求 8192 条、key 全落本分区
  （`global % 4 == 2`）、写满回零（`:1906-1925`）。
- **去轮间 barrier 属实**：`run_write_lane` 完成一批即取下一批、无整轮同步点
  （`:849-897`）；驱动侧 `join_all` 各分区独立流水并发推进（`:1269-1275`）。
- **`--write-batchs` 属实且口径正确**：全系统总批数——C++ `WriteLoop` 的
  `for (i = 0; i < FLAGS_write_batchs;)` 按 dequeue 跨全部 writer 计数，亲读
  `simple_bench.cpp:186-199` 证实；Rust `claim_batch` 共享 `AtomicU64` 动态
  认领不预分（`:841-843`），Churn 模式运行时断言 `kv_num == N × batch_size`
  （`:1301-1305`）；`PerfData.write_batchs` 带 `#[serde(default)]`（`:243-245`）。
- **partitions=1 行为不变属实**：Once 模式 `count == rows`、每批
  `remaining.min(batch_size)`，与旧 `while start < rows` 逐请求等价；运行时
  断言 `kv_num == rows` 且 `batch_num == Σ ceil(count/batch_size)`（`:1292-1300`）
  恰好钉死该不变量；CI（perf.yml 只传 rows/batch-size、partitions 默认 1）
  口径不受影响，与甄别分析一致。
- **delete 分区内序列位 + 存活闭式属实**：`delete_target = pos - (del_ratio-1)`
  与本条同分区（`:638-640` / `write_entry :736-749`）；`is_live` / `live_key_num`
  闭式 / `live_after` 部分前缀语义（`:648-691`），
  `live_after_covers_partial_passes` 覆盖未写 / 前缀 / 整遍 / 整遍+前缀全分支
  （`:1980-1999`）。
- **其余声称测试全在**：`write_lane_wraps_within_partition` /
  `write_entries_stay_in_partition` / `write_lanes_cover_rows` /
  `claim_batch_budget_is_global` / `churn_and_scan_bytes_smoke` /
  `perf_data_new_fields_back_compat`（旧 JSON 缺字段回读 0，`:1818-1848`）。
- **分片语义保留 + 登记属实**：键空间仍 `index % partitions` 分片（甄别第 2
  点裁定保留），`doc/metrics.md` §6.1 四项登记（独立写流水 / batch 语义 /
  write-batchs / del-ratio 分区内）+ 分片有意偏离专段全在（`doc/metrics.md:156-174`）。

验证声称（cargo test 34 passed / 冒烟）因并行会话占锁未复跑，以代码 + 测试
文本核对为准。
