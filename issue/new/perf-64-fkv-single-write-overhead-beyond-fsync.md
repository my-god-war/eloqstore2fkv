# perf-64 单条写入除了两次 fsync 之外的额外开销

## 现象

同一轮 CI：fkv 1 825 ops/s（0.55 ms/op）、RocksDB 6 390 ops/s（0.156 ms/op）。
fkv 与 C++ 都是两次同步，理应是 RocksDB 的 ~1/2；实测是 1/3.5，说明除同步之外
还有 ~0.25 ms/op 的额外开销。

## 证据（待补）

- 单条写每次都要：分配新页、构建 data 页（1 条 entry）、写回、`sync_data`、
  追加 manifest 记录、manifest fsync。
- C++ EloqStore 在同口径下 1068 ops/s（eloqstore2fkv run 42444729998），
  fkv 1280 —— 对 C++ 不输，说明额外开销大概率来自「每批固定成本」而不是数据量。
- 需要一次带符号采样确认：单条写 1000 次循环里，`sample` 落在
  `append_mapping_log` / `write_page` / `sync` 的占比。

## 建议

1. 采样定位（macOS `sample` / Linux `perf`），确认额外开销的具体函数。
2. 可能的优化点：manifest 记录只写增量映射（当前是否重复写 branch 元数据 / 全量
   区间？）；单条写页分配是否每次都 `fallocate`；data 页写回是否经过页缓存二次拷贝。
3. 验收：3 轮中位数下 `fkv / RocksDB` 从 0.29x 提到 ≥ 0.5x（同步次数的物理上限），
   并对 C++ 保持 ≥ 1.2x。

## 验收方式

CI 3 轮中位数 + `delete_bench` 式的单条写复现程序（可放
`crate/engine/fkv_store/examples/`）。
