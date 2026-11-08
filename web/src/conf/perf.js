// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "redb 阶段矩阵 · C++ EloqStore vs Rust fkv · 2026-11-08 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · 25000000 行 × 116 B ≈ 2766 MiB 数据 / 256 MiB 缓存（≈11:1，对齐 RocksDB 官方 97 GiB/6 GiB）· readers 1/4/8/16/32 · 每阶段 1 次采样";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42444729998 的 perf-matrix-42444729998 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
export const PERF_MATRIX_ROWS = [
  ["批量加载", "112109ms", "25806ms", "4.344x"],
  ["单条写入", "516ms", "456ms", "1.130x"],
  ["批量写", "378ms", "108ms", "3.492x"],
  ["len()", "43665ms", "90153ms", "0.484x", "bad"],
  ["随机读（1 线程）", "135293ms", "122745ms", "1.102x"],
  ["随机读（4 线程）", "46987ms", "46684ms", "1.006x"],
  ["随机读（8 线程）", "46915ms", "46863ms", "1.001x"],
  ["随机读（16 线程）", "46857ms", "46615ms", "1.005x"],
  ["随机读（32 线程）", "46655ms", "46527ms", "1.003x"],
  ["随机范围读", "104963ms", "79813ms", "1.315x"],
  ["删除", "3506ms", "2236ms", "1.568x"],
  ["未压实体积", "2.59 GiB", "2.59 GiB", "1.001x"],
  ["压实后体积", "2.59 GiB", "2.59 GiB", "1.001x"],
  ["压实耗时", "0.003 s", "0.000 s", null]
];

export const PERF_NOTE =
  "口径对齐 RocksDB 官方评测（97 GiB 数据库 / 6 GiB block cache ≈ 16:1）：数据约 11 倍于缓存，读请求真的走盘。每格是该阶段的墙钟耗时，最后一列 C++ / fkv = 两侧耗时之比（> 1 表示 fkv 更快）；体积行是 fkv / cpp 字节比。两侧同 runner、同 workload、单分区（C++ num_threads=1），key / value 用 db_bench 默认 16 B / 100 B。len() 两侧都没有 O(1) 接口，都是全表扫描计数，不能与 redb 表里 0ms 的 O(1) len() 比较。";

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
