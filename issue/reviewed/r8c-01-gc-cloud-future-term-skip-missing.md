# r8c-01 P3：云端 FileGc 缺 C++ 的「候选 term > process_term」三层护栏，未来 term 写者的文件可能被本实例删除

## 现象（亲读）

Rust GC 判定链（`crate/engine/fkv_task_gc/src/lib.rs`）：

- `retain_variant`（:444-470）：damaged → pin → 归属命中 → attr_failed → 外部引用
  → `variant_inflight`，**没有任何一处比较候选文件的 term 与本进程 term**。
- `variant_inflight`（:480-489）：`term >= guard.newest_term && file_id >= bound`，
  两个条件都真才保留。
- 候选循环 `gc_data_files`（:869-889）/ `gc_segment_files`（:969-983）同样无
  term 门控；云端与本地共用同一条判定（`GcScope` 只分叉列举/删除方法）。
- 本进程 term 唯一进入点是 `fkv_store/src/lib.rs:3889` / `:3902` 的
  `guard.seed(&self.branch_name, self.term, ...)`：只抬 high-water，不做「高于本
  term 的一律不动」。

推演（分裂脑 / failover 重叠窗口）：另一写者以**更高 term** 写入同一 branch，
但其 allocator 视图旧于本地最新 manifest 高水位（其 file id < 本地 bound）时：
`term >= newest_term` 真、`file_id >= bound` 假 → 不 in-flight → 无引用即删。
C++ 在同样输入下保留（见下）。

## C++ 对照

`src/file_gc.cpp` 云端路径有三层护栏，本地路径（`DeleteUnreferencedLocalFiles`
:1140 起）确无此护栏（Rust 本地/内存后端不受影响）：

1. `DeleteUnreferencedCloudFiles`（data）：`:973-977`
   `if (term > process_term) { /* Skip files with term greater than process term. */ continue; }`
   ——逐候选无条件保留。
2. 云端 segment 删除循环：`:1426-1428` 同款跳过。
3. `ExecuteCloudGC` 圆级熔断：`:1554-1560` 列举出的 manifest 里任一
   `term > process_term` → 整轮 GC 直接返回 `ExpiredTerm`（注释：check if term
   expired to avoid deleting invisible files）。

即 C++ 把「存在更高 term 的写者」视为本实例无权删除任何东西的信号；
fkv 三层均无对应实现。

## 目标

云端 `GcScope::File` 删除链补齐对齐：

1. `InflightGuard` 携带 process term（或 `retain_variant` 增参），候选
   `term > process_term` 一律保留（对齐 :973/:1426）。
2. `manifest_retained_file_page_set_budget` 或 `gc_with_scope` 在列举/回放阶段
   发现 manifest term > 本进程 term 时，云端 FileGc 整轮跳过删除并回
   `ExpiredTerm`（对齐 :1554-1560）；`GcScope::Local` 不受影响（C++ 本地路径无此护栏）。
3. 若裁定不加（fkv 单写者 fencing 已覆盖常见路径），须在 rev-39 的登记段落或
   `doc/architecture/06-gaps.md` 补「有意差异」条目，写明重开条件
   （C++↔Rust 混部、分裂脑窗口）。

## 验收

- `fkv_task_gc/tests/variant.rs` 增用例：候选 `(branch, term_f)` 其中
  `term_f` 高于 seeded 的进程 term、file id 低于本地高水位 → `retain_*_variant`
  为真、一轮 GC 删除数为 0。
- 既有 `fkv_task_gc` / `fkv_io*` / `fkv_cloud` 变体用例不回归。

## 核验记录

- 亲读 `fkv_task_gc/src/lib.rs` 全文（判定链 :444-489、候选循环 :869-889、
  :969-983、`apply_manifest` :566-603、`InflightGuard` :94-116）。
- 亲读 `fkv_store/src/lib.rs` `gc_with_scope`（:3872-4010）：seed 只经
  `fold_idx` 抬水位；damaged 全跳过；`prune_superseded_manifest_file_li`
  （fkv_cloud :2379-2437）只在「远端最高 term == 本地 term」时剪旧 manifest，
  是剪枝不是删除护栏。
- 亲读 `fkv_cloud/src/lib.rs` `list_file_variants`（:2906-2916）、
  `remote_file_variant_li`（:2657-2695）：列举无 term 过滤。
- 亲读 C++ `file_gc.cpp`：`GetRetainedFiles`（:33-94）、
  `SeedActiveBranchGuardFromInMemory`（:117-159）、`ProcessOneManifest`
  （:583-659）、`DeleteUnreferencedCloudFiles`（:937-1140，护栏 :973）、
  segment 循环（护栏 :1426）、`ExecuteCloudGC` 熔断（:1554-1560）、
  `DeleteUnreferencedLocalFiles`（:1140 起，无护栏）。
- 查重：`grep -rn "process_term|未来 term|term 大于" issue/` 仅 cld-44（standby
  安装侧 fencing，已做）与 sto-55/core-22（无关）；rev-39 登记的偏离为
  「guard 缺失按可删除」「归属失败保守剪枝」，均不含本条。r7 组 3 复审
  （00-r7-group3.md）核对的是 `term >= newest_term && file_id >= bound` 本身，
  未覆盖本护栏。


## DONE（2026-11-05，第二轮复审修复）

DONE：`InflightGuard` 增加 `process_term` 门控与 `term_above` 检查；`Store::gc_with_scope` 看到更高 term manifest 返回 `ExpiredTerm`，变体 term 高于本进程时保留。

## 复审（2026-11-05 v）

通过，归档。亲读（行号以当前盘面为准）：

- `fkv_task_gc/src/lib.rs`：`InflightGuard.process_term` :123（默认 `u64::MAX` :149）；
  `seed`/`seed_segment`/`set_process_term` :255/:263/:273 注入；`term_above` :279
  （`guard_li.iter().any(|g| g.newest_term > term)`）；`retain_variant` :520 的
  `if term > self.process_term { return true; }` :532 命中前置于引用/在飞判定。
- `fkv_store/src/lib.rs`：`gc_with_scope` :4062 内 `guard.set_process_term(self.term)`
  :4106、`if guard.term_above(self.term) { return Err(KvError::ExpiredTerm); }`
  :4109-4111（在 damaged 早退 :4130 之前）。
- DONE 两项声明属实，对齐 C++ `file_gc.cpp:973/:1426`（逐候选跳过）与
  `ExecuteCloudGC:1554-1560`（整轮熔断）。测试面由 r9c-01 承接。
