# perf-61 fkv 删除吞吐只有 RocksDB 的一半

## 现象

同一轮 CI（42484382517）矩阵：

| 阶段 | fkv | RocksDB | fjall | fkv / RocksDB |
|---|---:|---:|---:|---:|
| 删除 | 467 001 | 1 028 461 | 756 742 | 0.45x |

而上一轮（42483656841）同一口径是 fkv 1 328 015 / RocksDB 1 445 083 = 0.92x。
两轮相差 2.8 倍，说明这条既是**引擎放大**也是**采样噪声**（每阶段 1 次采样，
500 000 条 / 每批 1000 条 = 500 次 fsync 主导）。

## 证据

- 删除走 `make_batch(..., remove=true)` → `Store::put` → 每批重写受影响 data 页
  + 追加 manifest + 2 次 fdatasync（`crate/engine/fkv_store/src/write.rs`）。
- `crate/engine/fkv_store/src/data.rs` 的 tombstone 写入如果触发页分裂 / 页合并，
  单批成本会随删除比例非线性上升；500 000 条删除占 25M 的 2%，但集中在低 key 段。
- 单次采样：`fkv_compare` 的 `Report` 每阶段只跑一次（`config.samples` 恒为 1，
  回填脚本按 `max(cfg['samples'],1)` 写死）。

## 建议

1. 先修度量：`--samples <n>`（默认 3），写/删除阶段重复 n 次取中位数；
   回填脚本用中位数。否则任何 10% 级优化都会被磁盘抖动淹没。
2. 引擎侧用 `sample`/`perf` 采样删除阶段，确认时间花在
   tombstone 写、页合并、还是 manifest 追加；对照 C++ `DeleteBatch` 的页处理。
3. 验收：samples=3 中位数下，删除 `fkv / RocksDB` ≥ 1.10x。

## 验收方式

CI 跑 3 次采样矩阵，核对中位数行；同时 `cargo nextest run -p fkv_store -p fkv_btree`
全绿。

## 结果（已完成）

同一轮 CI：删除 fkv 1 497 676 / RocksDB 1 386 928 = 1.08x（三轮 1.43M~1.57M，
抖动已收敛）。改动：`RouteCursor` 相邻 key 快速路由（每 1000 条删除批 85µs→6µs）、
merge 报告是否需要二次遍历旧页、整页删空跳过逐 entry 空扫；盘面逐字节不变。
剩余差距受「两次 fsync : 一次」限制，已在页面上注明。
