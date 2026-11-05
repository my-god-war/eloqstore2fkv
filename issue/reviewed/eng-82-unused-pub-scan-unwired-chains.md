# eng-82 unused.js 扫描：7 个符号 / 6 组“有实现、无调用方”，属于未打通的链路或应删除的 YAGNI API

## 来源

`./sh/unused.js` 的未引用 pub 方法中，除 eng-81 的直接删除项外，剩下 7 个符号（6 组）不是单纯死代码：
实现已经存在，但生产调用链没有接上，或设计意图与当前唯一调用路径分叉。逐项给出“接线 / 删除”判断。

## 1. `KvOption::segment_file_size` / `file_page_offset_mask`

- 位置：`crate/foundation/fkv_config/src/lib.rs:181-183` / `:185-187`
- 现状：两个派生方法零引用；同一计算在别处各写一遍：
  - `SegmentLayout::file_size`（`crate/core/fkv_large/src/lib.rs:82-87`）
  - `CompioStore::sequence_file_size`（`crate/io/fkv_io_compio/src/lib.rs:1618-1622`）
  - 页内偏移 mask 在 `CompioStore::page_offset` / `segment_offset`（`:1608-1616`）、
    `fkv_store` 的 `(1 << pages_per_file_shift) - 1` 多处内联。
- 判断：**删除**，不要接线。`SegmentLayout` 已经是 segment 布局的唯一权威；
  `KvOption` 再加一份派生值只会增加第三处定义（与 eng-71 同源）。
  若未来需要从配置直接算 mask，应让 `CompioStore` 持有 `SegmentLayout` / shift 配置，
  而不是从 `KvOption` 取值再手算。

## 2. `Runtime::shard_metrics`

- 位置：`crate/engine/fkv_runtime/src/lib.rs:309-315`
- 现状：`metrics()`（`:303-307`）已返回全 shard 的 `MetricsSnapshot`；
  `shard_metrics(id)` 只是按下标取一个 `ShardSnapshot`。全仓零调用，
  `fkv_cli` / `fkv_bench` / `web` 都只用聚合结果或测试里的 `metrics()`。
- 判断：**删除**（或等 CLI 增加 `stats --shard` 需求时再接）。
  当前保留会让“每 shard 指标已对外可用”的错觉成立，但实际没有消费者。

## 3. `KvStore::inner_mut`

- 位置：`crate/facade/fkv_sdk/src/lib.rs:53-55`
- 现状：`inner()` 只读访问是合理的测试/高级用法；`inner_mut()` 直接暴露
  `&mut Store<CompioStore>`，可绕过 SDK 的状态假设，且零调用。
- 判断：**删除**。若确有高级用法，应在 SDK 上提供具名方法（如 `truncate` /
  `put_large` 已暴露），而不是把内部 `Store` 的可变引用交出去。

## 4. `CompioStore::preload_tables`

- 位置：`crate/io/fkv_io_compio/src/lib.rs:1552-1555`
- 现状：`preload_paths(tbl)`（`:1639`）是活接口，`Store` / `MeteredIo` / SDK 都转发它；
  `preload_tables(tbl_li)` 只是 for 循环求和，零调用。
- 判断：**删除**。批量预加载没有当前调用场景（attach/reopen 是逐表触发）；
  若未来要“启动时预热多表”，应由上层 `Runtime` 在 attach 后遍历表列表调用
  `preload_paths`，而不是在 IO 后端保留一个未接线的批量入口。

## 5. `Standby::list_remote`

- 位置：`crate/io/fkv_standby/src/lib.rs:198-200`
- 现状：包装 `self.source.list(tbl)`；但 `sync_table`（`:216-226`）直接用
  `self.source.list(tbl)`，绕过了这个公开入口。`list_remote` 零调用。
- 判断：**二选一**：
  - 让 `sync_table` 改调 `self.list_remote(tbl)`，统一走 standby 的公开入口（推荐，
    这样 mock `RemoteSource` 与真实后端的行为在同一个函数上测试）；
  - 或删除 `list_remote`，承认 `source` 是内部字段。
  当前“有公共方法但内部路径绕过”是最差形态。

## 6. `RsyncShell::with_rsync_bin`

- 位置：`crate/io/fkv_standby/src/rsync.rs:57-60`
- 现状：`RsyncShell::new`（`:48-55`）把 `rsync_bin` 固定为 `"rsync"`；
  `with_rsync_bin` 提供覆盖钩子，但全仓零调用，`KvOption` / `StandbyConfig`
  也没有对应字段。部署环境若 rsync 不在 PATH，无法通过配置修正。
- 判断：**接线或删除**：
  - 接线：`KvOption` 增加 `rsync_bin`（或从 `PATH` / 环境变量读取）并在
    `Standby::new` 构造 `RsyncShell` 时调用 `with_rsync_bin`；
  - 删除：明确只支持 PATH 中的 `rsync`，同时把 `rsync_bin` 字段简化为常量，
    避免留一个“看起来可配”的 builder。
  当前形态属于“配置钩子存在但调用链未闭合”。

## 建议顺序

1. `shard_metrics` / `inner_mut` / `preload_tables`：删除（无外部契约）。
2. `segment_file_size` / `file_page_offset_mask`：删除，统一到 `SegmentLayout`（eng-71）。
3. `list_remote`：让 `sync_table` 调用它，闭合入口。
4. `with_rsync_bin`：按部署需求接 `KvOption.rsync_bin`，否则删除。

## 严重度

P3（无行为错误；API 面误导 + 未来接线时容易接错层）

## 核验记录（2026-11-04）

6 个符号的 `grep -rn`（含 tests/tool）均 0 调用；`sync_table` 与 `list_remote`
的 `source.list` 重复实读；`RsyncShell::new` 硬编码 `"rsync"` 与
`with_rsync_bin` 零调用实读；`Runtime::metrics` 与 `shard_metrics` 的包含关系实读。

## 审查结论（2026-11-04）

复核方式：全仓 `rg`（含 tests/tool/web/doc）+ codegraph，逐个确认调用者；并行任务未接线任何一项。

| 符号 | 判定 | 理由 | 落地 |
| --- | --- | --- | --- |
| `KvOption::segment_file_size` | 删除 | `SegmentLayout::file_size` 是唯一权威，配置侧再派生属第三份定义（eng-71 同源） | 移除方法，无引用需清理 |
| `KvOption::file_page_offset_mask` | 删除 | mask 由 `CompioStore` 依据 shift 在内部计算，不经 `KvOption` | 移除方法 |
| `Runtime::shard_metrics` | 删除 | `metrics()` 已返回全 shard 快照；无 `stats --shard` 消费者 | 移除方法 + 更新 `doc/architecture/02-execution-model.md` |
| `KvStore::inner_mut` | 删除 | 把 `&mut Store` 交给 SDK 使用者会绕过状态假设；具名方法已覆盖高级用法 | 移除方法 |
| `CompioStore::preload_tables` | 删除 | `preload_paths` 是活入口；批量场景应由上层遍历调用，不在 IO 后端保留 | 移除方法 |
| `Standby::list_remote` | 接线 | 保留公开入口，`sync_table` 改为 `self.list_remote(tbl)`，mock 与真实后端走同一条路径 | 改 1 行调用 |
| `RsyncShell::with_rsync_bin` | 删除 | 无配置字段、无生产构造点（`RsyncSource::new` 仅测试使用），12b 规划也未要求覆盖 | 移除 builder 与 `rsync_bin` 字段，改常量 `RSYNC_BIN` |

### 验证

- `cargo check --workspace --all-targets --all-features`：通过
- `cargo nextest run --workspace --all-features --no-fail-fast`：480 passed，1 skipped
- `cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings`：零警告

## DONE（2026-11-04）

（审查结论表所列 7 项处置已执行：6 删 1 接线。）

## 复审（2026-11-05 r6b）

通过。

- 删除项全仓无残留定义：`segment_file_size`/`file_page_offset_mask`/`shard_metrics`/`inner_mut`/`preload_tables` 在 `crate/` 源码 grep 0 命中（唯一 `shard_metrics` 命中是测试函数名 `fkv_shard/tests/cloud_cache_metrics_wiring.rs:60`，非 API）。
- `with_rsync_bin` 已删：`fkv_standby/src/rsync.rs:22` 改常量 `RSYNC_BIN`。
- `list_remote` 已接线：`fkv_standby/src/lib.rs:272` 定义，`sync_table` 等内部路径改调 `self.list_remote`（`:309`、`:368`），公开入口闭合。
