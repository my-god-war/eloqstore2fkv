// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "redb 阶段矩阵 · C++ EloqStore vs Rust fkv · 2026-11-08 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · 25000000 行 × 116 B ≈ 2766 MiB 数据 / 256 MiB 缓存（≈11:1，对齐 RocksDB 官方 97 GiB/6 GiB）· readers 1/4/8/16/32 · 每阶段 1 次采样";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42444729998 的 perf-matrix-42444729998 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
// 表下三块配置：[标题, [[项, 值], ...]]
export const PERF_CONFIG = {
  "机器": [
    ["runner", "GitHub Actions ubuntu-latest (x86_64)"],
    ["页大小", "4 KiB"],
    ["日期", "2026-11-08"],
    ["来源", "CI run 42444729998"],
    ["口径", "三个引擎同机、同轮次、同一份数据形状"],
  ],
  "测试负载": [
    ["数据量", "25,000,000 行 × 116 B ≈ 2766 MiB"],
    ["缓存/引擎", "256 MiB（数据 ≈ 10.8× 缓存）"],
    ["key / value", "16 B / 100 B（db_bench 默认）"],
    ["点读", "1,000,000 次（并发 1/4/8/16/32）"],
    ["范围读", "500,000 次 × 10 条"],
    ["删除", "500,000 条"],
    ["采样", "每阶段 1 次"],
    ["分区", "单分区（EloqStore num_threads=1）"],
  ],
  "RocksDB 列": [
    ["状态", "本次未启用（--rocksdb-lib 未加载）"],
  ],
}

export const PERF_HAS_ROCKSDB = false;

export const PERF_MATRIX_ROWS = [
  ["批量加载", "968764", "222998", "—", "4.344x", null],
  ["单条写入", "2191", "1938", "—", "1.130x", null],
  ["批量写", "923688", "264524", "—", "3.492x", null],
  ["len()", "277305", "572542", "—", "0.484x", null, "bad"],
  ["随机读（1 线程）", "8147", "7391", "—", "1.102x", null],
  ["随机读（4 线程）", "21421", "21283", "—", "1.006x", null],
  ["随机读（8 线程）", "21339", "21315", "—", "1.001x", null],
  ["随机读（16 线程）", "21452", "21342", "—", "1.005x", null],
  ["随机读（32 线程）", "21493", "21434", "—", "1.003x", null],
  ["随机范围读", "6265", "4764", "—", "1.315x", null],
  ["删除", "223625", "142614", "—", "1.568x", null],
  ["未压实体积", "2.59 GiB", "2.59 GiB", "—", "1.001x", null],
  ["压实后体积", "2.59 GiB", "2.59 GiB", "—", "1.001x", null],
  ["压实耗时", "0.000 s", "0.003 s", "—", null, null]
];

export const PERF_NOTE =
  "单位：吞吐行为操作数/秒（ops/s），体积行为 MiB。`fkv / C++`、`fkv / RocksDB` 大于 1 表示 fkv 更快，加粗为更快一侧。len() 三侧都没有 O(1) 计数接口（RocksDB 用全表迭代），不能与 redb 表里 0ms 的 O(1) len() 比较。";

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
