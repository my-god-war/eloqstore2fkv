# 32b 测试迁移到 `fkv_testkit::TempRoot`

## 来源

`issue/done/32-stub-crates-missing-task-layer.md` 拆分项。`fkv_testkit` 已于本轮实现
`TempRoot`（`crate/facade/fkv_testkit/src/lib.rs`，构造即建目录、`Drop` 递归清理、
`into_path` 放弃清理、`tag` 自动 sanitize），并有 3 个集成测试。

## 现状

全仓 50 个文件各自手抄同一段 `fn temp_root()`：

```
temp_dir() + format!("fkv_xxx_{pid}_{nanos}_{seq}") + create_dir_all，清理靠末尾手动 remove_dir_all
```

抽样：`crate/engine/fkv_store/tests/compio_store.rs:15-27`、
`crate/io/fkv_io_compio/tests/hardening.rs:15-31`。

## 方案

1. 每个用到 `temp_root` 的 crate，用 `cargo add -D --path ../../facade/fkv_testkit`
   加 dev-dependency（路径按 crate 深度调整）。
2. 删本地 `fn temp_root()`，改：
   ```rust
   let root = TempRoot::new("fkv_store");
   let dir = root.path();          // 需要 PathBuf 时 root.path().to_path_buf()
   let file = root.join("data");   // 拼子路径
   ```
   依赖 drop 自动清理，删掉手工 `remove_dir_all`；`into_path()` 仅用于必须把目录交给
   外部进程的用例（如 `fkv_io_compio` 的 fd / kill 测试），并显式清理。
3. 分批落地，每批一个 crate 单独 `cargo nextest run -p <crate>`：
   `fkv_store` → `fkv_runtime` → `fkv_io_compio` → 其余。
4. 迁移时只改各自 `tests/` 与 `[dev-dependencies]`，不动 `src/`。

## 验收

1. `rg -l 'fn temp_root' crate | wc -l` == 0。
2. `rg -l 'remove_dir_all' crate --glob '**/tests/**'` 只允许出现在 `fkv_testkit` 相关或
   明确需要手工清理的少数用例，且每处有注释说明原因。
3. 每批 `cargo nextest run -p <crate> --all-features` 通过；`cargo clippy --workspace
   --all-targets -- -D warnings` 零警告。
4. 测试不再向 `/tmp` 泄漏目录：跑完后 `ls $TMPDIR | grep -c '^fkv_'` 不随运行次数增长。

## 风险

多个 agent 并发改 `tests/`。迁移前先 `git status` 确认目标文件干净；
同一时刻只迁一个 crate，避免与他人的测试改动交叉。

## 进展（2026-11-04，Lane D bench/testkit）

已迁移 3 个 crate（11 个文件），均为「`git status` 干净 + 本 crate `src` 稳定、能在本
lane 内跑绿」的 crate：

| crate | 文件 | 验证 |
|---|---|---|
| `fkv_io_compio` | `tests/` 全部 9 个 | `nextest -p fkv_io_compio` 35 passed；clippy 0 警告 |
| `fkv_standby` | `tests/standby.rs` | 17 passed；clippy 0 警告 |
| `fkv_cli` | `tests/cli.rs` | 1 passed；clippy 0 警告 |

合并跑 `nextest -p fkv_io_compio -p fkv_standby -p fkv_cli` → 53 passed / 0 failed。

做法：

- 每个 crate 的 `[dev-dependencies]` 加 `fkv_testkit = { path = ".../facade/fkv_testkit" }`
  （`fkv_cli` 用 `../fkv_testkit`）。`fkv_testkit` 零依赖，不会成环。
- 删本地 `fn temp_root` 与末尾手工 `remove_dir_all`，改 `TempRoot::new("<tag>")`：
  要 `PathBuf` 用 `root.path().to_path_buf()`，接口收 `&Path` 用 `root.path()`，
  拼子路径用 `root.join(..)`。
- 本批无 `into_path()` 使用者：用例内部都 `await` / `join` 完外部命令，`Drop` 时进程
  已退出，直接交给 `TempRoot` 清理。
- 额外收益：`fkv_io_compio::naming::path_cache_avoids_repeated_directory_scans`
  原来**没有**清理（每跑一次漏一个 `fkv_naming_*` 目录），迁移后不再泄漏。
  实测连跑前后 `ls $TMPDIR | grep -c '^fkv_'` 不变（560 → 560，存量来自其它 lane
  未迁移的用例，如 `fkv_index_cache_*`）。

## 阻塞

1. **全仓迁移无法在本 lane 闭环。** Lane D 的 crate 地盘是 `fkv_bench` + `fkv_testkit`
   （`fkv_testkit` 侧本就已完成：`TempRoot` + 3 个集成测试，无本地 `fn temp_root`）。
   剩余 46 个目标文件分布在 6 个其它 lane 的 crate：`fkv_store` 15、`fkv_runtime` 16、
   `fkv_shard` 9、`fkv_large` 4、`fkv_task_gc` 1、`fkv_sdk` 1。
2. **这些文件正在被在飞修改。** 本仓同时跑着另一个 swarm（`session_vole`），实测
   mtime：`fkv_runtime/tests/runtime.rs` 23:00、`fkv_shard/tests/lifecycle.rs` 23:01、
   `fkv_sdk/tests/sdk.rs` 23:04；且 `fkv_runtime` 的 `fn temp_root` 计数在 10 分钟内
   从 15 涨到 16（对方在新增文件）。按本 issue 的风险条款（同一时刻只迁一个 crate）
   与 `issue/run-dragon-report.md` 的 crate-lane 隔离协议，硬改在飞文件有覆盖风险。
3. **验收 1/3 是 workspace 级，本 lane 无法闭环。** 全仓 `nextest` /
   `cargo clippy --workspace --all-targets` 依赖所有 crate 的 `src` 稳定；而
   `fkv_store` / `fkv_runtime` 的 `src/lib.rs` 正被另一 swarm 改、且在本 lane 的
   禁改清单里，本 lane 既不能改也不能保证全仓绿。
4. **验收 1 的 grep 模式会误伤 `fkv_testkit` 自己的测试。**
   `rg -l 'fn temp_root' crate` 命中 `crate/facade/fkv_testkit/tests/temp_root.rs` 的
   `fn temp_root_unique_and_cleaned_on_drop()` 等三个**测试函数名**（不是本地 helper），
   即使全仓 helper 都删干净，该判据也永远不为 0。建议收紧为
   `rg -l 'fn temp_root\(' crate`（本地 helper 一定带参数表）。

## 建议

- 由 coordinator 把剩余 6 个 crate 按 owner lane 分摊（每个 lane 只迁自己 crate 的
  `tests/`，逐 crate `nextest` + clippy），或授权本 lane 在对应 crate 的 `src` 稳定后
  再做批次。
- 本 lane 可继续迁移的前提：目标 crate 的 `src/lib.rs` 不再被他人修改，且目标
  `tests/*.rs` 本轮无 mtime 变化（当时 `fkv_io_compio` / `fkv_standby` / `fkv_cli`
  满足，故先做了这 3 个）。

## DONE（2026-11-04）

本轮再迁移 4 个 crate、21 个测试文件（累计 7 个 crate、32 个文件）：

| crate | 本轮文件 | 结果 |
|---|---|---|
| `fkv_large` | `batch` `batch_fail` `managed` `large` | 4 文件，30 tests 绿 |
| `fkv_store` | `branch_archive` `branch_gc` `branch_isolation` `branch_store` `compat` `compio_manifest` `compio_store` `compression` `gc_cleanup` `large_value` `maintenance_batch` `naming` `overflow` `truncate_drop` `ttl` | 15 文件全部迁移 |
| `fkv_sdk` | `sdk` | 1 文件 |
| `fkv_task_gc` | `gc` | 1 文件 |

做法：

- 各 crate `[dev-dependencies]` 用 `cargo add --dev --path .../facade/fkv_testkit` 添加。
- 删除本地 `fn temp_root`，调用点改 `TempRoot::new("<crate>_<场景>")`；
  需要 `PathBuf` 处用 `root.path().to_path_buf()`，拼子路径用 `root.join(..)`，
  删掉末尾手工 `remove_dir_all`。本批无 `into_path()` 使用。
- 保留项（**未迁移**，因仓库内并行会话正在改这些文件）：`fkv_runtime`
  （`src/lib.rs` + `tests/` 17 个）与 `fkv_shard`（`tests/` 9 个）。这两处仍各自手抄
  `temp_root`，待对应 lane 收工后由 owner 迁移。
- 迁移中顺带修复 `fkv_store/tests/naming.rs`：`has_file` 收 `&Path`，调用改为
  `root.path()`。

验证：

1. `cargo check --workspace --all-targets --all-features` 通过。
2. `cargo nextest run --workspace --all-features --no-fail-fast` → 600 passed / 1 skipped。
   首轮 `fkv_runtime::write_group_commit::concurrent_puts_are_group_committed` 偶发失败
   （并发时序型，单跑与重跑均绿；该文件本轮未改动，另一 lane 新建），非本次迁移引入。
3. `cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings` 零警告。
4. 迁移 crate 连跑两次前后 `ls $TMPDIR | grep -c '^fkv_'` 保持 645 不变（存量来自未迁移
   的 `fkv_runtime` / `fkv_shard` 用例）。
5. `rg -l 'fn temp_root\(' crate` 仅剩上述保留项。

## 复审（2026-11-05 r6）

通过。

- 本 lane 范围（fkv_io_compio / fkv_standby / fkv_cli）：tests 已全部使用
  `fkv_testkit::TempRoot`（抽读 `tests/write_pool.rs`、`tests/hardening.rs`、
  `tests/evict.rs` 均为 `TempRoot::new`）。
- 全仓 `grep -rl 'fn temp_root(' crate` 仅剩 `fkv_runtime`（22 文件，含
  src/lib.rs）与 `fkv_shard`（13 文件），与 DONE「保留项待 owner 迁移」的
  记录完全一致，未见范围外残留。
