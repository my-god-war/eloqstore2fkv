# fkv vs C++ EloqStore 性能测试覆盖清单

口径：本文回答「应该测什么、怎么采、两侧现在能不能采、fkv_bench 还缺什么」。
不涉及调优结论，只给可执行的测试面。

**要直接开跑**：看 §8（`scripts/perf_local.sh` 的 `--runs` / `--trials` /
`--symmetric` / 资源侧车，以及仍然缺失的指标清单）。

基线：

| 侧 | 版本 / 路径 |
|---|---|
| fkv | `/Users/z/git/db/kv/fkv`，`75a0929` |
| C++ EloqStore | `/Users/z/git/db/kv/eloqstore`，`82b774c` |
| 参考套件 | `/tmp/refkv` 浅克隆，见 §1 |

关键前提：**fkv_bench 的普通 workload 直接调用 `Store`，绕过了 shard 的写后维护**。
生产路径在 `fkv_shard` 写完成后调用 `maintain`（`crate/engine/fkv_shard/src/dispatch.rs:246-253`、
`crate/engine/fkv_shard/src/maintenance.rs:36-50`，实现见
`crate/engine/fkv_store/src/compact.rs:329-336`）。普通 workload（`write-read` /
`write-only` / `read-only` / `write-scan` / `scan`）不触发 `compact` /
`file_gc` / `clean_expired`；显式维护只发生在两个 Rust-only 诊断 lane：
`compact` lane 每轮调用 `store.compact_if_needed`
（`crate/perf/fkv_bench/src/compact.rs:84`）并计文件搬运，`ttl-write` lane 在
回收阶段调用 `store.clean_expired`（`crate/perf/fkv_bench/src/ttl.rs:50`）并计
删除条数与耗时。因此 compaction 与 TTL 回收已有 lane 级数据（§2 矩阵，
Rust-only），普通 workload 的 compaction 仍缺测；`file_gc`、compaction 的
字节级写放大、空间放大（`data` / `segment` 分项与 `space_amp`）仍**缺测**，
不是「值为 0」。

---

## 1. 调研来源与可复用点

除特别注明，下列路径相对 `/tmp/refkv`。

| 套件 | 版本 | 关键文件 | 可复用点 |
|---|---|---|---|
| RocksDB | `a9bcab5` | `rocksdb/tools/db_bench_tool.cc` | 最全的 workload 名称表（4837-5110）、flags（356-2069）、统计输出（3279-3300、5122） |
| LevelDB | `bb74ef7` | `leveldb/benchmarks/db_bench.cc` | 最小 workload 集（583-635）、`micros/op` + `MB/s` 报告（318-337） |
| YCSB | `ae02b31` | `ycsb/workloads/workload{a..f}`、`workload_template` | 读写比例、zipfian/latest、scan 长度分布、RMW、histogram 配置 |
| WiredTiger wtperf | `4ca01f7` | `wiredtiger/bench/wtperf/wtperf.c`、`wtperf_config.c` | 分操作类型的 ops/s 与 avg/min/max 延迟 CSV（1196-1220）、延迟桶（198）、checkpoint/scan/truncate |
| redb | `17a28fa` | `redb/crates/redb-bench*/benches/*.rs` | 整数 key、1M 批量插入、1M×150B + 4000×2MB 大 value、1/2/4/8/16/32 线程扫描 |
| sled | `e449d17` | `sled/examples/bench.rs` | 每线程 4M 写、并发度数组、flush、`db.stats()` |
| C++ EloqStore | `82b774c` | `benchmark/simple_bench.cpp`、`benchmark/load_bench.cpp`、`tests/large_value_benchmark.cpp` | 与 fkv 直接对标的 workload 和输出字段 |

RocksDB 的 workload 名称可归成这些族：`fillseq` / `fillrandom` / `filluniquerandom` /
`overwrite` / `fillsync` / `fill100K` / `readseq` / `readreverse` / `readrandom` /
`readmissing` / `readhot` / `multireadrandom` / `seekrandom` / `seekrandomwhilewriting` /
`readwhilewriting` / `readrandomwriterandom` / `updaterandom` / `appendrandom` /
`deleteseq` / `deleterandom` / `compact` / `flush` / `stats`（`db_bench_tool.cc:4837-5110`）。

---

## 2. workload 类型覆盖矩阵

标记：✅ 有独立负载，⚠️ 只能近似/需改参数，❌ 无。

| 类型 | 定义 | RocksDB / LevelDB | YCSB | wtperf | C++ simple_bench | fkv_bench 现状 |
|---|---|---|---|---|---|---|
| fillseq | 顺序写满一遍 | ✅ `fillseq` | ⚠️ insert + hashed | ⚠️ insert | ✅ `load` / 顺序写 | ✅ 默认写（有序推进，`lane.rs:115-173` 生成 key 序列，`runner.rs:337-355` 起写阶段） |
| fillrandom | 随机 key 写 | ✅ `fillrandom` / `filluniquerandom` | ✅ insert | ⚠️ insert | ⚠️ 步进 1..4 随机（`simple_bench.cpp:111`） | ⚠️ `--rng cpp` 与 C++ 同源随机游走；`--rng grid` 才只前进 |
| overwrite | 覆盖写已有 key | ✅ `overwrite` | ✅ update | ✅ update | ✅ 非 load 写即 upsert | ⚠️ `--write-batchs` 回绕可近似 |
| readrandom | 随机点读 | ✅ `readrandom` / `readhot` | ✅ workload A/B/C | ✅ read | ✅ read | ✅ `read-only` / `write-read` 读阶段 |
| readseq / readreverse | 顺序/逆序读 | ✅ `readseq` / `readreverse` | ❌ | ⚠️ scan | ❌ | ❌ 只有 fullscan |
| seekrandom / floor | 随机 seek、前驱查询 | ✅ `seekrandom` | ❌ | ❌ | ❌ | ❌ `Store::floor` 存在（`read.rs:451`）但未测 |
| scan（页） | 固定页大小随机起点 | ✅ `multiscan` | ✅ workload E | ✅ scan | ✅ `scan` | ✅ `scan` / `write-scan`，每请求 256 条 |
| scan（区间/全表） | 指定字节窗或整表 | ✅ `seekrandom` + `max_scan_distance` | ✅ `maxscanlength` | ✅ `scan_pct` | ✅ `scan_bytes` | ✅ `--scan-bytes`，另有 `fullscan_*` |
| delete | 纯删除 / 删除比例 | ✅ `deleteseq` / `deleterandom` | ⚠️ 无独立 delete | ✅ truncate | ⚠️ `del_ratio=4` 混在写里 | ⚠️ `--rng cpp` 固定每 4 条一次 delete（C++ 同源）；`--rng grid` 用 `--del-ratio`；无 delete-only |
| mixed read-write | 读写同时压 | ✅ `readwhilewriting` / `readrandomwriterandom` | ✅ A/B/D | ✅ read+insert/update | ✅ `write-read` 真并发 | ❌ `write-read` 是「写完、reopen、再读/scan」的**分阶段**（`runner.rs:307-425` 的 `run_pipeline`） |
| read-modify-write | 读后写回 | ✅ `updaterandom` | ✅ workload F | ✅ modify | ❌ | ❌ |
| insert/append | 只追加不覆盖 | ✅ `appendrandom` | ✅ insert | ✅ insert | ⚠️ load | ⚠️ 写模式只有 Once/Churn，无 append 语义 |
| 大 value | 1MB 级以上 | ✅ `fill100K` / `value_size_max` | ⚠️ fieldlength | ✅ `value_sz_max` | ✅ 独立 `large_value_benchmark.cpp` | ❌ 只用 `put`，未接 `put_large` / `with_large_value` |
| TTL / 过期清理 | 写入带过期时间后清理 | ✅ `ttl_seconds` | ❌ | ❌ | ⚠️ 条目支持 `expire_ts`，bench 未用 | ✅ `ttl-write` lane：`--ttl-ratio` 写 `expire_ts` 并计 `clean_expired`（Rust-only） |
| recovery / reopen | 重启回放、崩溃恢复 | ✅ `open` / `db_stress` | ❌ | ⚠️ checkpoint | ⚠️ `Start` 可被外部计时 | ❌ reopen 不计时（`runner.rs:383-388`），且结尾删目录 |
| compaction | 主动/后台 compaction | ✅ `compact` / `compactall` | ❌ | ⚠️ checkpoint | ⚠️ 写路径内联 `CompactIfNeeded`（`write_task.cpp:885`） | ✅ `compact` lane 在 bench 内触发 `compact_if_needed` 并计搬运（Rust-only）；普通 workload 不触发 |
| space amplification | 物理/逻辑比 | ✅ `stats` | ❌ | ❌ | ⚠️ `local_space_used` 仅云端非 0 | ⚠️ `store_bytes` 已输出；无 data/segment 分项与 `space_amp` |
| 并发扫描 | 线程数扫描 | ✅ `--threads` | ✅ `threadcount` | ✅ `threads` | ✅ `partitions` / `read_thds` | ⚠️ 只有 `partitions` / `read-per-part`，写并发等于分区数 |

### 2.1 fkv_bench 现状（`fkv/crate/perf/fkv_bench/`，核对到 `75a0929`）

- 7 个 workload：`write-read` / `write-only` / `read-only` / `write-scan` / `scan`，
  以及 Rust-only 诊断 lane `ttl-write` / `compact`（`cli.rs` 的 `--workload` 帮助、
  `workload.rs` 的 `Workload`）。后两者不参与 C++ 比值：`ttl-write` 用
  `--ttl-ratio` 写 `expire_ts` 并计 `clean_expired`（lane 内强制
  `--rng grid`，因为 TTL 位置按序列位选取），`compact` 在 bench 内触发
  `compact_if_needed` 并统计搬运文件 / 吞吐 / 并发读。
- key / delete 生成默认 `--rng cpp`：逐位复刻 C++ `simple_bench` 的
  `mt19937(0)` 随机游走 + 每 4 条一次 delete；此时 `--key-interval` /
  `--del-ratio` 被忽略，history / baseline 记录生效值
  `rng=cpp, key_interval=4, del_ratio=4`（`cli.rs` 的 `CPP_KEY_INTERVAL` /
  `CPP_DEL_RATIO`）。只有 `--rng grid` 才走均匀网格并读取这两个 flag
  （默认 1 / 0）。`--rng cpp` 下不再是「key 只前进」。
- 写阶段是每分区一条流水，支持 `--write-batchs` 回绕覆盖；`--rng grid` 时
  还有 `--key-interval` 空洞与 `--del-ratio` tombstone。**没有 delete-only、
  没有 RMW；grid 口径没有随机写（cpp 口径已是随机游走）。**
- `write-read` 不是并发：先整段写，再 `reopen`，再整段读，随后还跑 scan /
  fullscan。C++ `write-read` 是写线程与读线程同时跑
  （`eloqstore/benchmark/simple_bench.cpp:593-602`）。
- 指标（`--history` / `runs`）：
  - 吞吐：`put_kv_s`、`put_batch_s`、`get_kv_s`、`scan_kv_s`、`scan_page_s`、
    `fullscan_kv_s`、`{put,get,scan,fullscan}_mib_s`；
  - 延迟：`{put,get,scan,fullscan}_avg/p50/p90/p95/p99/p999/p9999/max_us`；
  - 秒级窗口：`{put,get,scan}_window_num/qps/kv_s/p50_us/p90_us/p99_us`；
  - 资源 / IO：`rss_peak_kb`、`store_bytes`、`io_fd_open_num`、
    `io_write_at_num`、`io_read_at_num`、`io_data_sync_num`、
    `io_dir_fsync_num`；Linux 额外有 `/proc/self/io` 的
    `proc_io_read_bytes` / `proc_io_write_bytes`（`resource.rs`）；
  - 派生：`reopen_us`、`write_amp`、`space_amp_nominal`、
    `write_at_per_batch`、`sync_per_batch`；
  - 诊断 lane：`put_ttl_*` / `ttl_clean_*`、`compact_*` / `compact_put_*`。
- 仍缺：设备级 IOPS / 吞吐 / 队列深度、fsync 等待时间、data cache 命中率
  （index cache 已有 `cache_*`）；
  CPU 时间 / CPU% 由脚本 `time -v` 外采；
  delete-only / RMW / floor / 大 value 的独立 workload 仍未接（随机写在
  `--rng cpp` 下已覆盖）。
- 分区语义与 C++ 不同：fkv 是全局 key 空间切片，C++ 是每分区一份 `[0,max_key)` 全量副本，
  严格对等需 `--partitions 1`（`doc/metrics.md` §6）。
- `--kv-size` 是整对 KV 宽度，value = kv_size - 8。
  大 value 路径 `Store::with_large_value`（`config.rs:251`）和 `put_large`（`write.rs:377`）
  都未接线。

---

## 3. 指标清单：口径、采集方式、两侧可行性

优先级：P0 必测，P1 强烈建议，P2 诊断加分。
「fkv / C++」列写的是**当前能否采到**，不是引擎有没有这个能力。

### 3.1 吞吐

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| ops/s | 完成请求数 / 墙钟窗口 | bench 自带计数 + `Instant` | ✅ 已有 `put_batch_s` / `scan_page_s` | ✅ `kvs/s`、`QPS`（`simple_bench.cpp:250-272, 352`） | P0 |
| kv/s | 条目数 / 秒 | bench 自带 | ✅ 已有 | ✅ 已有 | P0 |
| MB/s（逻辑） | kv/s × (key+value) / 2^20 | bench 算，零成本 | ❌ 加一行 | ✅ `MiB/s`（`simple_bench.cpp:215-217`） | P0 |
| MB/s（设备） | 设备读/写字节 / 秒 | cgroup `io.stat`、`/proc/diskstats`、`iostat -x` | ⚠️ 外部 | ⚠️ 外部 | P1 |
| 并发度 / 在飞请求 | 同时在飞的请求数 | bench 自带 | ⚠️ 只等于分区数 | ✅ `read_thds` / `partitions` | P1 |
| 分操作吞吐 | put/get/delete/scan/floor/RMW 各自 ops/s | bench 按 op 分桶 | ⚠️ 只有 put/get/scan | ⚠️ 只有 write/read/scan | P0 |

### 3.2 延迟分位与尾延迟

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| avg / p50 / p90 / p99 / p999 / p9999 / max | 每请求提交到完成，微秒 | bench 内直方图 | ✅ 已有（`hist.rs:6-10, 23-55, 109-120`，1µs 线性桶 + 2 的幂溢出） | ✅ bvar `LatencyRecorder`（`simple_bench.cpp:54-56, 250-272`） | P0 |
| 分位跨实现可比性 | 同一批样本、同一桶边界 | 两侧都用 bench 自己的直方图，不读引擎 17 桶 | ⚠️ 需固定桶/样本口径 | ⚠️ bvar 桶边界需记录 | P0 |
| 尾延迟尖峰 | 每个采样窗口的 max、p999 | 窗口化直方图 + 时间序列 | ❌ 只有全程 max | ⚠️ 可加 | P1 |
| 慢请求计数 | 超过阈值（如 10ms/100ms）的请求数 | 直方图溢出桶或计数 | ❌ 可加 | ❌ 可加 | P1 |
| 分操作延迟 | put/get/delete/scan/floor/RMW 各自分位 | 每 op 一个直方图 | ⚠️ 只有 put/get/scan/fullscan | ⚠️ 只有 write/read/scan | P0 |
| 排队延迟 | 从入队到开始执行 | 引擎打点或 bpftrace | ❌ 需引擎 | ❌ 需引擎 | P2 |

注意：fkv 引擎侧 17 桶（`fkv_metrics`）与 C++ 外部 `meter` 桶边界未证明一致，
跨实现 p99 只能用 bench 自己的细粒度直方图，见 `doc/metrics.md` §2。

### 3.3 CPU

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 用户/内核 CPU 时间 | `ru_utime + ru_stime` | `getrusage(RUSAGE_SELF)` | ❌ 可加（可移植） | ⚠️ 外部 `time -v` | P0 |
| CPU 利用率 | CPU 时间 / 墙钟 / 核数 | 同上 | ❌ 可加 | ⚠️ 外部 | P0 |
| 每 op CPU | CPU ns / op | 同上相除 | ❌ 可加 | ⚠️ 外部 | P1 |
| cycles / instructions / IPC | 硬件计数 | `perf stat -p PID` | ⚠️ 外部 | ⚠️ 外部 | P1 |
| cache-miss / branch-miss | 硬件计数 | `perf stat` | ⚠️ 外部 | ⚠️ 外部 | P2 |
| 上下文切换 / 自愿让出 | `ctxt`、`nvcsw` | `/proc/self/status`、`/proc/self/schedstat`、`perf stat` | ⚠️ 外部 | ⚠️ 外部 | P1 |

### 3.4 内存

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| RSS（当前） | 常驻集 | `/proc/self/status: VmRSS`、`getrusage.ru_maxrss` | ⚠️ 只采峰值 | ⚠️ 外部 | P0 |
| 峰值 RSS | 进程生命周期峰值 | `VmHWM` | ✅ 已有 `rss_peak_kb`（`report.rs:60-73`） | ⚠️ `/usr/bin/time -v` | P0 |
| 虚拟峰值 | `VmPeak` | `/proc/self/status` | ❌ 可加 | ⚠️ 外部 | P1 |
| 匿名 / 文件映射 / 共享 | `RssAnon` / `RssFile` / `RssShmem` | `/proc/self/smaps_rollup` | ❌ 可加 | ⚠️ 外部 | P1 |
| mmap 字节 | 映射总大小 | `/proc/self/maps` 求和 | ❌ 可加 | ⚠️ 外部 | P2 |
| index cache 占用/命中/淘汰 | 字节、hit/miss/evict | fkv `Store::index_cache_stat()`（`state.rs:78-99`） | ✅ bench 已输出 `cache_*`（`resource.rs`） | ✅ `index_buffer_pool_used/limit` gauge（`eloqstore_metrics.h:43-48`） | P0 |
| data cache 命中 | 数据页 hit/miss | C++ `DataCacheHits()/DataCacheMisses()`（`eloq_store.h:1122-1125`） | ❌ 无 data cache（append + direct IO） | ✅ API 存在 | P1 |
| IO 缓冲池常驻 | 读/写 pooled bytes | fkv `read_buf_pooled_bytes` / `write_buf_pooled_bytes`（`api.rs:213-220`） | ✅ API 存在，bench 未调用 | ⚠️ `direct_io_buffer_pool_size` 间接 | P1 |
| cgroup 内存 | `memory.current` / `memory.peak` | cgroup v2 | ⚠️ 外部 | ⚠️ 外部 | P1 |

### 3.5 磁盘空间与放大

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 目录总大小 | 数据目录所有文件字节 | `du -sb` 或递归 `stat` | ✅ `store_bytes`（bench 删目录前统计；`perf_local` 另有轮询近似） | ⚠️ 外部 | P0 |
| data / segment / manifest 分项 | 按文件名前缀求和 | fkv `data_` / `segment_` / `manifest_`（`fkv_types/src/lib.rs:73-85`），C++ `data_` / `segment_` / `manifest_`（`common.h:25-29`） | ⚠️ bench 只输出 `store_bytes`；`perf_local.sh` 侧车按前缀拆 | ❌ 可加 | P0 |
| 逻辑存活字节 | 存活 key × (key+value) | bench 计数（`live_key_num`） | ⚠️ 只在固定写模式可精确断言 | ⚠️ 可加 | P0 |
| 空间放大 | 物理字节 / 逻辑存活字节 | 上两项相除 | ❌ 可加 | ❌ 可加 | P0 |
| 死字节比例 | 1 - 存活 / 物理 | 同上 | ❌ 可加 | ❌ 可加 | P1 |
| 每 1M key 的空间 | 物理字节 / 百万 key | 同上 | ❌ 可加 | ❌ 可加 | P1 |
| index 放大 | index 页字节 / 逻辑字节 | 需按页类型统计 | ❌ 需引擎 | ❌ 需引擎 | P2 |
| segment 放大（大 value） | segment 物理 / 逻辑 | fkv `segment_*` 前缀 + 大 value 计数 | ❌ 可加 | ⚠️ 外部 | P1 |
| 文件数 / 段数 | 目录内文件个数 | 递归计数 | ⚠️ `perf_local.sh` 侧车统计；bench 不输出 `file_num` | ❌ 可加 | P1 |
| 本地空间 gauge | 引擎自报已用字节 | C++ `GetLocalSpaceUsed()`（`async_io_manager.cpp:4208-4216`） | ❌ 需 getter | ⚠️ gauge 存在，但本地 `IouringMgr` 固定返回 0（`async_io_manager.h:703-709`），本地盘仍需 `du` | P1 |

补充：fkv 的 data 文件同时装 leaf / index / overflow 页，**无法只靠文件名拆出 index 放大**，
需要引擎按页类型记账。C++ 同样把 index 与 data 放在同一类 data 文件里。

### 3.6 IO、IOPS、fsync

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 逻辑读/写次数 | 页/段/file 级 op 数 | fkv `read_at_num` / `write_at_num`（`api.rs:157-166`），scan 计数在 `data_scan_num` / `segment_scan_num` 但暂无公开 accessor | ⚠️ `io_read_at_num` / `io_write_at_num` / `io_fd_open_num` 已随 bench 输出 | ❌ 无同级计数 | P1 |
| 逻辑读写字节 | 引擎层字节 | fkv `fkv_metrics` 的 `bytes_read` / `bytes_written`（`fkv_metrics/src/lib.rs:306-317`） | ⚠️ shard 侧有，bench 未接 | ❌ 无 | P1 |
| syscall 次数 | `syscr` / `syscw` | `/proc/self/io` | ⚠️ 外部（bench 只输出 `read_bytes` / `write_bytes`） | ⚠️ 外部 | P2 |
| 进程读写字节 | `rchar` / `wchar` / `read_bytes` / `write_bytes` | `/proc/self/io` | ✅ `proc_io_read_bytes` / `proc_io_write_bytes`（bench 自报 `read_bytes` / `write_bytes`） | ⚠️ 外部 | P1 |
| 设备 IOPS | 设备级读/写 IOPS | `iostat -x`、`/proc/diskstats`、cgroup `io.stat` | ⚠️ 外部 | ⚠️ 外部 | P0 |
| 设备吞吐 | 设备读/写 MB/s | 同上 | ⚠️ 外部 | ⚠️ 外部 | P1 |
| fsync / fdatasync 次数 | 提交到内核的同步次数 | fkv `data_sync_num` / `dir_fsync_num`（`api.rs:264-272`） | ✅ `io_data_sync_num` / `io_dir_fsync_num` 已随 bench 输出 | ❌ 无内置，需 `bpftrace`/instrument | P0 |
| fsync 总耗时 | 同步等待时间 | 引擎打点或 `bpftrace` | ❌ 需打点 | ❌ 需打点 | P1 |
| 读放大 | 设备读字节 / 逻辑读字节 | 上两项相除 | ⚠️ 外部 + 引擎 | ⚠️ 外部 | P1 |
| IO 队列深度 / 单 IO 延迟 | 设备级 | `iostat -x`、`blktrace`、`bpftrace` | ⚠️ 外部 | ⚠️ 外部 | P2 |

注意：`/proc/self/io: write_bytes` 只统计已提交到块层的写，受 page cache 延迟影响。
设备级放大优先用 cgroup `io.stat` 或 `iostat`。

### 3.7 写放大、compaction、GC

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 写放大（外部） | 设备写字节 / 逻辑写字节 | cgroup / `/proc/self/io` | ⚠️ 可外部 | ⚠️ 可外部 | P0 |
| 写放大（引擎分项） | flush / compaction / GC 各自字节 | 引擎计数器 | ❌ 需接线 | ❌ 需新增 | P1 |
| compaction 次数 / 文件数 | 触发次数、搬移文件数 | fkv `maintain` 返回值（`compact.rs:329-336`） | ✅ `compact` lane 输出 `compact_run_num` / `compact_file_num` | ✅ `request_type=compact` 计数（`metrics.md` §1） | P0 |
| compaction 耗时 / 字节 | 每次耗时、读写字节 | 引擎打点 | ⚠️ `compact_file_s` / `compact_put_*` 已输出；字节级仍缺 | ❌ 需打点 | P1 |
| GC 次数 / 回收字节 | file_gc / local_gc | fkv `fkv_metrics` 的 `file_gc` / `local_gc`，C++ 同名 request_type | ⚠️ shard 侧有 | ✅ 指标标签有 | P1 |
| 写停顿 / 背压 | 写被限速或等待的时间 | 引擎打点、`iostat` 队列 | ❌ 需引擎 | ⚠️ RocksDB 有 stall ticker 可借鉴（`statistics.h:205`） | P1 |
| 每批写条目数 / 批数 | batch size、batch 数 | bench 自带 | ✅ 已有 | ✅ 已有 | P0 |

### 3.8 恢复

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 干净 reopen 时间 | 进程启动到 `reopen` 完成 | bench 内 `Instant` 包裹 `Store::reopen` | ❌ 可加（现在 reopen 不计时） | ⚠️ 外部包 `Start` | P0 |
| 启动到首次成功读 | 端到端可用时间 | 进程外计时 + 首读 | ❌ 需保留目录 | ⚠️ 外部 | P0 |
| 崩溃恢复时间 | `kill -9` 后重启到可读 | 崩溃注入脚本 | ❌ 无 harness | ❌ 无 harness | P1 |
| 回放条目 / 字节 | manifest 回放量 | fkv `fkv_task_reopen` / `replay_manifest_bytes_with_len`（`manifest.rs:282-300`） | ❌ 需打点 | ❌ 需打点 | P2 |
| 恢复后数据正确性 | 校验和 / 计数 | 全表 scan 计数 + checksum | ⚠️ 可加 | ⚠️ 可加 | P0 |
| 冷/热缓存区分 | drop cache 后重启 | `echo 3 > /proc/sys/vm/drop_caches`（需 root） | ⚠️ 外部 | ⚠️ 外部 | P1 |

### 3.9 查询侧的 floor / seek / scan

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| floor 延迟 | 前驱查询 | bench 内直方图 | ❌ 可接 `Store::floor`（`read.rs:451`） | ⚠️ 需确认 API | P1 |
| seek + 小窗口 scan | 起点 + 1..N 条 | `scan_page` / `scan_entry_page` | ⚠️ 只测 256 条页 | ✅ `scan_bytes` | P1 |
| scan 吞吐（条/秒、页/秒、MB/s） | 页大小 × 页数 | bench 自带 | ✅ `scan_page_s` / `scan_kv_s`，缺 MB/s | ✅ `scan speed ... MB/s`（`simple_bench.cpp:502-516`） | P0 |
| 点读命中/未命中 | 存在 / 不存在 key | 两次 workload | ❌ 可加 `readmissing` | ⚠️ 需新增 workload，现有 ReadLoop 只读已写 key | P1 |

### 3.10 稳定性与报告

| 指标 | 口径 | 采集方式 | fkv | C++ | 优先级 |
|---|---|---|---|---|---|
| 重复运行的离散度 | 中位数、标准差、CI95 | 3-5 次运行 | ⚠️ `--history` 可存，不聚合 | ⚠️ 同 | P0 |
| 时间序列 | 每秒 ops/s、p99、RSS | `--report-interval` 类参数 | ❌ 可加 | ✅ RocksDB 有 interval percentiles（`db_bench_tool.cc:1795-1807`） | P1 |
| 失败/错误计数 | 错误码分布 | bench 计数 | ⚠️ 部分 unwrap 直接 panic | ⚠️ 同 | P0 |
| 运行元数据 | commit、内核、文件系统、设备、CPU、参数 | JSON 字段 | ⚠️ 部分有（`PerfData`） | ⚠️ 外部脚本 | P0 |

---

## 4. 改造分层：现在就能加 / 需要 profile / 需要引擎

### 4.1 只改 `fkv_bench` 就能加（无需新硬件、无需内核计数器）

1. **MB/s**：`kv_num * (value_size + 8) / elapsed / 2^20`，写/读/scan 各一份。
2. **CPU 时间与 CPU%**：`getrusage(RUSAGE_SELF)`，Linux/macOS 都可用。顺带输出
   `cpu_ns_per_op`。
3. **RSS 当前/峰值/虚拟峰值**：`VmRSS`、`VmHWM`、`VmPeak`，复用现有 `parse_vmhwm`
   （`report.rs:60-65`）扩展成一张表。Linux 再加 `smaps_rollup` 的 `RssAnon/RssFile/RssShmem`。
4. **目录空间**：写入阶段结束后、删目录之前，递归 `stat` 求和，按前缀拆
   `data_` / `segment_` / `manifest_`，并计算 `physical / live` 空间放大。
   需要把 `live_key_num` 的存活计数接到指标里（已有函数，`workload.rs:167`）。
5. **fsync 次数**：bench 构造 `CompioStore` 后保留句柄，读 `data_sync_num()` /
   `dir_fsync_num()`（`api.rs:264-272`）。若 `Store` 拿走所有权，给 `Store` 加一个
   `pub fn io(&self) -> &I` 的只读访问器即可，改动一行。
6. **index cache 命中**：每阶段前后调 `store.index_cache_stat()`
   （`state.rs:78-99`，public），输出 hit/miss/evict/bytes/len/capacity。
7. **reopen 计时**：把 `store.reopen(tbl)` 包进 `Instant`，按表输出
   `reopen_us`。加 `--keep-root`，跑完不删目录，供下一进程做恢复测试。
8. **compaction / GC 维护**：在写流水每 N 批后调用 `store.maintain(tbl)`
   （`compact.rs:329-336`），统计返回的文件数、耗时、空间前后差。这是让 bench
   与生产 shard 对齐的关键一步（`fkv_shard/src/maintenance.rs:36-50`）。
9. **随机写 / delete-only / RMW**：新增 `--write-order random`，按 key 空间洗牌；
   新增 `delete-only` workload。RMW 可在读链里读后立刻 `put` 同 key。
10. **floor / seek**：新增 `floor` workload，调 `Store::floor`（`read.rs:451`）；
    scan 增加 `--scan-entries 1` 之类的小窗口模式。
11. **大 value**：bench 接 `with_large_value(layout, threshold)`（`config.rs:251`）
    和 `put_large`（`write.rs:377`），新增 `large_value` workload。
12. **TTL**：`BatchEntry` 已有 `expire_ts`（`fkv_btree/src/lib.rs:32-44`），写时填未来时间，
    阶段中调 `clean_expired(tbl, now_ms)`（`ttl.rs:251`），计时并统计回收字节。
13. **并发混合**：把 `write-read` 从「分阶段」改成「写链 + 读链同时启动」，
    与 C++ `simple_bench.cpp:593-602` 对齐。保留旧分阶段模式做冷读基线。
14. **统一 JSON 字段**：在 `PerfData` 里加 `phase`、`op`、`latency_percentiles`、
    `cpu_*`、`mem_*`、`space_*`、`io_*`、`maintenance_*`，方便跨实现对比。

### 4.2 需要 profile / 内核计数器 / 外部工具（两侧通用）

1. **设备 IOPS、吞吐、利用率、队列深度**：`iostat -x 1` 或 cgroup v2 `io.stat`。
2. **设备级读写字节与写放大**：cgroup `io.stat`（比 `/proc/self/io` 更接近块层）。
3. **CPU 周期、指令数、cache/branch miss**：`perf stat -p <pid>`。
4. **上下文切换、调度延迟**：`/proc/self/schedstat`、`perf sched`。
5. **C++ fsync 次数**：`bpftrace -e 'tracepoint:syscalls:sys_enter_fsync,...'`，
   io_uring 路径用 `io_uring:io_uring_submit_sqe` 过滤 `IORING_OP_FSYNC`，
   或直接给 `async_io_manager` 加计数器。
6. **单 IO 延迟与 block 层排队**：`blktrace` / `bpftrace`。
7. **page cache 命中（fkv 无 data cache 时）**：对数据文件做 `mincore` 采样，
   或看 `/proc/self/smaps` 的 `RssFile`。
8. **冷启动**：`drop_caches`，需 root，记录是否真的丢弃。

### 4.3 需要引擎侧补 API / 打点

1. **真写放大分项**：flush / compaction / GC 各自写入字节。
   fkv 的 `fkv_metrics` 已有 `bytes_read/bytes_written`，但上报通道未接到 bench
   （`api.rs:202` 注释），需要 `Store` 暴露快照或在 `Store` 内累计。
   C++ 目前没有字节级 ticker，需要新增。
2. **按页类型记账**：leaf / index / overflow 页数与字节，用于 index 放大。
   两侧都没有现成公开计数。
3. **空间统计 getter**：fkv `mapper.space_size()` 只在 `needs_maintain` 内部用
   （`compact.rs:114-132`），需 `pub fn space_stat()` 返回 `(mapped, space, segment)`。
4. **compaction / GC 耗时与字节**：fkv `maintain` 可包计时，C++ 需在
   `WriteTask` / `BackgroundWrite` 打点。
5. **恢复回放量**：fkv `replay_manifest_bytes_with_len` 的输入长度与条目数
   （`manifest.rs:282-300`），C++ `Replayer` 同理。
6. **C++ fsync 计数**：在 `async_io_manager.cpp` 的 `io_uring_prep_fsync` 两处
   （2316、2562）加原子计数，比 bpftrace 稳定。
7. **cache 命中**：fkv 只有 index cache（`CacheStat`），如需 data cache 命中要先有
   data cache，当前 append + direct IO 没有。
8. **in-flight / 队列深度**：引擎任务队列打点，用于解释 p99 抬升。

---

## 5. 建议测试矩阵

14 个必跑 case（C01-C14）+ 1 个条件项（C15）。默认 key 8B，value = kv_size - 8，
单机同盘，每 case 至少 3 次，报告 median + CI95。C10 的 10MB 子项和 C12 的
崩溃恢复子项按机器条件启用。

| ID | 目的 | 负载 / 参数 | 必采指标 | 对齐注意 |
|---|---|---|---|---|
| C01 | 顺序写基线 | `write-only`，128B，10M key，batch 100 / 8192，1 分区 | kv/s、MB/s、p50/p99/p9999、CPU、RSS、目录大小 | 与 C++ `load` 对齐，注意 C++ 默认 batch 8192 |
| C02 | 随机写 | 新增 `--write-order random`，10M key，128B | 同上 + 空间放大 | C++ 用步进 1..4 随机，需说明不是同一分布 |
| C03 | 覆盖写 churn | `write-only`，总写入量 3×rows 条（`--write-batchs = 3×rows/batch_size`），`--del-ratio 4`，128B / 1KB | kv/s、写放大、空间放大、compaction 次数 | C++ 固定 `del_ratio=4`（`simple_bench.cpp:83-85`），fkv 默认 0，需显式对齐 |
| C04 | 冷读随机 | `read-only`，reopen 后立即读，index cache 默认 | get p50/p99/p9999、cache hit、IOPS | fkv reopen 不计时，需先加 `reopen_us` |
| C05 | 热读 / 倾斜 | 新增 zipfian 或 `--hot-ratio`，读 95% 集中在 10% key | p99、cache hit、MB/s | YCSB B/C 的 `requestdistribution=zipfian` |
| C06 | scan 页读 | `scan`，256 条/页，随机起点，`--scan-bytes 4K/64K/0` | page/s、kv/s、MB/s、p99 | C++ 同为 256 条/页 |
| C07 | floor / seek | 新增 `floor` workload + 1 条窗口 scan | p50/p99、kv/s | C++ 无直接对标，标为 fkv 专项 |
| C08 | 混合 50/50 | 改并发 `write-read`，50% read / 50% write，128B | 读/写各自 kv/s、p99、CPU、IOPS | 与 YCSB A、C++ `write-read` 对齐 |
| C09 | 混合 95/5 + RMW | 95% read / 5% update，另跑 workload F 式 50% RMW | 同上 + 写放大 | YCSB B/F |
| C10 | 大 value | 1MB / 10MB，`large-value` 与 `overflow` 两条路径 | MB/s、p50/p99、空间放大、segment 文件数 | 对齐 C++ `large_value_benchmark.cpp:460-640` |
| C11 | TTL / 删除回收 | 写 10M 带 TTL，等待过期，`clean_expired`，再 scan | 清理耗时、回收字节、空间放大、残留 key 数 | 两侧引擎都支持 TTL，bench 都未用 |
| C12 | reopen / 恢复 | 写满后进程退出，新进程 `reopen` + 首次读；另做 `kill -9` 恢复 | reopen_us、首次读延迟、恢复后计数/checksum | fkv 需 `--keep-root`；C++ 包 `Start` |
| C13 | 并发扫描 | `--partitions` / read lanes = 1/4/16/64 | 吞吐扩展比、p99、CPU、context switch | C++ `read_thds` / `partitions` |
| C14 | value / batch 扫描 | value 64B/1KB/16KB/256KB/1MB × batch 1/100/8192 | kv/s、MB/s、p99、空间 | 固定其它变量，单因素扫描 |
| C15 | append vs pooled | 同 C01/C03，两侧都跑 `data_append_mode=true/false` | 写放大、空间放大、p99 | Rust `AllocMode::Append/Pooled` 已按 `KvOption.data_append_mode` 接线 |

条件项：

- C15 的 Rust pooled 分支已可跑；默认与 C++ 一致是 pooled。
- C12 的崩溃恢复需要能注入 `kill -9` 的 harness，建议先用外部脚本。
- C10 的 10MB value 需要 `prlimit --memlock=unlimited`（C++ 侧注释同样要求，
  `large_value_benchmark.cpp:7`）。

运行协议：

1. 每 case 先 warmup 10% 或 30s，再开计量窗口。
2. 默认 3 次，关键 case 5 次，报告 median、min、max、CI95。
3. 冷读 case 之间 `drop_caches`，记录是否成功。
4. 固定 CPU 频率/绑核，记录 `nproc`、内核、文件系统、设备型号。
5. 两侧必须同 key/value、同 batch、同并发、同 cache 预算、同 sync 模式。
6. fkv 跨实现对比固定 `--partitions 1`，否则分区语义不同（`doc/metrics.md` §6）。
7. 输出统一 JSON，字段至少包含：`impl`、`commit`、`workload`、`phase`、`rows`、
   `kv_size`、`batch_size`、`partitions`、`threads`、`duration_s`、`ops`、`kv_s`、
   `mb_s`、`lat_*_us`、`cpu_user_s`、`cpu_sys_s`、`rss_peak_kb`、`dir_bytes`、
   `data_bytes`、`segment_bytes`、`manifest_bytes`、`space_amp`、`read_iops`、
   `write_iops`、`fsync_num`、`cache_hit`、`cache_miss`、`compact_num`、
   `compact_ms`、`reopen_ms`。

---

## 6. 优先级建议

P0（下一轮就补，能直接支撑「fkv vs C++」结论）：

1. MB/s、CPU 时间/CPU%、RSS 当前与峰值。
2. 目录总大小与 data/segment/manifest 分项、空间放大。
3. fsync 次数（fkv 内置计数）、逻辑读写次数。
4. `maintain` 接入写流水，compaction/GC 次数与耗时。
5. 并发混合读写，替代当前分阶段 `write-read`。
6. 随机写、delete-only、floor、大 value、TTL 五个缺口 workload。
7. reopen 计时与 `--keep-root`。

P1（解释性指标，建议同轮采集）：

1. 设备 IOPS/吞吐/队列深度（cgroup 或 iostat）。
2. index cache 命中率与占用、IO 缓冲池常驻。
3. `perf stat` 的 cycles/instructions/cache-miss。
4. 尾延迟窗口化、慢请求计数。
5. 写放大外部口径与 compaction 字节。

P2（需要引擎打点或更重工具）：

1. index/data/segment 按页类型字节。
2. 恢复回放条目/字节。
3. 单 IO 延迟、block 层排队。
4. fkv data cache 命中（当前无 data cache）。

---

## 7. 参考路径索引

参考套件（`/tmp/refkv`）：

- `rocksdb/tools/db_bench_tool.cc:356-2069` flags，`4837-5110` workload 名称，
  `3279-3300` 吞吐/延迟输出，`5122` `stats`。
- `leveldb/benchmarks/db_bench.cc:47-130` flags，`583-635` workload 名称，
  `318-337` `micros/op` + `MB/s`。
- `ycsb/workloads/workload_template`、`workloada`..`workloadf`。
- `wiredtiger/bench/wtperf/wtperf.c:1196-1220` CSV 字段，`198` 延迟桶，
  `wtperf_config.c:290-370` workload / truncate 校验。
- `redb/crates/redb-bench-compare/benches/int_benchmark.rs`、
  `large_values_benchmark.rs`，`redb-bench/benches/multithreaded_insert_benchmark.rs:13`。
- `sled/examples/bench.rs:8-80`。

本仓：

- `fkv/crate/perf/fkv_bench/src/cli.rs:74-385` CLI，`workload.rs:1-248` workload 与写/读口径，
  `lane.rs:115-332` 写流水，`runner.rs:171-425` 阶段编排与指标，`runner.rs:383-388` 未计时 reopen，
  `report.rs:60-73` RSS，`hist.rs:6-10, 23-55, 109-120` 延迟分位。
- `fkv/crate/engine/fkv_store/src/compact.rs:114-132, 329-336` 维护与空间判定。
- `fkv/crate/engine/fkv_store/src/state.rs:78-99` index cache 统计。
- `fkv/crate/engine/fkv_store/src/config.rs:251` 大 value 配置。
- `fkv/crate/engine/fkv_store/src/write.rs:377` `put_large`。
- `fkv/crate/engine/fkv_store/src/ttl.rs:251` `clean_expired`。
- `fkv/crate/engine/fkv_store/src/read.rs:451` `floor`。
- `fkv/crate/engine/fkv_store/src/manifest.rs:258-300` `reopen` / 回放。
- `fkv/crate/engine/fkv_store/src/config.rs` `with_alloc_mode`、`fkv_mapping/src/allocator.rs` 两种分配模式。
- `fkv/crate/io/fkv_io_compio/src/api.rs:152-166, 194-220, 264-272` IO 计数、fd 缓存、fsync 计数。
- `fkv/crate/foundation/fkv_metrics/src/lib.rs:306-317` 读写字节计数。
- `fkv/crate/engine/fkv_shard/src/dispatch.rs:246-253`、
  `fkv/crate/engine/fkv_shard/src/maintenance.rs:36-50` 写后维护。
- `eloqstore/benchmark/simple_bench.cpp:24-56` flags 与 recorder，`83-85` key 步进/删除比，
  `250-272` 写摘要，`593-620` 并发 write-read / write-scan。
- `eloqstore/benchmark/load_bench.cpp:15-19, 186-196` 批量写吞吐。
- `eloqstore/tests/large_value_benchmark.cpp:460-640` 大 value 读延迟与吞吐。
- `eloqstore/include/eloqstore_metrics.h:37-52` 10 个 gauge 与采集间隔。
- `eloqstore/include/eloq_store.h:1122-1125, 1182-1190` data cache 命中、metrics meter。
- `eloqstore/src/async_io_manager.cpp:4208-4216` 本地空间 used/limit。
- `eloqstore/src/tasks/write_task.cpp:885` 写路径内联 compaction。

---

## 8. 采集脚本：命令、产物、仍然缺什么

本节把「怎么跑、跑完能拿到什么」固化下来：`scripts/perf_local.sh`（跑两侧 +
落 JSON）与 `scripts/perf_gate.sh produce`（汇总）。目标是让后续报告**完整且可
比较**：同一份基线里同时有吞吐、延迟分位、磁盘占用、进程 RSS / CPU、多轮离散度。

### 8.1 默认 pair（与 CI 逐字节一致）

```bash
scripts/perf_local.sh \
  --cpp-bin /tmp/fkv-cpp-build/benchmark/simple_bench \
  --rust-bin /tmp/rust/target/release/fkv_bench \
  --root /tmp/fkv-perf-local \
  --out /tmp/fkv-perf-local/perf-baseline.json
```

展开成两侧的实际命令（`--root` 决定 store 路径与 ini）：

```bash
# C++（ini 由脚本写入 <root>/perf-default.ini，store_path=<root>/cpp-store）
simple_bench --kvoptions <root>/perf-default.ini --workload write-read \
  --kv_size 128 --batch_size 100 --write_batchs 10000 --partitions 1 \
  --max_key 1000000 --read_per_part 4 --read_thds 1 --test_secs 30 \
  --scan_bytes 0 --show_write_perf=true

# Rust
fkv_bench --impl rust --root <root>/rust-store --workload write-read \
  --rows 1000000 --max-key 1000000 --batch-size 100 --read-per-part 4 \
  --partitions 1 --kv-size 128 --key-interval 4 --del-ratio 4 \
  --write-batchs 10000 --scan-bytes 0 --test-secs 30 \
  --history <root>/rust-bench.json
```

参数都可由 CLI 覆盖，默认值与 `perf_gate.sh` 文档里的固定 workload 完全一致：
`--rows` `--batch-size` `--test-secs` `--write-batchs`（0＝rows/batch_size）
`--kv-size` `--read-per-part` `--partitions` `--key-interval` `--del-ratio`
`--scan-bytes` `--trials` `--rng`（`cpp` 默认 / `grid`）。

注意 `fkv_bench` 缺省 `--rng cpp`：上面的 `--key-interval 4 --del-ratio 4`
会被 C++ 兼容生成器取代，实际 history / baseline 记录
`rng=cpp, key_interval=4, del_ratio=4`（`CPP_KEY_INTERVAL` /
`CPP_DEL_RATIO` 常量）。只有 `--rng grid` 时这两个 flag 才生效
（默认 1 / 0）。`perf_local.sh` 每轮会把 produced JSON 里的实际 workload
回显到日志。冒烟用：

```bash
scripts/perf_local.sh --runs 1 --rows 20000 --test-secs 2 --sample-interval 1 \
  --cpp-bin ... --rust-bin ...
```

### 8.2 同一 store 的阶段重复：`--trials N`

`--runs N` 重复的是**整个 workload**（每轮重建 store、重写数据），衡量跨进程 /
跨轮的离散度。`--trials N`（默认 1，`PERF_TRIALS` 同义）只作用于 Rust
`fkv_bench` 的**已测量阶段**：

- 同一个 store、同一份已写入的数据集；写阶段（`write-only`、`write-read` 的写、
  `--load-only` 灌数据）**只跑一次**；
- 读 / scan 阶段在同一进程、同一 store 上重复 N 次，**逐指标取中位数**
  （`--trials > 1` 时 history / baseline 里是 N 次的中位数，不是累加值）；
- 因此它测的是「数据已就位后」的稳态读 / scan，不含建库与写放大。
  `fkv_bench --repeat N` 则是把**整个 workload（含写）**重复 N 次再取中位数，
  两者语义不同，不应混用或相加。

```bash
scripts/perf_local.sh --trials 5 ...
# 等价于给所有 Rust fkv_bench 调用追加 --trials 5
```

脚本把 `--trials` 放进 Rust 公共参数，normal / `--load-only` /
`--read-only-existing` / `--workload scan` 四类调用都会带上；是否真正重复由
`fkv_bench` 按阶段决定（写 / load 阶段仍只跑一次）。

**C++ 限制**：`simple_bench` 没有阶段级 trial flag，也没有「同一 store 只读
N 次取中位数」的模式。`TRIALS > 1` 且 `RUST_ONLY=0` 时 `perf_local.sh` 会打印
一次警告但**继续执行**（不失败）；C++ 侧只能整轮重复（用 `--runs N`）。
两边的统计口径不对称：C++ 的整轮数字不能直接与 Rust 的同 store 阶段中位数
比较，报告里必须分开描述。

### 8.2.1 从 redb 评测框架吸收的指标

redb 的 `redb-bench` 用固定阶段、`ResultType`（rate / latency / size / N/A）、
同一 store 上读 / scan 重复 3 次取中位数、compact 前后目录大小、显式 N/A
表示不支持项。fkv 侧只吸收框架形状，不照搬 redb 的绝对数字：

| 能力 | fkv 实现 |
|---|---|
| 阶段级重复取中位数 | `fkv_bench --trials N`（同一 store，写一次，读 / scan N 次） |
| 多档并发读 | `scripts/perf_scale_read.sh --readers 1,2,4,8`，输出 QPS / P99 / speedup |
| 延迟分位扩展 | `{put,get,scan,fullscan}_p95_us`（原有 p50/p90/p99/p999/p9999/max） |
| store 体积分解 | `store_bytes_data/_segment/_manifest/_other`、`store_file_num*` |
| index cache 统计 | `cache_hit_num` / `cache_miss_num` / `cache_evict_num` / `cache_bytes` / `cache_len` / `cache_capacity` / `cache_hit_ratio` |
| compact 前后体积 | `compact_store_bytes_before` / `compact_store_bytes_after` / `compact_reclaim_ratio` |
| 不支持项显式缺失 | 没有 `retain` / `extract_if` / `pop` / `len()` API 就不输出，不造 0 |

这些指标都是 Rust-only 或诊断 lane；C++ `simple_bench` 没有同名 API 时不参与
比值，`perf_gate.sh` 按 `info` 处理，只审计趋势，不做硬门槛。

### 8.2.2 redb 风格阶段矩阵

`scripts/perf_redb_matrix.sh` 用一个 Rust trait 同时驱动 C++ `simple_bench`
和 Rust `fkv_bench`，输出 redb crates.io 页面的阶段形状：

```text
bulk load / individual writes / batch writes / len()
random reads / random range reads / random reads (N threads)
removals
uncompacted size / compacted size / compact wall time
```

Rust 侧新增两个 lane：

- `delete-only`：先不计量地顺序 upsert 全部 key，再计时纯 delete；
  输出 `delete_kv_s`、`delete_batch_s`、`delete_p95_us`、`delete_mib_s` 等。
- `compact-only`：对已有 store 逐分区 `compact_if_needed`，输出
  `compact_total_us`、`store_bytes_before_compact`、
  `store_bytes_after_compact`、`compact_reclaim_ratio`。

C++ 侧首选 `cpp/bench_ffi` 共享库，Rust 运行时用 `dlopen` 调用
`fkv_bench_open/run/close`，全部 phase 在同一个矩阵进程内完成。legacy
`simple_bench` 进程适配器仍保留，通过 `cpp/patches/simple_bench_phases.patch`
增加 `--no_delete`、`--delete_only`、`--workload delete` 和
`--workload compact`。完整说明见 `doc/perf-redb-matrix.md`。

`len()` 两边都没有 O(1) count API：C++ FFI 用全表扫描计数，fkv 侧用
`--workload scan --test-secs 0` 的单轮 `fullscan_num` / `fullscan_total_us`
计出同样的扫描计数。

### 8.3 多轮离散度：`--runs N`

```bash
scripts/perf_local.sh --runs 5 --out /tmp/perf/multi.json --cpp-bin ... --rust-bin ...
```

- 每轮独立 store 目录、独立日志、独立 JSON：`<root>/<mode>/run-01/…`
  （`cpp-bench.log`、`rust-bench.json`、`resources.json`、`perf-baseline.json`）。
- `<out>` 仍是**第 1 对**的 produce 结果（保持既有读取路径可用）。
- `<out>.summary.json` 是汇总：按 workload 给出
  `put_kv_s`、`get_kv_s`、`put_p99_us`、`get_p99_us`、`rss_peak_kb`，以及
  `cpp_rss_peak_kb`（`/usr/bin/time -v` 或 `/proc` 采样）、`cpp_store_bytes`、
  `rust_store_bytes` 的 `n / min / median / max / mean / stdev / values`。
  缺失的指标不会伪造，直接不出现在 `stats` 里。
- `<out>.resources.json` 是资源聚合：`environment`（host/kernel/arch/nproc/
  page_size）+ 每轮完整 `resources` 对象。

### 8.4 对称子负载：`--symmetric`

C++ 的 `write-read` 是写线程与读线程**并发**，`fkv_bench` 的 `write-read`
是「写完 → reopen → 再读」的**分阶段**。两者不是同一形状，只看这个数字会把
并发收益与实现差异混在一起。`--symmetric` 因此跑三对独立、两侧形状一致的子负载：

| 子负载 | C++ 命令 | Rust 命令 | 说明 |
|---|---|---|---|
| write-only | `--workload write` | `--workload write-only` | 纯写吞吐 / 写延迟，两侧都无并发读 |
| read-only | 先 `--workload write` 灌数据，再 `--workload read`（只计时读） | `--load-only` 灌数据，再 `--read-only-existing`（只计时读） | 冷读：fkv 侧 reopen 后 index cache 为空 |
| write-read | `--workload write-read` | `--workload write-read` | 保留原口径，但**形状仍不对称**（见上） |

```bash
# C++ 写（灌数据）
simple_bench --kvoptions <ini> --workload write --kv_size 128 --batch_size 100 \
  --write_batchs 10000 --partitions 1 --max_key 1000000 --read_per_part 4 \
  --read_thds 1 --test_secs 30 --scan_bytes 0 --show_write_perf=true
# C++ 读（同一 store_path，只计时读阶段）
simple_bench --kvoptions <ini> --workload read  --kv_size 128 --batch_size 100 \
  --write_batchs 10000 --partitions 1 --max_key 1000000 --read_per_part 4 \
  --read_thds 1 --test_secs 30 --scan_bytes 0 --show_write_perf=true

# Rust 写
fkv_bench --impl rust --root <dir> --load-only        <写入参数>
# Rust 读（同一 <dir>，只计时读阶段）
fkv_bench --impl rust --root <dir> --read-only-existing <读取参数> --history <json>
```

**Rust 侧现状**：`fkv_bench` 已提供 `--load-only` / `--read-only-existing`
（`cli.rs:144-150`，编排见 `runner.rs:32-40` 的 `RunMode` 与
`runner.rs:238-273`），`--symmetric` 启动前的 `--help` 探测可通过，默认按
「跨进程 load → 计时读」执行；只有旧二进制缺这两个 flag 时才会在探测处
直接报错退出（不静默降级）：

```text
perf_local: symmetric 的 read-only 子负载需要 fkv_bench 支持: --load-only --read-only-existing
```

若想保留单进程近似，可显式加 `--read-only-inproc`：

```bash
scripts/perf_local.sh --symmetric --read-only-inproc ...
```

`--read-only-inproc` 让 Rust 侧用 `--workload read-only`：它在同进程里先静默
load（不计指标）、`reopen` 丢 index cache、再计时读。与「跨进程 load 一次、读一次」
的差别只在进程边界（page cache 冷热、启动开销），报告里必须注明用的是哪一种。

`symmetric` + `--runs N` 会得到 3×N 对，summary 按模式分组给统计量。

### 8.5 随机 scan 对比：`--scan`

C++ `simple_bench` 有随机 scan 负载，但默认 `write-read` 不跑 scan，所以旧
基线没有 C++ scan 行。`--scan` 跑独立的一对：

| 侧 | 命令 |
|---|---|
| C++ | 先 `--workload write` 灌数据，再 `--workload scan`（`ScanLoop`，随机起点，每页 256 条） |
| Rust | 先 `--load-only`，再 `--read-only-existing --workload scan`（`scan_page`，随机起点，每页 256 条） |

```bash
scripts/perf_local.sh --scan --root /tmp/perf-scan \
  --cpp-bin /tmp/build/simple_bench --rust-bin fkv/target/release/fkv_bench
```

`produce` 会从 C++ 的 `scan throughput` / `scan speed` 行解析
`scan_kv_s` / `scan_p99_us`，与 Rust 的 `scan_kv_s` / `scan_p99_us` /
`scan_page_s` / `scan_avg_us` 一起输出，并生成 `scan_kv_s/cpp` 比值。

### 8.6 资源与磁盘占用

`produce` 新增可选 `--resources <json>`：把侧车 JSON 原样并入 baseline 的
`"resources"` 字段（`compare` 只读已知字段，旧调用方不受影响）。`perf_local.sh`
每轮都会写一份并自动传入。

`resources.json` 结构（每轮一份）：

```json
{
  "mode": "write-read", "run": 1,
  "environment": {"hostname": "...", "kernel": "...", "arch": "aarch64",
                  "nproc": 6, "page_size": 4096, "root": "/tmp/..."},
  "cpp": {
    "command": "…完整命令行（%q 转义，可复制复现）…",
    "process": {"source": "time -v", "max_rss_kb": 0, "user_s": 0, "system_s": 0,
                "elapsed_s": 0, "cpu_percent": 0, "voluntary_ctxt_switches": 0,
                "involuntary_ctxt_switches": 0, "fs_inputs": 0, "fs_outputs": 0,
                "major_page_faults": 0, "minor_page_faults": 0},
    "store": {"path": "…", "present": true, "bytes": 0, "files": 0,
              "data_bytes": 0, "data_files": 0, "segment_bytes": 0,
              "segment_files": 0, "manifest_bytes": 0, "manifest_files": 0,
              "other_bytes": 0, "other_files": 0}
  },
  "rust": {"command": "…", "process": {…}, "store": {…}, "proc_self_io": null},
  "absent": ["device_iops", "device_throughput", "fsync_num", "…"]
}
```

采集方式与边界：

1. **store 体积**：`du -sb` 取总量；再按 `data_` / `segment_` / `manifest_`
   前缀递归 `stat` 求和并计数文件。C++ store 在进程退出后仍在，是精确值。
2. **Rust store 是采样值**：`fkv_bench` 结尾 `remove_dir_all`，退出后目录已空。
   所以脚本在 bench 运行期间用 `du -sb` 轮询（默认 `--sample-interval 5`），
   取删除前的最大值，JSON 里标 `"sampled": true` 与样本数。尺寸连续 3 次不变
   就停止采样，避免读阶段被 du 干扰。`--sample-interval 0` 关闭（则该项缺失）。
   要拿到精确值需要在 fkv 侧加 `--keep-root`（见 §4.1 第 7 条）。
3. **进程资源**：优先 `/usr/bin/time -v`（GNU time），字段映射见上；
   主机没有 GNU time 时（例如 n1）回退到 `/proc/<pid>` 采样 100ms 一次：
   `VmHWM`、`utime/stime`、`/proc/<pid>/io` 的 `rchar/wchar/syscr/syscw/
   read_bytes/write_bytes`，标 `"approximate": true`。
   这条路径**同时给了 C++ 侧的 RSS 与 CPU**（此前完全没有）。
4. **`/proc/self/io`**：只从 `fkv_bench --history` 里读（`io` 对象、
   顶层 `rchar` 等键，或 runs 里的 `proc_io_read_bytes` /
   `proc_io_write_bytes`）；fkv 尚未导出时记 `null` 并在 `absent` 里写明，
   **不臆造**。注意它与第 3 条的外部 `/proc/<pid>/io` 不是同一个口径
   （前者是 bench 自报，后者是脚本旁观），不要混用。

### 8.7 仍然缺失或只能近似采集的指标

| 指标 | 原因 | 补法 |
|---|---|---|
| `fsync_ms` / fsync 等待时间 | 两侧都没有内置耗时；C++ io_uring 路径也没有 | fkv 已有次数（`io_data_sync_num` / `io_dir_fsync_num`，见 §2.1），耗时需引擎打点；C++ 需在 `async_io_manager.cpp` 的 `io_uring_prep_fsync` 加原子计数 |
| 设备级 IOPS / 吞吐 / 队列深度 | 需要 `iostat -x` / cgroup `io.stat` / `/proc/diskstats` 前后差值，脚本只采进程级 | 在 `perf_local.sh` 的 run 前后读 `/proc/diskstats` 做差，或 CI 侧跑 `iostat -x 1` |
| 外部写放大 | 同上（设备写字节 / 逻辑写字节） | 同上；fkv history 已有 `proc_io_*`，C++ 侧仍靠脚本旁观的 `/proc/<pid>/io` 近似 |
| compaction / GC 次数与耗时 | `compact` lane 已计 `compact_run_num` / `compact_file_num` / `compact_file_s`；普通 workload 仍不触发 `maintain` | 普通 workload 见 §4.1 第 8 条 |
| cache 命中（data） | index cache 已输出 `cache_*`；fkv 无 data cache | 只有 index 命中，不能外推 data 命中 |
| `reopen_us` | Rust 已输出；C++ `Start()` 仍未包裹 | fkv 侧见 `reopen_us`；C++ 需打点 |
| MB/s（逻辑） | Rust 已输出 `{put,get,scan,fullscan}_mib_s`；C++ 仍靠 QPS 事后换算 | C++ 侧需在 `write_task` / `read_task` 加字节计数 |
| delete-only / RMW / floor / 随机写（grid）/ 大 value 的独立指标 | workload 缺口，见 §2；TTL 与 compaction 已由 `ttl-write` / `compact` lane 覆盖（Rust-only） | 见 §4.1 第 9-12 条 |
| `write-read` 的形状对称 | C++ 并发、Rust 分阶段 | 用 §8.4 的 write-only / read-only 两对标量做真正的同形状对比；并发混合读写仍需 fkv 侧改造 |

CI（`.github/workflows/perf.yml`，由协调者维护）目前仍只上传
`perf-baseline.json`；由于 `produce --resources` 已把资源并进 baseline，资源
数据会随基线一起发布，`<out>.summary.json` / `<out>.resources.json` 两个侧车
尚未上传为 artifact。

### 8.8 冒烟记录（n1 aarch64，非权威）

```bash
bash scripts/perf_local.sh --root /tmp/perf-smoke \
  --cpp-bin /tmp/fkv-cpp-build/benchmark/simple_bench \
  --rust-bin /tmp/rust/target/release/fkv_bench \
  --runs 1 --rows 20000 --test-secs 2 --sample-interval 1
```

产出（冒烟，只看流程是否走通）：

```text
write-read   put_kv_s           n=1 median=4713.6
write-read   get_kv_s           n=1 median=3593.0
write-read   put_p99_us         n=1 median=42987.0
write-read   get_p99_us         n=1 median=5577.0
write-read   rss_peak_kb        n=1 median=9728.0      # fkv VmHWM
write-read   cpp_rss_peak_kb    n=1 median=182840.0    # /proc 采样（无 GNU time）
write-read   cpp_store_bytes    n=1 median=4591616.0
write-read   rust_store_bytes   n=1 median=2469888.0   # 退出前采样
```

同样参数下 `--symmetric --read-only-inproc` 三个子负载都跑通；不带
`--read-only-inproc` 时按 §8.4 的文案直接失败（缺 `--load-only` /
`--read-only-existing`）。
