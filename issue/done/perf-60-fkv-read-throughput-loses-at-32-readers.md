# perf-60 fkv 点读在 16/32 读者档位落后 RocksDB

## 现象

CI run 42484382517（25,000,000 行 × 116 B / 每引擎 256 MiB 缓存 / 三方都逐批
fsync）的 fkv_compare 矩阵，点读吞吐（ops/s）：

| readers | fkv | RocksDB | fkv / RocksDB |
|---:|---:|---:|---:|
| 1 | 288 897 | 157 987 | 1.83x |
| 4 | 489 399 | 442 905 | 1.11x |
| 8 | 468 534 | 439 958 | 1.07x |
| 16 | 417 817 | 442 004 | 0.95x |
| 32 | 352 760 | 453 565 | 0.78x |

RocksDB 在 4 线程后基本饱和（439~453 k），fkv 反而随读者数下降。

## 证据

- `crate/perf/fkv_compare/src/main.rs` 的 `Phase::RandomReads`：
  `--readers N` 时开 N 个 OS 线程，**每个线程各开一个 `Store`**，页缓存按读者
  平分（`cfg.cache_bytes / readers`）。32 读者 = 每份缓存 8 MiB，热页互相踢出；
  4 vCPU 上还叠加 32 线程超订。
- `examples/threaded_read.rs`（同一份 25M 行数据）：1 线程 51 k、2 线程 108 k、
  4 线程 152 k、8 线程 212 k —— 引擎随线程涨，但缓存平分后单线程吞吐同步下降。
- `--fkv-driver runtime`（多 shard、共享一份缓存）本机 macOS 上点读只有
  50~109 k：请求队列 + 跨线程唤醒约 20 µs/次，本机不可用；Linux 待测。
- 单 Store 上「N 条异步链交错」模式（`--no-fkv-read-threads`）1 线程 289 k，
  但并发靠流水线深度而不是线程，16/32 档位也不涨。

## 建议

1. 让「缓存不被读者切碎」和「真并行」同时成立：
   - 优先试：`Store` 只保留一份缓存，读者线程各自持有 `Arc` 共享同一页缓存
     （若 `Store`/`PageCache` 可改成 `Send + Sync`）；或
   - 退路：读者数超过 shard 数时，把 shard 数固定为 vCPU 数、客户端线程做
     多路 in-flight（每线程窗口 8~32），让 io_uring 批量提交，而不是 1 线程 1 IO。
2. 在 `fkv_compare` 增加 `--read-window <k>`：每读者线程保持 k 个在飞请求，
   用 `futures::stream::FuturesUnordered` 限流；测出 k 与 readers 的最优组合。
3. 验收：25M 行 / 256 MiB / fsync 对齐口径下，readers=16 与 32 的
   `fkv / RocksDB` 都 ≥ 1.10x，且 readers=1 不退化（≥ 1.5x）。

## 验收方式

CI `perf.yml` 跑一轮 25M 行矩阵，读取 artifact 的
`fkv-compare.json`，逐档位核对上表。

## 结果（已完成）

CI run 42503234970（rounds=3 中位数，25M 行 / 256 MiB / 三方 fsync）：

| readers | fkv | RocksDB | fkv / RocksDB |
|---:|---:|---:|---:|
| 1 | 310 431 | 194 425 | 1.60x |
| 4 | 644 323 | 556 307 | 1.16x |
| 8 | 665 875 | 617 100 | 1.08x |
| 16 | 659 859 | 590 400 | 1.12x |
| 32 | 662 036 | 579 421 | 1.14x |

做法：线程数 = min(readers, 4)、缓存总预算固定按线程数平分、每线程
`FuturesUnordered` 维持 16 个在飞请求（`--read-window` / `--read-max-concurrency`）。
同时修掉了「每轮不清数据目录导致第 2、3 轮磁盘吃满、读阶段塌陷」的度量错误。
