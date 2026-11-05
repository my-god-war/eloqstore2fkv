# fmt-18a 页布局拆 data/index restart 间隔（task_write + store）

## 位置
- `crate/engine/fkv_task_write/src/lib.rs:39-59`（`WriteArgs.restart_interval` / `PageLayout.restart_interval` 单值）
- `crate/engine/fkv_task_write/src/lib.rs:1252,1266,1328`（`IndexPageBuilder::new(..., layout.page.restart_interval, ...)`）
- `crate/engine/fkv_task_write/src/lib.rs:659,668,747,756`（data 页归并/重分配用同一值）
- `crate/engine/fkv_store/src/lib.rs:166,200`（`Store { restart_interval }` 单字段）
- C++: `include/kv_options.h:38-39`；`src/storage/data_page_builder.cpp:54-57`；`src/storage/index_page_builder.cpp:113,121,157`

## 方案
1. `WriteArgs`：`restart_interval` 拆为 `data_restart_interval` + `index_restart_interval`。
2. `PageLayout`：同名字段两个；data 路径改用 `data_restart_interval`，index 路径（含 `build_index_page_region` 的 ri、`IndexPageBuilder`/`FastIndexPageBuilder` 编码）改用 `index_restart_interval`。
3. `Store`：`data_restart_interval` + `index_restart_interval` 两字段；`new(io, page_size, restart_interval)` 保留为「两页共用的初始值」（测试批量构造不被破坏），新增 `with_index_restart_interval(u16)` builder 覆盖 index 侧；两处 `WriteArgs` 构造按各自字段透传。
4. 不改 `fkv_page`：builder 仍按调用方给的单值编码，语义不变。

## 验收
- 新增测试（`fkv_task_write/tests/write.rs` 或 store 集成测试）：同一批数据，
  - `index_restart_interval` 不同 → index 页字节 / restart 数不同（`restart_num`），data 页字节**逐字节相同**；
  - 两值相同 → 与改动前单值路径产物逐字节一致。
- 默认 16/16 时盘面字节不变（C++ 口径：两旋钮独立，默认相同）。
- `cargo nextest run -p fkv_task_write -p fkv_store` 全绿；clippy `-D warnings` 零警告。

## DONE（2026-11-04）

改动文件：
- `crate/engine/fkv_task_write/src/lib.rs`：`WriteArgs` / `PageLayout` 各拆 `data_restart_interval` + `index_restart_interval`；data 路径（merge / rebalance）读 data 侧，index 路径（`build_index_page_li`、region 直拷 ri、`IndexPageBuilder`）读 index 侧。
- `crate/engine/fkv_store/src/lib.rs`：`Store` 双字段；`Store::new` 第三参同时作为两页初始值（测试批量构造不破坏），新增 `with_index_restart_interval(u16)` 覆盖 index 侧；4 处 `WriteArgs` 构造按各自字段透传。
- `crate/engine/fkv_task_write/tests/write.rs`：17 处 `WriteArgs` 字面量补双字段。

新增测试：
- `index_page_uses_index_restart_interval_only`（task_write 单测）：同一 children，data 侧从 16 改 2 → index 页逐字节不变；index 侧 16→4 → `restart_num` 1→3，字节变化但 children 序列一致。
- `data_page_ignores_index_restart_interval`（task_write 单测）：index 侧 16 vs 2，多页 data 页字节逐字节相同。
- `fkv_store/tests/restart_interval.rs`：`Store::new(.., 4)` vs `.with_index_restart_interval(2)`，root index 页 restart 数 5→9，leftmost data 页字节一致。

盘面影响：默认 16/16 时字节与改动前完全一致；只有用户显式设置且不相等才改变 index 页布局（对齐 C++ `index_page_builder.cpp` 独立旋钮）。

## 复审（2026-11-05 r6）

通过。`fkv_task_write/src/lib.rs:43-46` `WriteArgs`/`PageLayout` 双字段；data 路径读 `layout.data_restart_interval`（:779/:789/:888/:898）；`fkv_store/src/lib.rs:200-203` 双字段、`:254-261` `Store::new` 共用初始值、`:324` `with_index_restart_interval`。两个单测在 `fkv_task_write/src/lib.rs:1815/1849`（data 侧改动 index 页逐字节不变断言 :1837），集成测试 `fkv_store/tests/restart_interval.rs:49`。
