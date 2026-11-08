// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "random reads · C++ EloqStore FFI vs Rust fkv · 2026-11-08 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · 25000000 行 × 116 B ≈ 2766 MiB 数据 / 256 MiB 缓存（≈11:1，对齐 RocksDB 官方 97 GiB/6 GiB）· reads=1000000 · readers 16";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42431241655 的 perf-matrix-42431241655 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
export const PERF_MATRIX_ROWS = [
  ["random reads (16 threads)", "46887ms", "55070ms", "0.851x"],
  ["uncompacted size", "2.59 GiB", "2.59 GiB", "1.001x"],
  ["compacted size", "2.59 GiB", "2.59 GiB", "1.001x"],
  ["compact wall time", "0.003 s", "0.000 s", null]
];

export const PERF_NOTE =
  "口径对齐 RocksDB 官方评测（PERFORMANCE-BENCHMARK：97 GiB 数据库 / 6 GiB block cache ≈ 16:1），即读请求真的走盘，而不是全驻内存。每格是 random reads 阶段的墙钟耗时，最后一列 C++ / fkv 是两侧耗时之比（> 1 表示 fkv 更快）。两侧同 runner、同 workload、单分区（C++ num_threads=1），并发 16 线程（benchmark.sh 默认），key / value 用 db_bench 默认 16 B / 100 B。";

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
