# gap-12 compio 侧 kill point 位置缺口：窗口 D 无注入点，`:Sync`/`:Rename` 实际同一窗口（P2）

## 来源

为 gap-10 设计「intra-op 崩溃闭环」时对 `fkv_io_compio` 注入点逐个核位（2026-11-05，
HEAD `b5073e1`）。本条只记录位置问题，**不在本项修**。

## 现象

`CompioStore::atomic_write_file`（`crate/io/fkv_io_compio/src/lib.rs:1956`）的步骤与
注入点位置：

```
1959  File::create(&tmp)
1960  write_whole(tmp)
1961  file.sync_data()            // tmp 内容 durable
1962  kill_point("AtomicWriteFile:Sync")
1963  fail_point!("AtomicWriteFile:Sync")
1964  kill_point("AtomicWriteFile:Rename")
1965  fail_point!("AtomicWriteFile:Rename")
1966  rename(&tmp, path)
1973  sync_dir(dir)               // 失败只 warn
1981  kill_point("AtomicWriteFile:SyncDir")
```

问题两处：

1. **`:Sync`（1962）与 `:Rename`（1964）都位于 `rename` 之前**，中间没有任何状态变化，
   因此两个名字实际建模的是**同一个窗口**（tmp 已耐久、未改名）。名字给出的区分度是假的。
2. **窗口 D「rename 已完成、父目录项未 fsync」没有任何 kill point**：
   `AtomicWriteFile:SyncDir`（1981）落在 `sync_dir` 返回**之后**，属于提交后的一侧，
   不是「renamed but dir entry not durable」那一侧。

同一族的另两处（`write_file`，`:3098`）：

- 只有 `fail_point("WriteFile:SyncFail")`（`:3117`），**没有 rename 之前的 kill point**
  → 无法在真实 Windows/macOS 语义下模拟「tmp 已 fdatasync、未 rename」而进程崩溃；
- `:3131` 把目录项 fsync 显式推迟到下一次 `sync_data`（`dirty_dir`），但 `dirty_dir`
  的刷写路径（`sync_data` `:2542-2546`）**也没有 kill point** → 「文件已 rename、
  目录项未 fsync」同样不可注入。

另外 `reset_manifest` / `reset_manifest_file`（`:2557` / `:2566`）用 `set_len` 截断且
**不 fsync**，即「截断未持久」这一窗口（撕裂尾可能复活）也未建模 —— 而它恰好与
`split_manifest_with_len` 的 torn 分支配对。

## 建议

1. 把 `AtomicWriteFile:Sync` 与 `AtomicWriteFile:Rename` 中**至少一个**移到 `rename`
   之后、`sync_dir` 之前，使「tmp 已耐久未改名」与「已改名、目录项未持久」两个窗口
   各有独立名字；保留旧名语义时需在代码注释与文档里显式说明二者的等价性，不要让名字
   误导枚举式的 fuzz 驱动。
2. 给 `write_file` 补 rename 前的 kill point（与 `atomic_write_file` 对齐）。
3. 给 `dirty_dir` 的目录 fsync 路径补注入点，或在 `sync_data` 的目录分支补。
4. `reset_manifest` 的截断若不打算 fsync，应在其文档注释里写明「截断本身不持久」，
   并明确谁负责后续持久化。

## 验收

- 「tmp 已耐久未改名」与「已改名、目录项未持久」各有可独立 arm 的 kill point；
- 用 gap-10 的枚举式用例按名字集合驱动时，每个名字都对应**唯一**窗口（无重复、无空洞）；
- 若维持现状，则须在 `fkv_io_compio` 模块文档里列出「哪些名字是重复语义」，并让
  gap-11 的名字一致性检查把这份清单作为显式白名单。

## 处置（2026-11-05）—— 已实现

四处改动（`crate/io/fkv_io_compio/src/lib.rs`）：

1. **`atomic_write_file`：`AtomicWriteFile:Rename` 整对（kill + fail）移到 `rename` 之后、
   `sync_dir` 之前**，于是三个名字与三个窗口一一对应：
   `:Sync` = tmp 已耐久未改名；`:Rename` = 已改名、父目录项未 fsync；`:SyncDir` = 已提交。
   `:Rename` 的 **Fail** 动作由此变成「已提交但上报失败」这一真实组合（调用方必须能扛），
   与 `:Sync` 的「改名未发生、旧快照仍权威」互为对照。
2. **`write_file` 补 `kill_point("WriteFile:Rename")`**（`rename` 之后）：该路径的目录项
   fsync 推迟到下一次 `sync_data`，此前这个「已改名、目录项未持久」的窗口无法注入。
   该函数在仓内有调用点（`fkv_store/tests/concurrent_snapshot.rs`、`gc_inflight.rs`），
   非死代码。
3. **`sync_dir_entry` 补 `kill_point("SyncDir")`**：`ensure_dir`、`sync_data` 的 dirty
   目录分支、`append_file_at` 都走这里，因此「文件已耐久、目录项未持久」可注入。
4. **`reset_manifest` / `reset_manifest_file` 补文档**：写明 `set_len` 截断**不 fsync**，
   「截断未持久 ⇒ 撕裂尾掉电后复活」是真实窗口，reopen 必须幂等再切分。

验收对照：新增的 `SyncDir` 已纳入 `fkv_store/tests/crash_failpoint_invariants.rs` 的
名字枚举（在首次 `ensure_dir` 即命中），用例通过；`WriteFile:Rename` 不在该脚本的
6 条 op 路径上，故未纳入枚举，仅由改动 2 保证其存在与位置。

验证：`cargo nextest run -p fkv_io_compio -p fkv_store -p fkv_task_reopen` → 228 passed；
`cargo nextest run --workspace --all-features` → 1002 passed / 1 skipped；
`cargo clippy -p fkv_io_compio -p fkv_store --all-targets --all-features -- -D warnings` → 0。
