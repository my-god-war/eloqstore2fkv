# eng-88 P3：`InflightGuard::RangeEntry.term` 写后不读——sto-54 声称的 (branch,term) 归属维度未兑现

来源：第 5 轮 read/gc lane 侦查（sto-54 收尾缺口的代码证据；子代理摘录）。

## 证据

`crate/engine/fkv_task_gc/src/lib.rs:45-50/191-223`：

```rust
struct RangeEntry { max: FileId, branch_idx: u32, term: u64 }
...
fn entry_inflight(&self, entry: RangeEntry, file_id: FileId, kind: FileKind, ...) -> bool {
  match kind.guard_bound(&self.guard_li[entry.branch_idx as usize]) {
    Some(bound) => file_id >= bound,   // term 未参与任何判定
```

`term` 只在 `sort_chain` 去重时取 max（:238），`retain`/`entry_inflight` 均不消费。
C++（`file_gc.cpp:1207-1208`）保护条件是
`term >= it->second.newest_term_ && file_id >= it->second.least_unflushed_file_id_`
——term 维度真实参与判定。

## 问题

todo 里的 `sto-54` 审查结论声称「(branch,term) 维度落在 in-flight 归属链上」，
实际代码未兑现：对「term 更新但 file id 落在区间链中部」的文件没有 C++ 那层
保护（现实中靠 allocator bump + 末端兜底掩盖）。

## 目标

并入 sto-54 开发（不单独开发）：`entry_inflight` 补 term 维度判定（对齐 C++
`newest_term_` 语义），或把 sto-54 文档中该句修正为实际行为。
本文件仅作证据留档，避免 sto-54 开发时按错误结论验收。

## 验收

- sto-54 的验收清单加一条：`entry_inflight` 与 C++ `file_gc.cpp:1207-1208`
  条件逐项对齐（含 term 维度），或书面记录偏离理由。

## 依赖 / 重叠

- 严格属于 sto-54（todo）的补充证据，非新开发项。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上；sto-54 开发时验证。

## 审查结论（dragon 复核，2026-11-04）

**成立，且比原描述更彻底：`(branch, term)` 的两个 term 字段都是死的。**

- `RangeEntry.term`：只在 `add_ranges`（`:126-131`）写入、`sort_chain`（`:235-238`）
  里取同 `(branch, max)` 的最大值；`retain` / `entry_inflight`
  （`fkv_task_gc/src/lib.rs:170-223`）**零读取**。
- 顺带发现：`BranchGuard.newest_term`（`:109/113`）同样只写不读 ——
  `fold_idx` 更新它、`guard_bound` 只返回 file id 边界。也就是说 C++
  `term >= it->second.newest_term_` 这一半条件整体缺失，不只是 `RangeEntry`。
- C++ 侧条件确认（`eloqstore @ 82b774c`，`src/file_gc.cpp:1207-1208`）：
  `term >= it->second.newest_term_ && file_id >= it->second.least_unflushed_file_id_`。

**已更正 `done/sto-54` 中「(branch, term) 维度落在 in-flight 归属链上」这句**
（那句为假），并在原文件留了更正块。

## 阻塞 / 归口（留 todo）

`sto-54` / `sto-54a` 均已 done，本条的残留需要新 owner，故留在 todo。落地需要跨
lane 批次：

1. 判定点拿不到**候选文件自己的 term**：`gc_files_with_retained`
   （`:327-345`）只用 `io.list_file_id_li(tbl) -> Vec<FileId>`，term 得从文件名
   （`parse_data_file_name`）或 `FileIo` 新增的 `(file_id, branch, term)` 列举接口取；
   `FileIo` 的实现在 `fkv_io` / `fkv_io_compio` / `fkv_io_mem`（外加 `fkv_cloud`），
   这些是**另一会话的热点**，改 trait 签名必须同一批次落地。
2. 判定式改为 `candidate.term >= guard.newest_term && file_id >= bound`
   （data / segment 各自一套 guard），并补「term 更新但 file id 落在区间中部」
   的用例 —— 现有测试覆盖不到（现实中被 allocator bump + 末端兜底掩盖）。
3. 替代方案（若判定成本大于收益）：把这些 dead 字段删掉并把 `sto-54` 的措辞
   固化为止式偏离记录；**不允许**保留「字段在但语义没实现」的中间态。

## DONE（2026-11-04，本文档部分）

证据核对完成、`done/sto-54` 的错误结论已更正；开发项归入 todo（上面三条）。

## DONE（2026-11-05）

改动：

- `crate/engine/fkv_task_gc/src/lib.rs`
  - `entry_inflight` 真正消费 `RangeEntry.term` 与 `BranchGuard.newest_term`，
    对齐 C++ `file_gc.cpp:1207-1208` 的 `term >= newest_term_`：归属区间
    `term` 早于 branch 最新 term 且区间上界已越过该 branch 最新高水位时，
    该区间按旧 term 已 complete，不再按 file id 继续兜底保留。
  - 安全边界：候选文件自身 term 不可得（`FileIo` 只列 `FileId`），用归属
    区间 term 作代理。新 term allocator 从旧上界之后起步，in-flight 文件会
    落在区间链中部并归到旧区间；因此旧 term 淘汰只在 `entry.max > bound`
    （新 term 明确没写到这里）时生效，其余仍按 `file_id >= bound` 判定。
  - 更新 `BranchGuard` / `RangeEntry` 文档，说明 term 维度与上述偏离。
- `crate/engine/fkv_task_gc/tests/inflight.rs`
  - `guard_ignores_mid_chain_range_from_older_term`：file id 150 落在区间链
    中部（100 < 150 < 200），归属 feature term 5 旧区间；branch 已到 term 9
    且只写到 80 → 按 term 淘汰（旧代码误保留）。
  - `guard_keeps_mid_chain_file_of_unflushed_new_term`：term 9 只有 in-memory
    seed（bound 101），旧区间上界 100 未越过新高水位 → 101 必须保留，
    防止无条件 `entry.term < newest_term` 误删新 term in-flight 文件。

验证：

- `cargo test -p fkv_task_gc` → 9 passed（gc 2 + inflight 7），0 failed。
- `cargo clippy -p fkv_task_gc --all-targets -- -D warnings` → 零警告。
- `cargo +nightly fmt -p fkv_task_gc`。
- 反向验证：禁用 term 判定 → `guard_ignores_mid_chain_range_from_older_term`
  失败；改为无条件 `entry.term < guard.newest_term` →
  `guard_keeps_mid_chain_file_of_unflushed_new_term` 失败（真实 in-flight 被删）。
  两个用例共同锁定「term 参与判定但不误删新 term 写入」。

风险 / 偏离记录：

- 未改 `FileIo`（另一会话热点）。C++ 用候选文件名里的 term 精确判定；Rust
  用归属区间 term 代理，跨实例 / 云端 manifest 落后（writer 已 bump term 但
  未写 manifest）时仍可能比 C++ 保守或激进，属本次已知偏离。彻底对齐需
  `FileIo` 新增 `(file_id, branch, term)` 列举接口（含 `fkv_io_compio` /
  `fkv_io_mem` / `fkv_cloud`），本批次不做。
- 正常 allocator bump 链路下 `entry.max > bound` 不会成立，term 检查是
  stale / rollback 区间的否决条件，不改变正常 in-flight 保护。
- 按任务约束只跑 `fkv_task_gc` targeted 测试，未跑 workspace 全量。


## 复审（2026-11-05 r6）

通过。`fkv_task_gc/src/lib.rs:246-260`（`entry_inflight`）：
`if entry.term < guard.newest_term && bound.is_some_and(|bound| entry.max > bound) { return false; }`
——`RangeEntry.term` 与 `BranchGuard.newest_term` 均真实参与判定（对齐 C++
`file_gc.cpp` `term >= newest_term_` 的否定侧），安全边界（`entry.max > bound` 才按旧 term
淘汰）与 DONE 描述一致，:249-254 注释完整记录代理判定与偏离。测试
`fkv_task_gc/tests/inflight.rs:90`（`guard_ignores_mid_chain_range_from_older_term`，
断言 `!retain_data(150)` 且 `retain_data(250)`）与 `:107`
（`guard_keeps_mid_chain_file_of_unflushed_new_term`，断言 `retain_data(101)`）——
两个方向都锁死。已知偏离（FileIo 无 (file_id,branch,term) 列举、用归属区间 term 代理）
已在 DONE 记录，不重复立项。
