# 性能评测指标报告（2026-11-06）

本文回答三个问题：测了哪些指标、每个指标怎么算、哪些数字可以严格对比。
数据源是 `perf-baseline` release 的 schema 2 基线，以及 2026-11-06 的 gate
运行 `42176958629`（非权威硬件，仅用于回归验证）。该次运行已包含 cpp RNG
读 key 空间修复：此前读路径把 `key_interval=4` 当成 key 步长，75% 探测落在
从未写过的 key 上，`get_p99` 被明显低估。

## 1. 评测口径

### 1.1 基线来源

| 项 | 值 |
|---|---|
| baseline | `perf-baseline.json`，schema 2，`generated_at=2026-11-06T04:25:36Z` |
| runner | `github-ubuntu-latest-x86_64` |
| 机器 | `runnervm7aw5e`，Linux 7.0.0-1017-azure，x86_64，2 vCPU |
| 页大小 | 4096 B |
| bench commit | `50c27aad25d95d1f2a023a1302608eaab5668fa2` |
| 分配 / IO | `data_append_mode=false`（Pooled），`direct_io=true` |
| C++ 进程 | `time -v`，elapsed 59.52 s |
| Rust 进程 | `time -v`，elapsed 141.66 s |

`workload` 固定为：

```text
workload=write-read  rows/max_key=1000000  batch_size=100  write_batchs=10000
read_per_part=4  partitions=1  kv_size=128  rng=cpp
key_interval=4  del_ratio=4  test_secs=30  scan_bytes=0
```

说明：`key_interval/del_ratio` 在 release 资产里曾错标为 grid 默认 `1/0`，
已按 C++ `simple_bench` 生成器的实际常量校正为 `4/4`，指标未重算。

### 1.2 当前 gate 运行

`42176958629` 使用 fkv `43ad216`，workload 与基线一致。当时
`ubuntu-latest` 给出 `sda (rotational=1)`；硬件门已不再因盘型阻断，数字按
同 runner 趋势读。

### 1.3 两侧负载形状的差异

- C++ `write-read`：`WriteLoop` 与 `ReadLoop` 两个线程同时运行。写阶段
  59.29 s，读阶段与写阶段重叠，写循环结束后读线程停止。
- Rust `write-read`：先写，再 `reopen`，再点读 30 s，再 scan 30 s，最后
  fullscan 直到 30 s 窗口结束。因此 Rust 的 `elapsed` 和资源计数包含
  scan / fullscan，C++ `write-read` 没有 scan 阶段。
- C++ 每个 `Writer` 持有完整 `[0,max_key)` 键空间；Rust 分区是全局键空间
  分片。本报告固定 `partitions=1`，规避这个差异。
- 写请求延迟：两侧都是“一个 batch（100 条）”的端到端延迟，不是单条 KV。

## 2. 指标总览

| 层 | 指标 | 是否进入硬门 | 两侧可比性 |
|---|---|---|---|
| 端到端吞吐 | `put_kv_s`, `put_batch_s`, `get_kv_s`, `scan_kv_s`, `scan_page_s`, `fullscan_kv_s` | 是（Rust） | put/get 可比；scan/fullscan 无 C++ 对应 |
| 请求延迟 | `{put,get,scan,fullscan}_{avg,p50,p90,p99,p999,p9999,max}_us` | 仅 avg/p50/p90/p99 硬门；p999/p9999/max 跳过 | put/get 可比；scan/fullscan 无 C++ 对应 |
| 秒级窗口 | `{put,get}_window_{num,qps,kv_s,p50,p90,p99}` | 否，只审计 | C++ 无同结构持久化 |
| 进程内存 | `rss_peak_kb` | 是（Rust） | 可比，但 Rust 含 scan/fullscan 峰值 |
| 存储占用 | `store_bytes` | 否，只审计 | 统计时机不同 |
| 引擎 IO 计数 | `io_fd_open_num`, `io_write_at_num`, `io_read_at_num`, `io_data_sync_num`, `io_dir_fsync_num` | 否，只审计 | C++ 无原生计数器 |
| 进程 IO | `proc_io_read_bytes`, `proc_io_write_bytes` | 否，只审计 | C++ 只能从 `time -v` 外采 |
| CPU / 调度 | `time -v` 的 user/system/CPU%/page faults/context switches | 否 | 外采，口径不同 |
| 空间分项 | data / segment / manifest / files | 否 | 前缀分类启发式 |

## 3. 吞吐指标

### 3.1 定义

| 指标 | 公式 / 含义 |
|---|---|
| `put_kv_s` | 写阶段写入条数 / 写阶段墙钟秒数 |
| `put_batch_s` | 写阶段完成批数 / 写阶段墙钟秒数；`put_kv_s = put_batch_s * batch_size` |
| `get_kv_s` | 读窗口内完成点读数 / 读窗口秒数；本次 `read_per_part=4`，即 4 条并发读链 |
| `scan_kv_s` | scan 窗口内返回条数 / scan 秒数；一次请求最多 256 条 |
| `scan_page_s` | scan 窗口内完成页请求数 / scan 秒数 |
| `fullscan_kv_s` | 顺序全表遍历返回条数 / fullscan 秒数；Rust 诊断指标 |

### 3.2 数值

| 指标 | C++ 基线 | Rust 基线 | Rust 当前 gate | Rust/C++ 基线 |
|---|---:|---:|---:|---:|
| `put_kv_s` | 16,866 | 21,376.8 | 27,323.3 | 1.27x |
| `put_batch_s` | 未持久化 | 213.77 | 273.23 | - |
| `get_kv_s` | 5,526 | 7,293.2 | 9,348.7 | 1.32x |
| `scan_kv_s` | 无 | 141,165.9 | 177,038.0 | 不可比 |
| `scan_page_s` | 无 | 551.54 | 691.67 | 不可比 |
| `fullscan_kv_s` | 无 | 108,274.4 | 166,172.3 | 不可比 |

当前 gate 相对 Rust 基线：put `+27.8%`，get `+28.2%`。
这些变化发生在另一台非权威 runner 上，只能说明“没有回归”，不能当作真实提升。

### 3.3 C++ 侧被丢弃的吞吐字段

C++ 日志还会打印 `write speed` 每秒窗口、`MiB/s`、`scan speed` 的 MB/s 和
QPS。`perf_gate.sh produce` 只保留：

- `write summary` 的 kvs/s、average、p99、p99.9；
- `read average` 的 QPS；
- 如果日志里有 scan 行，才保留 `scan throughput` 的 kvs/s 和 `scan speed` 的 p99。

`write-read` 不跑 scan，所以 C++ 基线里没有 scan/fullscan 指标。写 MiB/s 也
不是物理写带宽，而是 `kvs/s * 0.75 * kv_size`，且整数截断在乘法之前。

## 4. 延迟指标

### 4.1 定义

Rust 每个请求记一次耗时，阶段结束后算：

- `avg_us`：算术平均；
- `p50/p90/p99/p999/p9999`：直方图分位；
- `max_us`：实测最大值。

直方图 `[0, 65,536) µs` 每 µs 一个桶，之上按 2 的幂分桶，溢出桶取桶上界并
钳制到实测 max。窗口直方图线性区到 16,384 µs。亚微秒按 1 µs 计。

C++ 使用 bvar `LatencyRecorder`，写延迟按 batch 记录，读延迟按单次 Get 记录。
`write summary` 是整段累计统计；`read speed` 行的延迟分位也是进程启动以来的
累计值，最后一行接近整段结果。`read average` 的 QPS 是整段吞吐。

### 4.2 Put 延迟（每请求 = 100 条）

| 指标 | C++ 基线 | Rust 基线 | Rust 当前 |
|---|---:|---:|---:|
| avg | 8,016 | 4,673.3 | 3,655.4 |
| p50 | 2,467 | 3,363 | 2,788 |
| p90 | 33,181 | 7,070 | 6,107 |
| p99 | 51,154 | 35,070 | 20,430 |
| p99.9 | 85,962 | 223,299 | 165,315 |
| p99.99 | 106,089 | 223,299 | 165,315 |
| max | 218,909 | 223,299 | 165,315 |

C++ 的 p50 更低，但 p90 明显更重：它的分布是“多数批次很快，少数批次被读
线程 / IO 拖长”。Rust 基线 p99 更低，但 p99.9/max 更重；当前 gate 的尾部
明显收敛。注意 C++ 写阶段与读并发，Rust 写阶段没有读竞争。

### 4.3 Get 延迟

| 指标 | C++ 基线 | Rust 基线 | Rust 当前 |
|---|---:|---:|---:|
| avg | 729 | 547.8 | 427.2 |
| p50 | 156 | 121 | 126 |
| p90 | 432 | 151 | 147 |
| p99 | 31,171 | 10,034 | 4,957 |
| p99.9 | 48,206 | 355,490 | 49,828 |
| p99.99 | 78,387 | 355,490 | 49,828 |
| max | 215,814 | 355,490 | 49,828 |

C++ `get_p99_us` 在基线 JSON 里是对所有周期行取 `max`，实际接近最后一行；
这里给的是持久化值 `31,171`，最后一行日志是 `30,197`。Rust p50/p90 更好，
但 p99.9 尾部更差，说明 Rust 慢读是低频长尾，而不是整体慢。

注：上表 C++ 的 p50/p90/p99.9/p99.99/max 来自 `write summary` 和最后一行
`read speed` 日志；`perf-baseline.json` 只持久化了 `put_avg_us`、`put_p99_us`、
`put_p999_us` 和 `get_p99_us`。所以 C++ 的其余分位只能在原始日志里核对，
不能从基线 JSON 直接重算。

### 4.4 Scan / Fullscan（仅 Rust）

| 指标 | Rust 基线 | Rust 当前 |
|---|---:|---:|
| `scan_avg_us` | 7,249.8 | 5,780.9 |
| `scan_p50_us` | 1,780 | 1,627 |
| `scan_p90_us` | 27,417 | 29,753 |
| `scan_p99_us` | 131,072 | 50,641 |
| `scan_p999_us` | 421,605 | 110,450 |
| `scan_max_us` | 421,605 | 110,450 |
| `fullscan_avg_us` | 4,936,659 | 3,216,624 |
| `fullscan_p50_us` | 5,569,508 | 3,492,546 |
| `fullscan_p99_us` | 5,569,508 | 3,492,546 |

`fullscan` 每跑一遍只记一个样本，样本数很少，所以 p50/p90/p99/max 往往相等。
这些指标没有 C++ 对应物，只能看 Rust 自身趋势。

## 5. 秒级窗口指标

Rust 把请求按“距阶段启动的整秒”分桶，每个窗口独立算吞吐和分位，最后对
窗口序列取中位数。末窗口按实际长度折算。C++ 有逐秒日志，但 `produce` 不
持久化这些窗口。

| 指标 | Rust 基线 | Rust 当前 | 含义 |
|---|---:|---:|---|
| `put_window_num` | 47 | 37 | 写阶段有样本的秒数 |
| `put_window_qps` | 204 | 228 | 每秒 batch 数中位数 |
| `put_window_kv_s` | 20,400 | 22,800 | 每秒 KV 数中位数 |
| `put_window_p50_us` | 4,010 | 3,564 | 窗口 p50 的中位数 |
| `put_window_p90_us` | 5,581 | 5,098 | 窗口 p90 的中位数 |
| `put_window_p99_us` | 32,768 | 23,424 | 窗口 p99 的中位数 |
| `get_window_num` | 31 | 31 | 读窗口秒数 |
| `get_window_qps` | 7,260 | 9,316 | 每秒请求数中位数 |
| `get_window_kv_s` | 7,260 | 9,316 | 点读每请求 1 条 |
| `get_window_p50_us` | 121 | 125 | 窗口 p50 的中位数 |
| `get_window_p90_us` | 152 | 144 | 窗口 p90 的中位数 |
| `get_window_p99_us` | 8,010 | 2,237 | 窗口 p99 的中位数 |

窗口指标只审计，不参与硬门。原因是单窗口可能撞上一次 fsync 长尾，即使整段
吞吐没变也会大幅波动。

## 6. 内存、CPU 与调度

| 指标 | C++ 基线 | Rust 基线 | 说明 |
|---|---:|---:|---|
| `rss_peak_kb` / `max_rss_kb` | 179,804 | 148,980 | C++ 来自 `time -v`，Rust 来自 `VmHWM` |
| user CPU | 33.1 s | 6.51 s | Rust 包含 scan/fullscan |
| system CPU | 33.13 s | 6.12 s | 主要来自 IO 提交 |
| CPU 利用率 | 111% | 8% | C++ 两线程；Rust 异步等待占多数 |
| elapsed | 59.52 s | 141.66 s | 阶段数不同，见 1.3 |
| major page faults | 11 | 0 | 需要 IO 的缺页 |
| minor page faults | 54,438 | 183,942 | 内存映射 / 分配 |
| voluntary ctxt switches | 536,585 | 794,797 | 主动让出 |
| involuntary ctxt switches | 112,657 | 274 | 被抢占 |
| file system inputs | 2,815,776 | 6,665,912 | `time -v`，块层 |
| file system outputs | 674,808 | 642,664 | `time -v`，块层 |

C++ 的 CPU 利用率高，是因为写线程和读线程一直活跃；Rust 端大部分时间在等
O_DIRECT IO。不能把 8% 直接解读为“Rust 更省 CPU 完成同样工作”，因为 Rust
的 elapsed 包含了额外的 scan/fullscan。

## 7. 存储占用

| 指标 | C++ 基线 | Rust 基线 |
|---|---:|---:|
| `store_bytes` | 282,112,000 | 106,352,640（采样最大值） |
| data bytes | 274,558,976 | 未分项 |
| manifest bytes | 7,553,024 | 未分项 |
| segment bytes | 0 | 0 |
| files | 2 | 2 |

C++ 是进程退出后 `du -sb`；Rust 在退出前按 5 s 轮询，记录删除前的最大值。
两者统计时机不同。C++ 在 `write-read` 下保留更多历史页 / 旧版本，Rust 的
106 MB 反而小于 1M × 128B 的名义逻辑量，说明池化 / 回收在本次写模式下更紧。

## 8. 引擎 IO 与进程 IO

Rust `fkv_bench` 在收尾时从 `CompioStore` 读累计计数器：

| 指标 | Rust 基线 | Rust 当前 | 含义 |
|---|---:|---:|---|
| `io_fd_open_num` | 8 | 8 | 实际 open 次数，缓存命中不计 |
| `io_write_at_num` | 48,625 | 48,627 | 实际 `write_at` 系统调用数 |
| `io_read_at_num` | 833,236 | 975,650 | 页读尝试次数，含同路径重试 |
| `io_data_sync_num` | 10,000 | 10,000 | `sync_data` 调用次数 |
| `io_dir_fsync_num` | 8 | 8 | 目录 fsync 次数 |
| `proc_io_read_bytes` | 3,412,946,944 | 3,996,262,400 | `/proc/self/io:read_bytes` |
| `proc_io_write_bytes` | 329,027,584 | 329,027,584 | `/proc/self/io:write_bytes` |

本次 1,000,000 条写入、10,000 个 batch：

- `io_write_at_num / 10,000 = 4.86` 次写系统调用每批；
- 每条写调用平均覆盖 `1,000,000 / 48,625 = 20.6` 条 entry；
- `io_data_sync_num = 10,000`，即每批一次 `sync_data`，与 C++ 每批一次
  持久化的语义一致；
- `io_dir_fsync_num = 8` 只发生在文件 / 目录创建。

`io_read_at_num` 和 `proc_io_read_bytes` 包含点读窗口、scan、fullscan 的全部
读，不能全部归因于 Get。

C++ 没有 `write_at/read_at/sync` 原生计数器，只能从 `time -v` 的
`fs_inputs/fs_outputs` 外采，粒度更粗。这是两侧 IO 对比最大的结构缺口。

## 9. 门禁判据与本次结果

`perf_gate.sh compare` 只对 **Rust** 指标做硬门，C++ 只作为
`put_kv_s/cpp`、`get_kv_s/cpp` 两个比值门槛的分母。

| 类别 | 方向 | 阈值 | 是否硬门 |
|---|---|---|---|
| 吞吐 / 速率 `*_kv_s`, `*_batch_s`, `*_qps`, `*_page_s`, `*_file_s` | 越高越好 | 20% | 是 |
| 延迟 `*_us`（avg/p50/p90/p99） | 越低越好 | 100% | 是 |
| `rss_peak_kb` | 越低越好 | 20% | 是 |
| `*_window_*` | 只审计 | 无 | 否 |
| `store_bytes`, `io_*`, `proc_io_*`, `*_num`, `*_count` | 只审计 | 无 | 否 |
| `*_max_us`, `*_p999_us`, `*_p9999_us` | 跳过 | 无 | 否 |
| 未登记指标 | 只审计 + warning | 无 | 否 |

本次非权威 gate 结果：

- put `27,323`（+27.8%），get `9,349`（+28.2%）；
- put p99 `20,430`（-41.7%），get p99 `4,957`（-50.6%）；
- put p99.9 `165,315`（-26.0%），get p99.9 `49,828`（-86.0%）；
- RSS `145,412 kB`（-2.4%），store 不变；
- `io_read_at_num` +22.9%，`io_write_at_num` 基本不变；
- 31 个指标硬门，14 个审计，0 个未登记。

`gxhash_4096B_gib_s` 已退役：它只是 4 KiB 单块的进程内哈希微基准，不在
KV 引擎路径上，也没有 C++ `simple_bench` 对应物。旧基线里仍带这个键时，
gate 会打印 `retired metric skipped` 并跳过。
退役后的 gate 运行 `42178229451` 已通过：45 个指标，31 个硬门，14 个审计。

## 10. 没测到的指标

以下指标没有进入本次基线：

- 设备级读 / 写 IOPS、吞吐、队列深度、单 IO 延迟；
- fsync 等待时间、每次 sync 的脏文件数；
- 引擎级 `bytes_read` / `bytes_written`、按页类型拆分的 index / data 放大；
- 写放大、compaction 次数、空间放大、死字节比例；
- index / data cache 命中率、cache 淘汰次数；
- `reopen_us`、崩溃恢复时间；
- 大 value、TTL、纯 delete、RMW、floor、read-modify-write；
- 多分区、多线程、多 shard 扩展性；
- 网络 / S3 / 云端路径；
- C++ 侧 scan 指标（本次 `write-read` 不跑 scan）。

## 11. 结论

1. 指标覆盖已经比较宽：吞吐、延迟分位、秒级窗口、RSS、CPU、页错误、上下文
   切换、进程 IO、引擎 IO 计数、存储分项、哈希微基准。
2. 真正严格可比的只有 `put_kv_s`、`get_kv_s`、put/get 的整段延迟分位。
   Rust 端 scan/fullscan 没有 C++ 对应，C++ 端没有 Rust 的 IO 计数器。
3. `write-read` 两侧并发模型不同：C++ 读写同时跑，Rust 写完再读。因此
   1.27x / 1.32x 不能当作纯写 / 纯读性能结论。
4. 严格同口径的下一轮应使用 `scripts/perf_local.sh --symmetric --runs 3`
   分别比较 write-only 与 read-only，并取中位数。

## 12. 补充：c1 磁盘与并发口径

### 12.1 c1 不是 guest-visible NVMe

2026-11-06 在 c1 上只读检查：

- `/sys/block/sda` 的祖先链是 `virtio1 -> host0 -> target0:0:0 -> 0:0:0:0`，
  HBA 驱动是 `virtio_scsi`，PCI ID `1af4:1004`；
- 设备型号 `QEMU HARDDISK`，序列 `drive-scsi0`，没有 `/dev/nvme*`，
  PCI 设备里没有 NVMe class `0x010802`；
- `/tmp` 是 btrfs 子卷，挂载选项含 `compress-force=zstd:1` 和
  `discard=async`，和根、`/nix` 共用同一块 QEMU 盘。

结论：宿主机底层可能是 NVMe，但 guest 看到的是 QEMU virtio-scsi 镜像盘，
无法从 guest 证明介质，IO 路径还包含 QEMU writeback cache、宿主文件系统和
btrfs 压缩。硬件门现在只要求 Linux + 4 KiB 页，盘型只打印到日志，不再阻断；
读结果时把这一层噪声算进口径即可。

### 12.2 为什么 Rust `write-read` 不是并发

- `fkv_bench` 的 `run_pipeline` 先 `await run_write_phase(...)`，再
  `store.reopen`，最后 `run_read_phase`；
- `Store::put` 需要 `&mut self`，`Store::get` 需要 `&self`，同一 `Store`
  不能同时持有这两种引用；
- 生产 shard 也按这个模型工作：`Stores::store_mut` 在有读任务持有
  `Rc<ShardStore>` 时返回 `Busy`，写任务必须 `take()` 独占，读要等写完成；
- C++ `simple_bench` 则启动 `write_thd` 和 `read_thd` 两个线程，writer 完成
  后才 `stop_` reader。

因此混合 `write-read` 的 1.27x / 1.32x 只能作方向性参考。

### 12.3 可执行的对标方式

1. **纯写 / 纯读对标**：`scripts/perf_local.sh --symmetric` 已经分别跑
   write-only 和 read-only，两侧都用“先 load、再独立读进程”的冷缓存口径。
   这是当前最低风险、最接近 apples-to-apples 的方案。
2. **保留混合负载**：新增 `fkv_runtime::Runtime` 驱动的 writer + reader
   lane，让请求经过 shard 队列和写门，与 C++ `EloqStore` API 层对齐。需要
   新 lane 和新基线。
3. **改 C++ 为顺序模式**：给 `simple_bench` 加 write-then-read 模式，与 Rust
   顺序阶段对齐；需要处理两侧 reopen / index cache 语义。

推荐先采用方案 1 发布 C++/Rust 对比，方案 2 作为后续把混合负载也纳入
门槛的工作项。

## 13. C++ 随机 scan 与磁盘占用

### 13.1 C++ 有随机 scan，但旧基线没跑

C++ `simple_bench` 的 `ScanLoop`（`cpp/eloqstore/benchmark/simple_bench.cpp:416-545`）
本来就是随机起点、每页 256 条的范围读，并输出 `scan throughput` 与
`scan speed` 的 QPS / kvs/s / average / p50 / p90 / p99 / p99.9 / p99.99 / max。

旧基线没有 C++ scan 行，是因为固定 workload 选了 `write-read`，而 C++ 只有
`scan` / `write-scan` 才启动 `ScanLoop`；`produce` 在 `write-read` 日志里
找不到 scan 行，自然不会写 `scan_kv_s`。

现在 `scripts/perf_local.sh --scan` 会跑一对独立随机 scan：

- C++：先 `--workload write` 灌数据，再 `--workload scan`；
- Rust：先 `--load-only`，再 `--read-only-existing --workload scan`。

`produce` 输出 `scan_kv_s`、`scan_p99_us` 以及 `scan_kv_s/cpp`。n1 上的
小规模 smoke 已跑通，但由于 n1 是虚拟盘、二进制也较旧，只验证流程，不作为
权威数字。

### 13.2 为什么 Rust 磁盘占用更小

同一次基线：

- 名义数据量：`1M × 128B = 128MB`
- C++：`282,112,000` 字节（data 274.6MB + manifest 7.5MB）
- Rust：`106,352,640` 字节（退出前采样最大值）

这不是“Rust 把 128MB 压成了 106MB”，而是三个因素叠加：

1. **存活 key 少于 1M**：`rng=cpp` 有 25% delete，且随机游走会覆盖写，
   物理上只需要容纳存活版本。Rust `106MB` 对应的是这个较小的存活集合。
2. **C++ 保留了更多历史页**：`simple_bench` 不调用 `FileGc` / `Compact`，
   C++ 的旧页版本留在 data 文件里，所以 274.6MB 是含历史页的物理占用。
   Rust pooled 分配器在覆盖 / 删除后回收页，bench 结束时也没有 GC 队列，
   因此更接近存活集合。
3. **统计时机不同**：C++ 是进程退出后 `du`；Rust 是 bench 退出前按 5s
   轮询，记录删除目录前的最大值。manifest 大小也不同（C++ 7.5MB，Rust
   合并进 2 个文件）。

因此当前 `space_amp_nominal = store_bytes / 128MB` 只能说明“物理占用 / 名义
数据量”，**不是**“物理占用 / 存活数据量”。公平的空间放大需要：

- 两侧都统计存活 key 的 key+value 字节；
- 两侧都在相同维护状态测量（都不跑 GC/compact，或都跑一次）；
- C++ 侧额外拆分 data / segment / manifest，Rust 侧拆分 data / manifest。

redb 的 `uncompacted size` / `compacted size` 正是这个思路；fkv 的 `compact`
lane 可以给出 compact 后大小，C++ `simple_bench` 目前没有对应调用。
