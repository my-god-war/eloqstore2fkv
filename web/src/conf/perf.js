// 首页性能矩阵数据：redb crates.io 页面阶段形状，C++ EloqStore FFI vs Rust fkv。
// 真实数字由 scripts/perf_redb_matrix.sh 生成后回填；空值在页面上显示为 —。
export const PERF_META =
  "redb 阶段矩阵 · C++ EloqStore vs Rust fkv · 2026-11-08 · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · 25000000 行 × 116 B ≈ 2766 MiB 数据 / 256 MiB 缓存（≈11:1，对齐 RocksDB 官方 97 GiB/6 GiB）· readers 1/4/8/16/32 · 每阶段 1 次采样";

// [phase, cpp, fkv, fkv/cpp, tone]
// 数据源：CI 运行 42474412476 的 perf-matrix-42474412476 artifact（fkv-matrix.md / .json）。
// tone: "bad" 表示 fkv 明确更差；规则固定为吞吐 ratio < 0.5（size / compact 行不判 tone）。
// 缺省表示 fkv 更好或中性。
// 表下三块配置：[标题, [[项, 值], ...]]
export const PERF_CONFIG = {
  "机器": [
    ["runner", "GitHub Actions ubuntu-latest (x86_64)"],
    ["页大小", "4 KiB"],
    ["日期", "2026-11-08"],
    ["来源", "CI run 42474412476"],
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
  "持久化 / fsync": [
    ["fkv", "每批 1×数据 + 1×manifest fdatasync（无关闭开关）"],
    ["EloqStore", "每批 SyncData + AppendManifest（KvOptions 无开关）"],
    ["RocksDB", "WAL 开、WriteOptions.sync=false（默认，不逐次 fsync）"],
    ["影响", "写阶段两侧多付同步，读阶段不受影响"],
  ],
  "RocksDB 列": [
    ["版本", "CI 系统包 librocksdb-dev"],
    ["block cache", "256 MiB LRU（与两侧缓存同额）"],
    ["block size", "4 KiB"],
    ["index/filter", "cache_index_and_filter_blocks=true（同池）"],
    ["压缩", "kNoCompression（三侧都不压缩）"],
    ["持久化", "WAL 开、WriteOptions.sync=false（RocksDB 默认）"],
    ["memtable", "write_buffer_size 64 MiB"],
    ["注意", "EloqStore 与 fkv 每批固定 fsync，写阶段多付同步"],
  ],
};

export const PERF_HAS_ROCKSDB = true;

export const PERF_MATRIX_ROWS = [
  ["批量加载", "741708", "200974", "1283936", "3.691x", "0.578x"],
  ["单条写入", "1280", "1068", "572365", "1.198x", "0.002x"],
  ["批量写", "722098", "170103", "2166566", "4.245x", "0.333x"],
  ["len()", "210430", "479264", "1877739", "0.439x", "0.112x", "bad"],
  ["随机读（1 线程）", "5623", "5130", "178336", "1.096x", "0.032x"],
  ["随机读（4 线程）", "21563", "21527", "447819", "1.002x", "0.048x"],
  ["随机读（8 线程）", "21594", "21553", "65763", "1.002x", "0.328x"],
  ["随机读（16 线程）", "21611", "21583", "52863", "1.001x", "0.409x"],
  ["随机读（32 线程）", "21600", "21485", "47483", "1.005x", "0.455x"],
  ["随机范围读", "4319", "3825", "6088", "1.129x", "0.709x"],
  ["删除", "180817", "147449", "1516810", "1.226x", "0.119x"],
  ["未压实体积", "2.59 GiB", "2.59 GiB", "2.69 GiB", "1.001x", "0.962x"],
  ["压实后体积", "2.59 GiB", "2.59 GiB", "2.67 GiB", "1.001x", "0.970x"],
  ["压实耗时", "0.000 s", "0.002 s", "0.394 s", null, null]
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
