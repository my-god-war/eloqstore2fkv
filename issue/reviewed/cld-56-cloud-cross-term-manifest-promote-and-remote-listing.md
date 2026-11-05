# cld-56 云端跨 term manifest 选择/提升缺失 + 远端旧 term 对象不在 GC 候选集

来源：`issue/done/core-22b-cloud-read-per-file-branch-term-key.md` 的两条遗留（22b 落地时明确「另行立案」，当时未立案）。

## 缺口 1：云端 GetManifest 跨 term 选择/提升未实现

C++ `async_io_manager.cpp` 的 `GetManifest`：列出全部 term 的 manifest，选最新 term；
若最新 term 与进程 term 不同，把进程 term 提升（promote）到最新，保证跨 term 云 reopen
能继续。Rust 侧（`fkv_cloud`）只取当前 identity 的 manifest，跨 term 云 reopen 会
先读不到新 term manifest（22b 验证记录原话）。

## 缺口 2：`remote_file_id_page` 只列当前 identity 的对象

旧 term 写入的对象 key 带 `(branch, term)` 后缀，当前 identity 列举拿不到 → GC 候选集
漏掉旧 term 对象（云盘垃圾只增不减）。C++ `ListObject` 按前缀列举全部。

## 对齐建议

- 缺口 1：cloud manifest 列举按 term 前缀展开，取最大 term；term 不一致时走
  `install_manifest` + term 提升（复用 sto-70 的安装对齐路径）。
- 缺口 2：列举前缀从 `data_` 起而不是 `data_<id>_<branch>_<term>` 全拼，
  解析出 (file_id, branch, term) 三元组后再归一；GC 侧按 file id 归并。

## 验收

- 云端：term A 写入 → term B reopen → 读取 term A 数据 + GC 后旧 term 对象被清。
- `cargo nextest run -p fkv_cloud -p fkv_task_gc`。

## 关联

- `sto-70`（standby 安装 term 对齐，done）是缺口 1 的本地侧先例。
- `io-29a/b`（list 分页契约，done）是缺口 2 的列举基座。

## DONE（2026-11-05）

### 改动（仅 `crate/engine/fkv_cloud`）

- 缺口 1：新增 `latest_remote_manifest_bytes`。按 `<tbl>/manifest_` 前缀分页列举，
  解析 `manifest_<branch>_<term>`，只取同 branch 的无 tag manifest 并选最大 term；
  `read_manifest` / `refresh_manifest` 都改走该选择，选中内容用
  `local.switch_manifest` 安装为本地活跃 manifest。archive（tag）与其它 branch 不参与。
- term 对齐：选中 manifest 的 term 与进程 identity 不同时不改写 identity / term 文件，
  由既有 `Store::install_replay_state`（`replay.branch_metadata.term != self.term`）
  做 allocator bump + `set_file_range_li`。Rust 的 term 权威是 attach 的 term 文件
  CAS（`claim_term`），cloud 层擅自提升 identity 会让数据 key 与 Store / term 文件脱节。
- 缺口 2：`remote_file_id_li_of` 改为按 `data_` / `segment_` 对象名前缀列举，
  解析 `(file_id, branch, term)`，保留**同 branch 的全部 term**并归并成 file id
  候选（跨 branch 对象留给各自 branch 的 GC 轮次）。prewarm 的
  `remote_data_file_page` / `remote_segment_file_page` 仍只返回当前 identity，
  不改变下载语义。
- `list_file_id_li` / `list_segment_file_id_li` 输出改为升序（对齐 io-41 的
  `list_*` 升序契约，GC / compact 遍历可复现）。
- 旧 term manifest 剪枝：云端 `main` 的活跃 manifest 同样按 term 命名，因此
  `prune_superseded_manifest_files` 不再无条件跳过 `main`；仍保留原有保守约束
  （`term > 0`、identity 一致、远端同 branch 最高 term == 本实例 term、只删更旧
  无 tag manifest）。否则旧 manifest 会一直让 GC retained 集合引用旧 term 对象。

### 测试（`MemObjectStore`，离线）

`crate/engine/fkv_cloud/tests/term_key.rs`：
- `read_manifest_selects_latest_term_for_branch`：选最大 term，忽略 archive / 其它 branch，
  并安装为本地活跃 manifest。
- `cross_term_reopen_reads_old_term_data`：term 3 写入 → term 5 reopen 读到旧数据，
  新写入落 `data_*_main_5`。
- `install_external_snapshot_selects_latest_term`：follower 采纳远端更高 term。
- `remote_listing_includes_previous_terms_for_gc`：旧 term data / segment 进入候选，
  其它 branch 不混入，prewarm 分页仍只取当前 identity。
- `gc_reclaims_superseded_term_objects`：大 value + data 端到端；第一轮 GC 剪旧
  manifest，第二轮回收 `*_main_3` 对象，当前 `manifest_main_5` 保留。

`crate/engine/fkv_cloud/tests/manifest_prune.rs`：
- `prune_main_removes_superseded_terms`：main 旧 term manifest 剪枝；
- 原 main 用例改名为 `prune_keeps_main_when_newer_term_missing`（远端无本实例 term
  时仍保守跳过）。

### 验证

- `cargo test -p fkv_cloud`：43 passed。
- `cargo clippy -p fkv_cloud --all-targets -- -D warnings`：EXIT=0。
- `cargo +nightly fmt -p fkv_cloud`。

### 风险 / 边界

- 旧 manifest 被剪后，仅被旧 manifest 引用的 data / segment 需要再跑一轮 GC 才回收
  （既有设计：本轮 retained 集合仍含旧引用），测试按两轮断言。
- 同一 file id 的跨 term 变体以 manifest 的 `file_ranges` 解析删除；core-22a 的
  term bump 保证同 branch 跨 term 使用新 file 边界，正常路径不会出现同 id 多 term。
- 未改 `fkv_task_gc` / `fkv_store` / `fkv_io`；未依赖真实 S3。

## 复审（2026-11-05 r6）

**通过。** 核实（`fkv_cloud/src/lib.rs`）：

- :1980 `latest_remote_manifest_bytes`（按 `<tbl>/manifest_` 前缀分页列举、同 branch 最大 term、tag/他 branch 排除）；`read_manifest`/`refresh_manifest`（:1809）改走该选择。
- :2129 `remote_file_id_li_of` 按 `data_`/`segment_` 前缀列举并解析 `(file_id,branch,term)`（缺口 2）；:1891 `prune_superseded_manifest_files` 不再无条件跳过 main。
- 测试 `tests/term_key.rs:163/203/251/274/313` 五条 + `tests/manifest_prune.rs:157/188` 两条全部在位；抽读 :313 `gc_reclaims_superseded_term_objects`：两轮 GC 断言旧 `manifest_main_3` 剪除（:355-360）、`_main_3` 对象回收（:366）、`manifest_main_5` 保留（:372）——断言实质覆盖两个缺口。
- 「cloud 层不擅自提升 identity，由 attach term CAS + install_replay_state bump」的记档与实现一致。
