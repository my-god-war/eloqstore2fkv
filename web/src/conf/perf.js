// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "redb 风格阶段矩阵 · C++ EloqStore FFI vs Rust fkv · 2026-11-06 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · rows=200000 · batch_size=1000 · kv_size=128 B · readers 1/4/8/16/32 · read_secs=5";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42312580576 的 perf-matrix-42312580576 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
export const PERF_MATRIX_ROWS = [
  ["bulk load", "241.73 kops/s", "429.20 kops/s, p99 4567.0 µs", "1.776x"],
  ["individual writes", "2.10 kops/s", "357.2 ops/s, p99 7150.0 µs", "0.170x", "bad"],
  ["batch writes", "306.79 kops/s", "447.91 kops/s, p99 4646.0 µs", "1.460x"],
  ["random reads (1 thread)", "6.40 kops/s, p99 292.0 µs", "6.20 kops/s, p99 435.0 µs", "0.969x"],
  ["random reads (4 threads)", "19.28 kops/s, p99 473.1 µs", "19.64 kops/s, p99 380.0 µs", "1.019x"],
  ["random reads (8 threads)", "19.49 kops/s, p99 2324.0 µs", "19.65 kops/s, p99 1589.0 µs", "1.008x"],
  ["random reads (16 threads)", "19.53 kops/s, p99 31617.2 µs", "19.71 kops/s, p99 32421.0 µs", "1.009x"],
  ["random reads (32 threads)", "19.56 kops/s, p99 36661.3 µs", "19.78 kops/s, p99 38827.0 µs", "1.011x"],
  ["random range reads", "3.33 kops/s, p99 507.4 µs", "173.78 kops/s, p99 3905.0 µs", "52.180x"],
  ["removals", "201.01 kops/s", "173.75 kops/s, p99 9561.0 µs", "0.864x"],
  ["len()", "418.43 kops/s", "202.75 kops/s", "0.485x", "bad"],
  ["uncompacted size", "25.26 MiB", "25.26 MiB", "1.000x"],
  ["compacted size", "25.26 MiB", "25.26 MiB", "1.000x"],
  ["compact wall time", "0.000 s", "0.000 s", null]
];

export const PERF_NOTE =
  "同一 runner、同一 workload；ratio = fkv / cpp，大于 1 表示 fkv 更快；size ratio = fkv / cpp 字节。C++ 侧为同进程 FFI，fkv 侧为 fkv_bench。len() 两侧都是全表扫描计数（非 O(1)）；random range reads 两侧 scan 粒度不同；compact 两侧都是近 no-op，实测都显示 0.000 s。";

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
