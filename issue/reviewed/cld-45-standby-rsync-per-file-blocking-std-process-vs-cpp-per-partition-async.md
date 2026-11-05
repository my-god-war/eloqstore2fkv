# cld-45 standby 每文件起一次 rsync 且用 std::process 阻塞执行，C++ 每分区一次 rsync + pidfd 异步监管

## 位置
- Rust: `crate/io/fkv_standby/src/rsync.rs:78-91`（`RsyncShell::fetch` 对**单个文件**跑 `rsync -a ...`）；`crate/io/fkv_standby/src/lib.rs:216-237`（`sync_table` 对每个 data 文件 `for file in data_li { self.source.fetch(...) }` 串行）；`:199-207, 377-384`（`block()` 用 `spawn_blocking` 跑同步命令，D34 明确先用 `std::process`）；`:57-92`（`CommandRunner::output` 用 `std::process::Command::output()` 阻塞等待）
- C++: `eloqstore/src/standby_service.cpp:706-713`（`StartRsyncJob` 对**整个分区目录**一次 `rsync -a --inplace --include=data_* --exclude=* <remote>/ <dest>/`）；`:671-735`（`SpawnChild` 起子进程 + pidfd 注册到 epoll）；`:966-1015`（`HandleExitedChildren`/`waitpid` 异步回收）；`:1190+`（`SpawnChild` 用 `fork`+`exec`+`pipe`+`pidfd` 非阻塞）

## 现状
Rust standby replica 同步一个分区：
1. `sync_table` 先 `list` 出远端所有 data 文件（`:185`）；
2. `:216-237` 对每个 data 文件**逐个**调用 `source.fetch`，每次 fetch 都 `block(move || shell.fetch(...))` → `std::process::Command::output()` **阻塞**起一个 `rsync -a` 子进程并等它结束；
3. N 个数据文件 → N 次 rsync 子进程、N 次串行阻塞等待；`max_concurrency` 配置（`StandbyConfig.max_concurrency`，`:57`）在同步路径里**根本没有被使用**。

fetch 用 `rsync -a`（`rsync.rs:81`），**没有 `--inplace`**，目标写到 `.tmp` 再 rename（`lib.rs:225,235`）。

## C++ 实现
C++ 对每个分区**只起一次 rsync**：`rsync -a --inplace --include=data_* --exclude=* <remote_partition>/ <dest>/`（`standby_service.cpp:706-713`），由 rsync 自身在一次 ssh 会话内增量传输整个目录。子进程用 `SpawnChild`（`:1190`）`fork`+`exec`，pidfd 注册到 epoll，`HandleExitedChildren`（`:966`）异步 `waitpid` 回收；数据 rsync 与 manifest 准备（`RunPrepareManifestJob`）分阶段推进。多分区并发由 supervisor 的 ready queue 调度（`max_concurrency`）。

## 差异与风险
- **进程/会话数放大 O(N)**：Rust 每文件一次 rsync（N 个文件 → N 次 ssh/rsync 会话、N 次进程 fork/exec）；C++ 每分区一次。文件数多时，进程启动与 ssh 握手开销远大于数据传输本身。
- **阻塞线程**：`std::process::Command::output()` 在 `spawn_blocking` 线程里阻塞，D34 已声明这是 compio-process 之前的临时退化；但当前连“每分区一次、异步监管”都没有，shard 线程被 N 次串行 rsync 拖住。
- **无 `--inplace`**：Rust 先写 `.tmp` 再 rename（多一倍临时磁盘占用 + rename 开销）；C++ `--inplace` 直接原地更新。
- **并发配置未接线**：`max_concurrency` 存在但同步路径未使用，多分区/多文件无法并行。
- 行为差异：C++ `--include=data_*` 只同步 data 文件（不含 segment）；Rust `is_data_file`（`:327-329`）把 `data_` 与 `segment_` 都算进去，segment 会被同步——这是 Rust 更全，但与 C++ 分区 rsync 的过滤口径不同。

## 对齐建议
- 把“每文件一次 rsync”改为“每分区一次 rsync -a --inplace --include=data_* --exclude=*”，对齐 C++ `StartRsyncJob`；用 `max_concurrency` 控制并发分区数。
- 按 D34 路线把 `CommandRunner` 从 `std::process` 迁到 `compio-process`（或至少把 `fetch` 的阻塞 `output()` 换成异步 spawn + 监管），消除串行阻塞。
- 若保留逐文件 fetch（便于 stage+rename 原子安装），至少用 `max_concurrency` 并发化，并评估是否加 `--inplace`。
- 明确 segment 是否纳入 standby 同步口径，与 C++ 对齐或记录为有意扩展。

## 性能优化建议
- 每分区一次 rsync + 并发化 + `--inplace`，可把 standby 追赶时间从 O(N×单文件会话开销) 降到 O(分区数×会话开销)，大分区/多文件场景收益显著。

## 严重度
P2（standby 同步性能 / 并发未接线 / 进程退化）

## 审查结论（2026-11-04）

**采纳（P2），转 todo**。

- 成立：逐文件起 rsync、`max_concurrency` 未使用、无 `--inplace`。
- 与 `io-10` 的「遗留：目录级两阶段 rsync」重复，本 issue 是执行项。
- 需要改 `RemoteSource` 增加分区级 `sync_dir`（或让 `RsyncSource` 直接
  跑目录 rsync），并保留 `LocalDirSource` 作为测试后端。
- 阻塞：`compio-process` 尚未引入；先用 `spawn_blocking` + 每分区一次
  rsync 即可消除 O(N) 进程。

## DONE（2026-11-04）

**实现：每分区一次传输 + `max_concurrency` 接线。**

改动文件：

- `crate/io/fkv_standby/src/lib.rs`
  - `RemoteSource` 增加 `sync_dir(tbl, dst, compare)`：一轮一次分区级传输，
    与 `compare`（本地分区目录）相同者不传，增量语义对齐 rsync `--compare-dest`。
  - `sync_table` 重写：`list` → `sync_dir` 到 staging → 逐文件 rename 安装
    （staging 里**存在与否**即传输端的增量判定结果，缺席计 `data_skipped`）
    → manifest 单文件 fetch + 校验 + 最后安装。逐文件 fetch 只留给 manifest。
  - 新增 `StandbyService::sync_all(&[TableIdent])`：按 `max_concurrency` 分批
    并发同步多分区（`futures::future::join_all`，无额外运行时依赖），
    对齐 C++ supervisor 的 ready queue。
  - `StandbyConfig::staging_dir`；`sync_table` 起始清空 staging（`--inplace`
    只在空目录原地写）；staging 清理统一走 `remove_dir_async`。
  - 表名拼路径前用 `is_safe_file_name` 校验：`remove_dir_all` 之前必须挡住
    `../` 逃逸。
  - quick-check 合并为 `unchanged_sync` + `mtime_secs`（`list_local_file_li`
    复用），`LocalDirSource::sync_dir` 用 `copy_changed_data_file` 复刻
    `--compare-dest`。
- `crate/io/fkv_standby/src/rsync.rs`
  - `RsyncShell::sync_dir`：`rsync -a --inplace --compare-dest=<local>/ 
    --include=data_* --include=segment_* --exclude=* <remote>/ <staging>/`，
    常量表 `RSYNC_ARCHIVE/RSYNC_INPLACE/RSYNC_COMPARE_DEST/RSYNC_INCLUDE/RSYNC_EXCLUDE_ALL`；
    ssh 场景只一次会话（原来 N 个文件 N 次 ssh 握手）。
  - `parse_modified` 补注释：rsync 输出为 sender 本地时区，绝对值只用于同一
    列表内的相对比较（manifest 同 term 平局），不参与 mtime 判定。
- `crate/io/fkv_standby/tests/standby.rs`：`one_partition_transfer_per_sync`
  （6 个 data 文件仍只 1 次 `sync_dir` + 1 次 manifest `fetch`）、
  `sync_all_catches_up_every_partition`（3 分区、max_concurrency=2）、
  `rsync_sync_dir_command_shape`（断言 rsync 参数形状）、
  `rsync_local_source_syncs_real_store` 补第二轮断言（`data_fetched == 0`，
  `--compare-dest` 命中）。
- `Cargo.lock` / `fkv_standby/Cargo.toml`：`cargo add futures --no-default-features
  --features alloc`。

顺带修掉的隐患：原 `unchanged()` 拿 rsync `--list-only` 的本地时区时间戳与
本地真实 mtime 比较，时区非 UTC 时永远判「已变」（即每轮全量重传）。现在
增量判定以传输端为准，该比较仅保留在本地后端（真实 epoch，口径一致）。

有意保留的与 C++ 差异：

- 仍走 staging + 原子 rename（C++ 是 `--inplace` 直写 table 目录）：保留
  「本地 manifest 绝不指向半成品 data」的不变量与既有测试。
- include 口径含 `segment_*`（C++ 只 `data_*`）：Rust 侧 segment 同属分区数据，
  比 C++ 更全，非缺项。
- `compio-process`（pidfd / epoll 异步监管）仍未引入：进程等待依旧在
  `spawn_blocking`；本 issue 的进程数 O(N)→O(1) 目标已达成。

测试命令与结果：

- `cargo nextest run -p fkv_standby`：13 passed, 0 skipped（含真实 rsync 集成测试）。
- `cargo clippy -p fkv_standby --no-deps --all-targets -- -D warnings`：零警告。
- `cargo fmt -p fkv_standby -- --check`：干净。

## 复审（2026-11-05 r6）

**通过。** 核实：

- `fkv_standby/src/lib.rs:140` `RemoteSource::sync_dir`（分区级一轮传输）、:319-333 staging + `sync_dir(compare=local_dir)` 增量、逐文件 rename 安装；:434 `sync_all` 用 `join_all` 按 `max_concurrency` 分批并发（:28 futures 导入）；:210/224/306 `is_safe_file_name` 挡路径逃逸。
- `fkv_standby/src/rsync.rs:13-15` `RSYNC_INPLACE/RSYNC_COMPARE_DEST` 常量、:91-103 `sync_dir` 命令形状（`-a --inplace --compare-dest= --include=data_* --include=segment_* --exclude=*`）。
- 测试 `tests/standby.rs:143` `one_partition_transfer_per_sync`、:198 `sync_all_catches_up_every_partition`、:475 `rsync_sync_dir_command_shape`、:274/579 `data_skipped` 增量断言。
- 记档的三条有意差异（staging+rename、含 segment、仍 spawn_blocking）在 md 中明示，判定合理，不判回归。
