# sem-75 COMPAT_BASELINE「INI 扩展键清单补全（sem-69）」不完整：9 个 fkv INI 扩展键从未登记

- 领域：语义 / 配置面（`fkv_config` 全键 × C++ `KvOption::LoadFromIni` 的 INI 键清单复核）
- 优先级：P3
- 置信度：高（宏声明点为唯一事实源，逐键比对 baseline 全文；9 键在 baseline 中零命中）

## 现象

`fkv_config` 用 `kv_options!` 宏把「字段 + 类型 + 默认值 + INI 节 / 解析器」单源
声明，`[Ext]` 标记表示该键在 Rust 的 `[run]` / `[permanent]` **两节均可**配置、
且 C++ `KvOption::LoadFromIni` **不读取**（C++ 只能代码 / C API 设置，或根本无
同名字段）。宏里 `[Ext]` 键共 **19 个**：

```
crate/foundation/fkv_config/src/lib.rs（kv_options! 展开体，283-406 行）
page_cache_bytes, segment_file_amplify_factor, segment_compact_yield_every,
chunk_size, segment_size, gc_global_mem_size_per_shard, pinned_tail_scratch_slots,
segments_per_file_shift, enable_local_standby, pages_per_file_shift,
enable_compression, zstd_compression_level, auto_reopen_retry_times,
auto_oom_retry_times, auto_reopen_pending_time_us, max_processing_time_us,
yield_budget_us, request_timeout_us, request_queue_timeout_us
```

`COMPAT_BASELINE` 第 692 行的第 12 项标题即「INI 扩展键清单补全（sem-69）」，
但实际只登记了 **10 个**，**9 个从未在 baseline 任何位置出现**（全文 grep 零命中）：

| 未登记扩展键 | Rust 侧是否接线 | C++ `KvOption` 有同名字段？ |
|---|---|---|
| `page_cache_bytes` | 已接线（`startup.rs:151`、SDK `lib.rs:87`） | 无（fkv 独有键） |
| `segment_file_amplify_factor` | 已接线（`startup.rs:138`、SDK `lib.rs:102`） | 有，`LoadFromIni` 不读 |
| `auto_reopen_retry_times` | 已接线（`fkv_runtime/src/config.rs:128`） | 有，`LoadFromIni` 不读 |
| `auto_oom_retry_times` | 已接线（`startup.rs:122`） | 有，`LoadFromIni` 不读 |
| `auto_reopen_pending_time_us` | 已接线（`config.rs:134`） | 有，`LoadFromIni` 不读 |
| `max_processing_time_us` | 已接线（`startup.rs:168`） | 无（fkv 独有键） |
| `yield_budget_us` | 已接线（`startup.rs:171`、SDK `lib.rs:96`） | 无（fkv 独有键） |
| `request_timeout_us` | 已接线（facade `start_runtime`，`fkv/src/lib.rs:44`） | 无（fkv 独有键） |
| `request_queue_timeout_us` | 已接线（同上，`fkv/src/lib.rs:47`） | 无（fkv 独有键） |

baseline 已覆盖的 10 个：第 8 项 `enable_compression` / `zstd_compression_level`；
第 12 项 `segment_size` / `segments_per_file_shift` / `pages_per_file_shift` /
`enable_local_standby`；第 455-463 行「解析但不生效」清单里的
`chunk_size` / `gc_global_mem_size_per_shard` / `pinned_tail_scratch_slots` /
`segment_compact_yield_every`（后 4 个是 Ext + 未接线）。

## 证据

- Ext 键全集提取（宏展开）：19 个，见上。
- baseline 全文命中：
  - `grep -c page_cache_bytes COMPAT_BASELINE` → **0**；
  - `max_processing_time_us` / `yield_budget_us` / `request_timeout_us` /
    `request_queue_timeout_us` / `auto_oom_retry_times` /
    `auto_reopen_retry_times` / `auto_reopen_pending_time_us` → 均 **0**。
- 反证「清单不完整」：`fkv_config` 模块文档「INI 扩展键」一节
  （`crate/foundation/fkv_config/src/lib.rs:53-60`）也只点名
  `enable_compression` / `zstd_compression_level`，未覆盖这 9 个；即
  fkv 自身文档亦未把它们标为「扩展键」。
- 第 12 项标题用词「**补全**」（sem-69 动作），与「9 键缺失」的事实冲突——
  与第 38 轮 sem-73 修正的「已知偏离仅限根不存在」同类过强断言。

## 影响

- 无盘面格式 / 可见数据差异（这些键在 fkv 侧本就按预期生效）。
- 但**同一份 INI 在 C++ 与 fkv 下行为不同、且未被账本登记**：
  - fkv 独有键（`page_cache_bytes` / `max_processing_time_us` /
    `yield_budget_us` / `request_timeout_us` / `request_queue_timeout_us`）：
    C++ 视为未知键静默忽略（按字段默认跑），fkv 按配置生效——与第 8 项
    `enable_compression` 的登记理由完全同构（「同一份 INI 在 C++ 下按默认启动、
    在 Rust 下按 INI 生效」）。
  - 有同名字段但 C++ 不读的键（`segment_file_amplify_factor` /
    `auto_reopen_retry_times` / `auto_oom_retry_times` /
    `auto_reopen_pending_time_us`）：与第 12 项 `enable_local_standby` 同构
    （C++ 只能代码 / C API 设置）。
- 迁移 / 差分口径缺失：Rust→C++ 迁移时这些键不生效、两侧差分不得按
  「同配置」对拍——这条口径 baseline 对压缩 / 布局键已给，对这 9 键没给。

## 建议

（登记型）

- 把上表 9 键补进 `COMPAT_BASELINE` 第 12 项（或新增「INI 扩展键全量清单」条目），
  与第 8 项 / 第 12 项同口径分两类：
  1. fkv 独有键（无 C++ 字段）：仅 fkv 生效，C++ 静默忽略；
  2. C++ 有同名字段但 `LoadFromIni` 不读：C++ 只能代码 / C API 设置。
  并同步给「Rust→C++ 迁移需改代码 / C API」的差分口径。
- 或（可选）在 `fkv_config` 模块文档「INI 扩展键」一节把 19 键整体列出，让
  宏声明点与文档单源对齐；本项随即移出清单。

倾向补齐 baseline（登记型，不动代码）。

## 验收方式

```bash
cd /Users/z/git/db/kv/eloqstore2fkv
for k in page_cache_bytes segment_file_amplify_factor auto_reopen_retry_times \
         auto_oom_retry_times auto_reopen_pending_time_us max_processing_time_us \
         yield_budget_us request_timeout_us request_queue_timeout_us; do
  printf '%-30s %s\n' "$k" "$(grep -c "$k" COMPAT_BASELINE)"
done   # 修复后应全部 >= 1
cd /tmp/r29/fkv && cargo test -p fkv_config   # 配置解析套件保持全绿
```

（本轮已跑：`fkv_config` 全套全绿；Ext 键接线点逐键 grep 见上表。）

## 实现记录（2026-11-08，登记型）

- COMPAT_BASELINE「INI 解析边界」第 12 项重写：由只列 10 键改为**覆盖全部 19 个
  `[Ext]` 键**并按后果分三类——布局类（segment_size / segments_per_file_shift /
  pages_per_file_shift / enable_local_standby）、行为类
  （auto_reopen_retry_times / auto_reopen_pending_time_us / auto_oom_retry_times /
  max_processing_time_us / yield_budget_us / request_timeout_us /
  request_queue_timeout_us）、容量类（page_cache_bytes /
  segment_file_amplify_factor）；并注明 C++ `LoadFromIni` 对全部 19 键零读取、
  接线点在 `fkv_runtime::startup` / `fkv_config` 的 `kv_options!` 宏。
- 验证：`check_baseline_sync --check` 44/44（`--fix` 刷新压行后）；
  `check_issue_links --check` 1397/0/0。

## 复审记录（2026-11-08，第 40 轮结构域）

- 判定：**达标**，移入 reviewed/。19 个 `[Ext]` 键在 COMPAT_BASELINE 全部命中；
  第 12 项三分类（布局 4 / 行为 7 / 容量 2）与宏体逐键对账**无遗漏、无多余**；
  其余 6 键由第 8 项 + 「解析但不生效」清单覆盖；「C++ `LoadFromIni` 对 19 键
  零读取」经 `reader.Get*` 列表核对成立。
