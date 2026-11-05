# perf-73 bench 读/写单请求串行 await，无并发在飞；C++ 多读者多写者并发

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:304-313`（写：逐分区串行 await）、`main.rs:326-333`（读：单请求串行 await）、`main.rs:276-277`（`Runtime::new().block_on`）
- C++: `benchmark/simple_bench.cpp:181-197`（`partitions` 个写者同时 `ExecAsyn`）、`simple_bench.cpp:304-335`（`read_per_part*partitions` 个读者并发）、`simple_bench.cpp:321-326`（`send_req` 异步提交）

## 现状
fkv_bench：
- 写阶段：`for (part, store) in store_li.iter_mut() { store.put(tbl, &batch).await }`（`main.rs:304-312`），每个分区 batch **串行 await**，前一个完成才发下一个。
- 读阶段：`for index in 0..rows { store.get(tbl, &key).await }`（`main.rs:327-331`），单请求串行，无在飞并发。
- 全部在单个 `Runtime::block_on` 内驱动（`main.rs:277`），`Store<CompioStore>` 直连 async，不经过 `fkv_runtime`/`fkv_shard` 请求队列。

## C++ 实现 / 目标口径
C++ `simple_bench`：
- 写：`partitions` 个 Writer 同时 `store->ExecAsyn(&writer->request_, ...)`（`simple_bench.cpp:181-185`），`partitions` 个 batch 在飞。
- 读：`num_readers = read_per_part * partitions`（`simple_bench.cpp:304`），每个 reader 循环 `send_req` 异步提交，`read_per_part*partitions` 个读请求在飞。
- 全部走 `ExecAsyn` → shard work loop 真实请求调度路径。

## 差异与风险
- 并发度不可比：C++ 默认 `4*1=4` 并发读者 + `1` 并发写者（write-read 时读写同时），fkv_bench 读写均为 **1 在飞**。吞吐口径差一个数量级。
- fkv_bench 用 `Store` 直连 async，绕过 `fkv_runtime`/`fkv_shard` 请求准入、写队列、`WriteGate` 串行化逻辑，测的是「引擎裸吞吐」而非「生产请求路径吞吐」。C++ 走完整 `ExecAsyn` 路径。
- 单请求串行 await 使每个 IO 延迟直接叠加进吞吐分母，掩盖了异步并发收益，也掩盖了 shard 调度开销。

## 对齐建议
- 读/写改为并发提交：用 `futures::stream::iter` + `join_all` 或 compio `spawn` 并发驱动 `read_per_part*partitions` 个请求。
- 写阶段对 `partitions` 个 batch 并发 `put`，对齐 C++ `partitions` 写者。
- 端到端 bench 应走 `fkv_runtime::Runtime` + `Request`/`oneshot` 完成契约（`fkv_api`），而非 `Store` 直连，才能测到真实请求路径。
- write-read workload 应让写线程与读线程**同时**运行（C++ 用两个 `std::thread`），fkv_bench 当前是写完再读。

## 性能优化建议
- 并发 bench 才能暴露 shard 内 `WriteGate` 串行化、请求准入、MPSC 队列的真实开销。
- 读请求用 `no_std` 预分配 key buffer 复用，避免每请求 `key_of(index)` 分配（`main.rs:292`）。

## 严重度
P0（测量失真/验收无法达标）

## 审查结论（2026-11-04）

判定：**合理**。P0 成立：读/写阶段都是「逐请求串行 await」，全程只有 1 个请求在飞；
`--read-per-part` 只被当作「重复遍数」，没有并发语义，与 C++ 的并发读者数口径不符。

已按成立的核心部分落地（并发提交 / 在飞并发度），并把两条更大的对齐建议按边界拆出
（理由见「未落地」）。

## DONE（2026-11-04）

改动文件：`crate/perf/fkv_bench/src/main.rs`、`crate/perf/fkv_bench/Cargo.toml`
（仅本 crate，未动 `fkv_store` / `fkv_runtime` 等）。

实现：

1. **写阶段并发**：每分区一个 batch，`partitions` 个 `put` 先全部提交再统一收集
   （`futures::future::join_all`），对齐 C++ `partitions` 个写者同时 `ExecAsyn`。
   分区内每批仍只发一次 `put`，同表写入顺序不变；`partitions == 1` 退化为原串行行为。
2. **读阶段并发**：新增 `ReadLane`，链数 = `partitions * read_per_part`（对齐 C++
   `read_per_part * partitions` 个并发读者）。每链绑定一个分区，各自独立 RNG 种子
   （`lane_seed`，由全局 `READ_SEED` 派生，链间不重复）与独立 `LatencyHist`，
   `join_all` 同轮推进后 `merge` 成一份直方图。
3. **读次数不变**：每链读该分区全部 key，`Σ partition_key_num(rows, partitions, part)
   == rows`，故总读次数仍为 `rows * read_per_part`，`get_kv_s` 分母口径不变。
   `rows < partitions` 时空气分区不建链（避免 `rng.usize(0..0)` panic）。
4. `LatencyHist::merge`：桶逐项相加、`count/sum_us` 累加、`max_us` 取大；归并后分位
   与单份记录等价。
5. **读 key 零分配**（原「性能优化建议」）：`key_bytes` 已是栈上 `[u8; 8]`，无需再改。

新增单测：`latency_hist_merge_combines_counts`、`partition_key_num_covers_rows`、
`read_lanes_match_cpp_reader_num`、`concurrent_read_lanes_cover_all_partitions`。

验证：

- `cargo nextest run -p fkv_bench` → 20 passed / 0 failed。
- `cargo clippy -p fkv_bench --all-targets --no-deps -- -D warnings` → 0 警告。
- `cargo +nightly fmt -p fkv_bench -- --check` → clean。
- 端到端 `write-read`（`--partitions 4 --read-per-part 4`）跑通，全部读断言通过，
  写出 `put_/get_/scan_/scan_page_` 指标，无临时目录残留。
- 并发收益实测（release，`--rows 20000 --kv-size 128 --partitions 1 read-only`）：
  `--read-per-part 1` → `get_kv_s 119 966`；`--read-per-part 4` → `get_kv_s 217 941`
  （1.8×，总读次数相同）。证明读请求真的多路在飞，而非只改重复次数。

未落地（拆出理由）：

1. **端到端走 `fkv_runtime::Runtime` + `Request`/oneshot（对齐建议 3）**：属独立的
   架构迁移（`Runtime::start` + shard 线程 + config），会显著改变 CI 比值门禁的机器
   数值，且 `fkv_shard` 请求路径本身仍在改动中（eng-32/eng-33）。宜另开 issue 单独做，
   本 issue 只修「单请求串行」。
2. **`write-read` 写线程与读线程同时运行（对齐建议 4）**：被 `Store` API 挡住——
   `put` 需要 `&mut self`、`get` 只需 `&self`，同一张表无法同时持有可变与不可变借用；
   真要并发读写同表需要改 `fkv_store` 的 `src/lib.rs`（本 lane 明令禁改），且会让每请求
   读断言变成非确定（读到未写完的 key）。故保持「写完 → reopen → 并发读」，
   冷读口径不变。

## 复审（2026-11-05 r6）

通过。

- 写阶段 `join_all` 并发提交各分区 batch（`fkv_bench/src/main.rs:734`）。
- 读阶段 `ReadLane`（`:603`）/ `lane_seed`（`:615`）/ `read_lane_li`（`:623`，
  链数 = partitions * read_per_part），`join_all`（`:765`）并发驱动后 merge。
- `partition_key_num`（`:592`）保证总读次数 = rows * read_per_part，口径不变。
- 测试：`:1006 concurrent_read_lanes_cover_all_partitions`、`:1144
  latency_hist_merge_combines_counts`、`:1167 partition_key_num_covers_rows`、
  `:1181 read_lanes_match_cpp_reader_num` 全部在位。
- 两条拆出项（走 fkv_runtime 请求路径、write-read 同步并发读写）理由成立，
  后者受 Store API 借用规则约束的记录属实。
