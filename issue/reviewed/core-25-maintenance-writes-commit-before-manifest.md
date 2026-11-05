# core-25 维护写路径的事务纪律不一致：compact 在 manifest 落盘前 commit、失败后 abort 空转；truncate 完全不进 batch

## 位置

`crate/engine/fkv_store/src/lib.rs`：

1. `compact_file` `:1773-1816`
   - `:1785` `mapper.begin_batch()`
   - `:1796-1799` `fkv_task_compact::compact_file` 成功后立即 `mapper.commit_batch()`
   - `:1808` 之后才 `append_mapping_log(...)` 写 manifest
   - `:1809-1813` 若 manifest 失败，再 `mapper.abort_batch()` —— 但 journal 已在 `:1797` 清空，
     该调用只会返回 `false`，**回滚不会发生**。
2. `truncate` `:1312-1370`
   - `:1319-1332` 直接对 `mapper` / `segment_mapper` 逐页 `free_page`，**没有 `begin_batch`**
   - `:1355` 才 `write_manifest_record`；写失败时已释放的 mapping 不会恢复，
     而 `root_map` / `ttl_root_map` 仍指向旧 root（`:1356-1357` 在写成功后）。
3. `clean_expired` `:1412-1600`
   - `:1526` `begin_batch`，`:1571` `commit_batch`，`:1581` `sync_data`，`:1582` `append_mapping_log`
   - 与 `compact_file` 同型：manifest 失败时 mapping 已提交，`root_map` 仍是旧值。
   对照 `put_at` `:815-836`：`put_at_inner` 内部完成 sync + manifest 后才
   `commit_batch`，失败则 `abort_batch`，顺序是对的。

## 现状

- 三个维护入口（compact / truncate / clean_expired）对同一组 `PageMapper` 的提交时机各不相同：
  - put：manifest 成功 → commit；失败 → abort（正确）
  - clean_expired / compact：先 commit，再写 manifest；失败后 abort 无效
  - truncate：无 journal，失败后既不能 abort 也无法重建旧 root
- 失败条件不是理论上的：`append_mapping_log` 会做 `write_manifest_record`（写文件 + fsync），
  磁盘满、IO 错误、manifest 超限转 snapshot 时的分配失败都会返回 Err。
- 后果：
  - 当前进程内 `PageMapper` 已按新状态修改，`root_map` 仍旧；
    后续读会用旧 root 落到已 remap/free 的页，返回 `Corrupted`/`ResourceMissing`；
  - 后续写从旧 root 下降，可能把新写路由到错误的 data page；
  - 崩溃重启会回到旧 manifest（数据不丢），但进程内状态已经不一致，无法自愈到“重试一次”的语义。
- `compact_file` 的 `abort_batch` 是明确的死分支；注释“失败时保留供上层重试”只对 `dirty_li`
  成立，对 mapping/allocator 不成立。

## C++ 实现

C++ 维护任务全程操作 COW root：
- `src/tasks/background_write.cpp:100-163`（`BackgroundWrite::Compact`）：
  `MakeCowRoot()` 复制一份 `cow_meta_`，在副本上 `DoCompactDataFile` / `DoCompactSegmentFile`，
  最后 `UpdateMeta()` 写 manifest；任何一步 `CHECK_KV_ERR` 失败直接返回，`cow_meta_` 析构，
  线上 `RootMeta` 完全没动。
- 因此不存在“先改 live mapping 再写 manifest”的窗口；失败后调用方看到的是原状态，
  重试从干净状态开始。

## 建议

1. `compact_file`：把 `commit_batch` 移到 `append_mapping_log` 成功之后：
   ```rust
   mapper.begin_batch();
   let moved = compact(...).await;      // Err -> abort + return
   let root = ...;
   append_mapping_log(...).await?;      // Err -> 仍持有 journal，abort 可回滚
   mapper.commit_batch();
   ```
   注意 `append_mapping_log` 需要读取 `dirty_page_li`，回滚前不要清空；当前实现正是
   “成功后清脏”，顺序可满足。
2. `clean_expired`：同样把 `commit_batch` 移到 `sync_data` + `append_mapping_log` 成功之后；
   `root_map` 更新保持在 commit 之后。
3. `truncate`：要么用 `begin_batch` 包住两次 `free_page` 循环并在 manifest 成功后 commit，
   要么改成“先生成 snapshot record，再一次性替换 mapping/root”（等价 C++ COW）。
   当前“先 free 再写”的路径必须消除。
4. 给三个入口各加一个 failpoint 测试：在 `AppendManifest:Write` / `AtomicWriteFile:Sync`
   注入失败，断言 `mapper.max_fp_id()`、`root_map`、`dirty_li` 与调用前一致。
5. 统一用一个 `struct MaintenanceBatch<'a>` 封装 begin/commit/abort，避免三个入口再各写一套顺序。

## 严重度

P1（manifest 写失败时进程内 mapping/root 失配；compact 的 abort 是确定无效分支）

## 核验记录（2026-11-04）

`compact_file` `:1785/:1797/:1808/:1812`、`truncate` `:1319-1332/:1355`、
`clean_expired` `:1526/:1571/:1581/:1582`、`put_at` `:815-836` 的 commit/abort 顺序均按当前源码实读。
C++ `BackgroundWrite::Compact` 的 `MakeCowRoot` → `UpdateMeta` 引用自
`src/tasks/background_write.cpp:100-163` 实读。

## 审查结论（2026-11-04）

判定：**合理**，三条全部按当前源码复核成立（行号已漂移）。

1. `compact_file`：`compact(...)` 成功后立即 `commit_batch`，再 `append_mapping_log`；失败分支的
   `abort_batch` 因 journal 已清空必然返回 `false`，是确定无效分支。
2. `truncate`：逐页 `free_page` 完全在批次外，manifest 失败后 mapping / 分配器 / 空闲链表
   无法恢复，进程内读会落到已释放页。
3. `clean_expired`：`commit_batch` 在 `sync_data` + `append_mapping_log` 之前，manifest 失败
   后 `root_map` 与 mapping 失配。

落实方案（拆分 1 个 todo，单文件改动，不拆多文件）：

- 新增 `Store::begin_mapper_batch` / `Store::settle_mapper_batch` 统一
  begin/commit/abort，mapper 与 segment_mapper 一起结算。
- `compact_file`、`clean_expired`：`commit_batch` 后移到 `append_mapping_log` 成功之后；
  `changed_page_li` 的丢弃也移到成功之后（失败由 abort 恢复长度）。
- `truncate`：整表释放纳入 `begin_mapper_batch`；`snapshot_record` / `write_manifest_record`
  任一失败即 abort；`clear_dirty_page_li` / `clear_changed_page_li` / `take_garbage_file_page_li`
  移到 manifest 成功之后。
- 未采纳建议 5 的 `struct MaintenanceBatch<'a>`：其生命周期要跨越 `&mut self` 的其它字段
  借用与 `.await`，封装会引入双重借用，改用两个 `Store` 方法表达同一顺序约束，语义等价。
- 测试：新增 `crate/engine/fkv_store/tests/maintenance_batch.rs`，用 `AppendManifest:Write` /
  `AtomicWriteFile:Sync` 注入失败，断言 `max_fp_id` / `root_id` / `ttl_root_id` /
  `dirty_page_li` / `changed_page_li` 与调用前一致，且失败后可读、可重试。

## DONE（2026-11-04）

- 改动文件：
  - `crate/engine/fkv_store/src/lib.rs`：新增 `begin_mapper_batch` / `settle_mapper_batch`；
    `compact_file` / `clean_expired` 提交后移到 manifest 成功之后；`truncate` 整表释放入批，
    manifest 失败即 abort 恢复 mapping / 分配器 / root。
  - `crate/engine/fkv_store/tests/maintenance_batch.rs`（新增）：3 个 failpoint 注入测试。
- 验证：
  - `cargo check -p fkv_store` 通过。
  - `cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast`：112 passed, 1 skipped。
  - `cargo clippy -p fkv_store --no-deps --all-features --all-targets -- -D warnings`：零警告。
- 遗留：无。

## 复审（2026-11-05 r6）

结论：**通过**。三个维护入口的提交时机均已改为 manifest 成功后 commit。

- `compact_file_li`：`fkv_store/src/lib.rs:2319` `begin_batch`，compact 失败即 `abort_batch`（:2334）；`:2345` `append_mapping_log` 之后 `:2360` `settle_mapper_batch(tbl, log_result.is_ok())` —— commit 后移到位。
- `clean_expired`：`:2124` `begin_mapper_batch`；两次 `write_batch` 失败 abort（:2185）；`:2190-2194` `sync_data + append_mapping_log` 后 `:2195` settle，root 更新在 commit 之后（:2197-2200）。
- `truncate`：`:1863` 整表释放入批；snapshot/manifest 任一失败 abort（:1903/:1908）；成功后才置 NULL root 与清 dirty/changed（:1911-1926）。
- 三个 failpoint 测试齐备：`fkv_store/tests/maintenance_batch.rs:22`（compact，断言 max_fp_id/root/dirty/changed 复原 :49-52）、`:63`（truncate，:79-83）、`:91`（clean_expired，:111-116）。断言真实覆盖。
