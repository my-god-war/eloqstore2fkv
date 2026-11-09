# perf-63 批量加载 / 批量写 落后 RocksDB（0.89~0.91x）

## 现象

CI run 42498653970（25M 行 / 256 MiB / 三方逐批 fsync，单轮采样）：

| 阶段 | fkv | RocksDB | fkv / RocksDB |
|---|---:|---:|---:|
| 批量加载 | 1 008 297 | 1 112 227 | 0.91x |
| 批量写 | 1 063 255 | 1 197 097 | 0.89x |
| 单条写入 | 1 825 | 6 390 | 0.29x |

上一轮（42484382517）同一口径批量加载是 1.52x、批量写 1.03x —— 跨轮抖动大（不同
runner 的 fsync 延迟不同），但两轮都显示「写路径 CPU 占比不低」。

## 证据

- 写批固定 2 次 fdatasync（data + manifest），RocksDB 1 次；批量 1000 条时同步
  成本被摊薄，剩下的是页构建 / 路由 / 页写回。
- 删除侧已有的快速路由（`RouteCursor` 相邻 key 缓存）目前只用于 delete 路径；
  升序批量 upsert 同样每 key 从 root 下降一次。
- `WriteTask::flush` 里每批 `sync_data` 之前要等所有页写回；页写回是逐页 `pwrite`
  串行提交（没有批量提交 / 合并的相邻页）。

## 建议

1. 把 `RouteCursor` 的相邻 key 快速路由复用到 upsert 路径（批量加载的 key 严格
   升序，收益与删除侧同量级）。
2. 相邻脏页合并成一次 pwrite（同一文件、连续 FilePageId）：批量加载每 1000 条会
   产生多个连续新页，合并能显著减少 syscall 数。
3. 验收：3 轮中位数下，批量加载与批量写 `fkv / RocksDB` ≥ 1.10x。

## 验收方式

CI `perf.yml`（rounds=3）artifact 的 `fkv-compare.json`。
