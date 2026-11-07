// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "redb 风格阶段矩阵 · C++ EloqStore FFI vs Rust fkv · 2026-11-07 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · rows=1000000 · batch_size=1000 · kv_size=174 B · readers 1/4/8/16/32 · read_secs=0";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42397058871 的 perf-matrix-42397058871 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
export const PERF_MATRIX_ROWS = [
  ["bulk load", "206.95 kops/s", "859.83 kops/s, p99 3021.0 µs", "4.155x"],
  ["individual writes", "1.35 kops/s", "2.59 kops/s, p99 600.0 µs", "1.919x"],
  ["batch writes", "112.57 kops/s", "881.42 kops/s, p99 2301.0 µs", "7.830x"],
  ["random reads (1 thread)", "450.33 kops/s, p99 3.0 µs", "636.93 kops/s, p99 2.0 µs", "1.414x"],
  ["random reads (4 threads)", "378.48 kops/s, p99 20.6 µs", "660.27 kops/s, p99 287.0 µs", "1.745x"],
  ["random reads (8 threads)", "374.71 kops/s, p99 38.2 µs", "665.25 kops/s, p99 665.0 µs", "1.775x"],
  ["random reads (16 threads)", "384.09 kops/s, p99 74.0 µs", "663.43 kops/s, p99 1434.0 µs", "1.727x"],
  ["random reads (32 threads)", "375.94 kops/s, p99 160.7 µs", "666.43 kops/s, p99 2951.0 µs", "1.773x"],
  ["random range reads", "4.96 kops/s, p99 324.6 µs", "123.88 kops/s, p99 15.0 µs", "24.974x"],
  ["removals", "1.55 Mops/s", "1.78 Mops/s, p99 1111.0 µs", "1.148x"],
  ["len()", "579.55 kops/s", "28.82 Mops/s", "49.720x"],
  ["uncompacted size", "787.57 MiB", "787.57 MiB", "1.000x"],
  ["compacted size", "787.57 MiB", "787.57 MiB", "1.000x"],
  ["compact wall time", "0.001 s", "0.000 s", null]
];

export const PERF_NOTE =
  "同一 runner、同一 workload；ratio = fkv / cpp，大于 1 表示 fkv 更快；size ratio = fkv / cpp 字节。C++ 侧为同进程 FFI，fkv 侧为 fkv_bench，两侧都是单分区（C++ num_threads=1），读并发由客户端读线程数提供。缓存两侧各 1 GiB，大于 1M × 174 B ≈ 166 MiB 的数据集，读阶段全驻内存，与 redb（4 GiB 缓存 / 870 MB 数据）同口径。len() 两侧都没有 O(1) 接口：C++ 分页 scan 每次物化 256 条再计数，fkv 用 scan_count 只计数不物化，因此这一行是「数键速率」而非 redb 的 O(1) 查表速率；random range reads 两侧都是每次 10 条 / 1500 B 窗口；compact 两侧都是近 no-op，实测都显示 0.000 s。";

export const VERIFY_ROWS = [
  ["格式语料", "22 / 22"],
  ["engine-cross", "通过"],
  ["崩溃恢复", "已验证"]
];

export const DOC_LINKS = [
  ["README", "项目说明"],
  ["performance", "性能口径"],
  ["workflow", "验证流程"],
  ["issues", "审查记录"]
];
