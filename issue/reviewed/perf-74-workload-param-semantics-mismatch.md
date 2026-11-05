# perf-74 workload 参数口径不一致：kv_size 语义、key 编码、batch 默认、无 delete/稀疏写

## 位置
- Rust: `crate/perf/fkv_bench/src/main.rs:93-124`（参数默认值）、`main.rs:292`（key 编码）、`main.rs:305-308`（写模式）、`main.rs:237-251`（Workload 枚举）
- C++: `benchmark/simple_bench.cpp:24-35`（flag 默认值）、`simple_bench.cpp:83-85`（key_interval/del_ratio）、`simple_bench.cpp:111-127`（稀疏写 + delete）、`simple_bench.cpp:23`（workload 枚举）

## 现状
fkv_bench 参数：
- `kv-size` 默认 64（`main.rs:98`，实际是 value 字节），`batch-size` 默认 100（`main.rs:103`），`partitions` 默认 1（`main.rs:123`），`rows` 默认 20_000（`main.rs:93`）。
- key 编码 `format!("key:{index:010}")`（`main.rs:292`），15 字节 ASCII。
- 写模式：纯 upsert，连续 key，无 delete（`main.rs:305-308` `BatchEntry::upsert`）。
- workload 枚举只有 `write-read`/`write-only`/`read-only`（`main.rs:238-242`），无 `write-scan`/`load`/`scan`。

## C++ 实现 / 目标口径
C++ `simple_bench`：
- `kv_size` 默认 128（`simple_bench.cpp:24`），value = `kv_size - sizeof(uint64_t)`（`simple_bench.cpp:113`），即 kv_size 含 8 字节 key。
- `batch_size` 默认 8192（`simple_bench.cpp:25`），README 用 20000。
- `partitions` 默认 128（`simple_bench.cpp:27`），README 用 1。
- key 为 8 字节大端 uint64（`simple_bench.cpp:71-74`）。
- 写：key 稀疏递增 `key_interval=4`（`simple_bench.cpp:83`、`111`），25% delete（`del_ratio=4`、`simple_bench.cpp:114`），75% upsert。
- workload：`write/read/scan/write-read/write-scan/load`（`simple_bench.cpp:23`）。

## 差异与风险
- **kv_size 语义错位**：fkv 的 `kv-size` 只算 value，C++ 含 key；同样参数下 value 大小不同，影响 page 密度与 IO 量。
- **key 编码不同**：15 字节 ASCII vs 8 字节大端，key 长度影响 hash 分布、B+tree 叶容量、page 装填率。
- **batch_size 默认差 80 倍**（100 vs 8192），写放大与 manifest 记录数不可比。
- **无 delete / 无稀疏写**：C++ 写路径 25% delete + 稀疏 key 触发 COW/tombstone/TTL 清理，fkv 纯 upsert 连续 key 走最乐观路径，写吞吐口径偏易。
- **workload 缺 `write-scan`/`scan`/`load`**：验收 scan 吞吐 ≥ 0.9× 需要独立 scan workload，当前只能从 write-read 顺带测。

## 对齐建议
- `kv-size` 语义改为含 key（value = kv_size - 8），或新增 `--value-size` 与 `--kv-size` 区分并文档化。
- key 编码改为 8 字节大端 uint64，对齐 `EncodeFixed64(ToBigEndian(key))`。
- `batch_size` 默认对齐 C++（8192 或 README 的 20000）。
- 写模式加入 `del_ratio`/`key_interval`：按 C++ 比例生成 delete 与稀疏 key。
- workload 枚举补 `write-scan`/`scan`/`load`。

## 性能优化建议
- 稀疏 key + delete 才能暴露 COW 合并、tombstone、TTL 清理的真实写成本，是写吞吐 ≥ 0.85× 的合理负载。
- key 用固定 8 字节可让 `key_of` 走栈上数组，避免每请求堆分配。

## 严重度
P1（口径不一致/功能缺失）

## 审查结论（2026-11-04）

**部分成立，已按成立部分落地**。原文两条已过期：

- 「key 编码 15 字节 ASCII」不成立：现为 `key_bytes()` = 8 字节大端 uint64
  （`KEY_BYTES`，注释即对齐 `EncodeFixed64(ToBigEndian(key))`，有单测）。
- 「kv_size 语义错位」保留为**文档差异**：`--kv-size` 是 value 字节数，已把
  help 文案改成「value 字节数（C++ simple_bench 的 kv_size 含 8B key）」。

成立且已实现（`crate/perf/fkv_bench/src/main.rs`）：

1. **稀疏 key**：新增 `--key-interval <N>`（默认 1＝历史行为），第 `index` 个 key
   写成 `index * key_interval`，对齐 C++ `key_interval`（C++ 用 4）。
2. **delete 比例**：新增 `--del-ratio <N>`（默认 0＝不删）。delete 删的是
   `del_ratio - 1` 步之前**已写过**的 key（`delete_target`），写路径真的走到既有项的
   tombstone 覆盖，而不是删一个不存在的 key；`del_ratio < 2` 视为不删。
3. **workload 补齐**：新增 `scan` 与 `write-scan`（C++ 有 `scan`/`write-scan`）。
   `scan` 只计量扫描（写入静默作数据准备），`write-scan` 计量写+扫描；
   `Workload::measures_write/read/scan` 三个谓词取代原来的 `!= WriteOnly/ReadOnly` 判断。
4. **参数进 history**：`PerfData` 增 `key_interval` / `del_ratio`（`#[serde(default)]`），
   每行 run 自带口径，旧 `perf.json` 兼容。
5. **正确性对齐**：读路径对 delete 位断言 `get == None`，scan 断言条数 == `live_key_num`
   （按 `is_live` 数出来，和写侧判定同一套口径）；
   `scan_kv_s` 用实际扫描条数而非 `rows`。

## DONE（2026-11-04）

验证：
- `cargo nextest run -p fkv_bench` 16 passed（新增 `scan_only_workload_measures_scan_not_write`、
  `sparse_keys_and_deletes_stay_consistent`、`delete_targets_earlier_keys`）；
- `cargo clippy -p fkv_bench --all-targets --no-deps -- -D warnings` 零警告；
- 端到端：`cargo run -p fkv_bench -- --max-key 2000 --rows 800 --batch-size 64 --kv-size 128
  --key-interval 4 --del-ratio 4 --workload write-scan --partitions 1 ...`
  跑通并写出 `put_kv_s/put_batch_s/scan_kv_s/scan_page_s`，JSON 记录
  `key_interval=4, del_ratio=4, workload=write-scan`。

未对齐（有意保留）：

- `batch-size` 默认仍是 100（C++ 8192 / README 20000）。改默认会让历史 `perf.json` 曲线
  断点，故只在 help/本文件记差异，需要用 C++ 口径时显式传参。
- 多分区时 delete 目标可能落在别的分区（`partitions=1` 时才与 C++ 严格等价），
  已在 `WriteMix` 文档里写明。
- `load` / `read` / `write` 这组 C++ workload 名未加别名（fkv 用
  `write-only`/`read-only`，`as_str` 记录的是 fkv 名）。

## 复审（2026-11-05 r6）

通过。

- `fkv_bench/src/main.rs:191/315 key_interval`、`:193/320 del_ratio`、
  `PerfData` 字段（`:347-348`）进 history；`WriteMix`（`:510-519`）文档写明
  key 稀疏与 delete 目标语义。
- `Workload` 谓词 `measures_write/read/scan`（`:492-505`），`write-scan`（`:466`）
  与 `scan` workload 补齐。
- 读断言对 delete 位 `get == None`、scan 断言 live 条数（`is_live/is_deleted`
  导出于 `:863`）。
- 测试 `:1025 scan_only_workload_measures_scan_not_write`、`:1046
  sparse_keys_and_deletes_stay_consistent`、`:1072 delete_targets_earlier_keys`。
- batch-size 默认 100 保持 + 差异记录在案，属有意偏离，无异议。

## 更正（2026-11-05，r8e-02 / r9e-03）

原「`--key-interval 4 --del-ratio 4` 即对齐 C++」不成立：Rust 是均匀网格
（`i × interval`），C++ `simple_bench` 是逐条随机步进 `rand() % interval + 1`。
该差异已登记在 `COMPAT_BASELINE`（key-interval 条目）与 `doc/metrics.md` §6.1；
跨实现比值仅在 `key_interval = 1` 时严格可比。跟踪：
`issue/done/r8e-02-bench-key-interval-stride-diverges-from-cpp.md`。
