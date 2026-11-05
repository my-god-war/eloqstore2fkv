# fmt-18 index_page_restart_interval 是死配置：单一 restart_interval 同时写 index 与 data 页

## 位置
- Rust: `crate/foundation/fkv_config/src/lib.rs:19-20`（两字段定义）、`:90-91`（默认各 16）、`:260-261`（均可从配置解析）—— 但全仓 `index_page_restart_interval` **零消费者**；`crate/facade/fkv_sdk/src/lib.rs:31`（`Store::new(io, page_size, options.data_page_restart_interval)` 单值）；`crate/engine/fkv_store/src/lib.rs:107-108,144-148`（`Store { restart_interval: u16 }` 单字段）；`crate/engine/fkv_task_write/src/lib.rs`（`IndexPageBuilder::new(..., layout.restart_interval, ...)` 与 `DataPageBuilder` 共用同一值）
- C++: `include/kv_options.h:38-39`（`data_page_restart_interval = 16; index_page_restart_interval = 16;` 两个独立旋钮）；`src/storage/data_page_builder.cpp:54-57`（data 页用 `data_page_restart_interval`）；`src/storage/index_page_builder.cpp:113,121,157`（index 页用 `index_page_restart_interval`）

## 现状
两个独立子系统（恢复/replay、facade/options、页构建器与 comparator）的审查**独立**发现了同一条，交叉印证。

## C++ 实现
index 与 data 页 restart 间隔分别配置、分别生效。

## 差异与风险
- 默认 16/16 时盘面字节一致，无直接影响；
- 用户设 `index_page_restart_interval` 完全无效（静默死配置）；
- 设 `data_page_restart_interval ≠ 16` 会连带改变 **index 页** restart 布局 → 与 C++ 盘面分歧（index 页字节不同、树形不同）→ 字节级 oracle 差分（issue 01）必失败。属潜伏炸弹。

## 对齐建议
- `PageLayout`/`Store`/`WriteArgs` 拆 `data_restart_interval` + `index_restart_interval` 两字段，`KvStore`/`ShardConfig`/Runtime 各自从 KvOption 透传；
- 过渡期至少在 config 解析处对两旋钮不相等时告警。

## 严重度
P1（潜伏盘面分歧 + 死配置）

## 核验记录（2026-11-04）
两路独立发现 + 本人全仓 grep 复核：`index_page_restart_interval` 仅 config 三处出现，无消费者；`fkv_task_write` 中 IndexPageBuilder 与 DataPageBuilder 共用 `layout.restart_interval` 实读确认。

## 审查结论（2026-11-04）

结论：**合理**，需实现。

复核（当前工作区实读）：
- `KvOption.data_page_restart_interval` / `index_page_restart_interval` 两字段、默认 16/16、INI 均可解析。
- `index_page_restart_interval` 仍**零消费者**：`KvStore::new` 只取 data 值，`ShardConfig`/`RuntimeConfig` 各只有一个 `restart_interval`，`Store` 单字段，`WriteArgs`/`PageLayout` 单值。
- `fkv_task_write` 中 `IndexPageBuilder::new(.., layout.page.restart_interval, ..)` 与 `DataPageBuilder` 共用同一值，确认死配置 + 连带改 index 页布局的隐患。
- C++ 口径：`data_page_builder.cpp` 用 `data_page_restart_interval`，`index_page_builder.cpp` 用 `index_page_restart_interval`，两旋钮独立。

拆分落地：
- `issue/todo/fmt-18a-page-layout-split-restart-interval.md`（task_write + store 拆字段）
- `issue/todo/fmt-18b-kvoptions-chain-plumbing.md`（config/sdk/shard/runtime 透传）

盘面影响：默认 16/16 时字节与现状完全一致；仅当用户显式设置 index 旋钮且不等于 data 旋钮时 index 页 restart 密度改变（语义不变，对齐 C++）。不引入过渡告警，直接接线。

## DONE（2026-11-04）

死配置已接线：`index_page_restart_interval` 经 `KvOption → KvStore/ShardConfig/Runtime → Store.with_index_restart_interval → WriteArgs/PageLayout` 生效，index 页与 data 页 restart 布局互不影响。

默认 16/16 盘面字节不变；显式设置 index 旋钮只改变 index 页。

测试：`cargo nextest run -p fkv_config -p fkv_page -p fkv_task_write -p fkv_store -p fkv_runtime -p fkv_shard -p fkv_sdk` → 228 passed（含 4 个新增用例）。

后续可选项：C++ 字节级 oracle 差分（issue 01）可用 `data_page_restart_interval=8 / index_page_restart_interval=2` 作交叉 case。

## 复审（2026-11-05 r6）

通过。死配置已全链接线：`fkv_sdk/src/lib.rs:44` `with_index_restart_interval(options.index_page_restart_interval)`、`fkv_shard/src/lib.rs:100,530`、`fkv_runtime/src/lib.rs:1110-1112`、`fkv_config/src/lib.rs:167`（字段）+ `:311-312`（validate 拒 0）。实现细节见 18a/18b（均已复核）。
