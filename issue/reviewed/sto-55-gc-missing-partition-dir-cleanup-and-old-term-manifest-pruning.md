# sto-55 FileGc 缺收尾清理：空分区目录移除、旧 term manifest 剪枝

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:2222-2243`（`Store::gc` 仅删除 data/segment 文件）；`crate/engine/fkv_task_gc/src/lib.rs:127-163`（只删 unreferenced 文件，无目录级清理、无 manifest 剪枝）；archive 剪枝只在 `create_archive` 内（`fkv_store/src/lib.rs:1730-1761`）
- C++: `src/file_gc.cpp:338-372`（`ExecuteLocalGC` 第 5 步：data/archive 全空时 `TryCleanupLocalPartitionDir`）；`src/file_gc.cpp:1051-1068`（云 GC 删除 active branch 的旧 term manifest）

## 现状
Rust 的 GC 收尾语义：删除所有不在 retained set 里的 data/segment 文件，然后结束。被截断/compact 清空的分区目录、上一 term 遗留的旧 manifest 文件都留在磁盘上，直到下次 `create_archive` 的 `prune_archives`（只处理有 tag 的 archive，不处理普通 `manifest_<branch>_<old_term>`）或进程重启。

## C++ 实现
- `ExecuteLocalGC` 删除后重新 `ClassifyFiles`，若 `data_files.empty() && archive_files.empty()` 调用 `TryCleanupLocalPartitionDir(tbl_id)`（`file_gc.cpp:364-372`），把空分区目录（连同 manifest）整体移除，不依赖后续 root-meta 驱逐；
- 云 GC 在删除 unreferenced data 文件之外，把 active branch 且 `term < process_term` 的旧 manifest 一并删除（`file_gc.cpp:1056-1068`，注释说明只有同 branch 版本链才可安全剪枝）；
- 云 GC 在「该分区所有 data 文件都将被删」时连当前 manifest 一起删并回写 RootMeta 一致性（`file_gc.cpp:1016-1048、1104-1121`）。

## 差异与风险
- 空目录/旧 manifest 永久驻留：truncate/compact 清空的分区目录在 Rust 里永不回收，与「GC 后分区目录可被移除」的 C++ 行为不一致；
- 旧 term manifest 累积：Local 模式每次 term bump（AttachShard 新 term）都会留下一份旧 manifest 文件，Rust 的 GC/create_archive 都不清理它，长期运行目录膨胀，且这些旧 manifest 又全部进入 `manifest_retained_file_page_set` 的 replay 枚举（sto-53 的 O(n) 问题进一步放大）；
- 云模式：旧 manifest 还意味着其 retained file 集合被持续计入 GC 保护集，回收面变窄。

## 对齐建议
- `Store::gc`（或 `fkv_task_gc`）增加收尾：data/segment/manifest 全空时删除分区目录（CompioStore 层实现 `try_cleanup_partition_dir`，对齐 `TryCleanupLocalPartitionDir`）；
- 增加 `prune_superseded_manifests(tbl)`：按 active branch + 当前 term 删除旧 term manifest（云/本地一致），在 `maintain`/`gc` 收尾调用；
- 注意与 `drop_table`（删全部 manifest）和 `delete_branch`（删 branch manifest）的去重。

## 性能优化建议
- 旧 manifest 剪枝直接缩小 `manifest_retained_file_page_set` 的 replay 输入，间接缓解 sto-53。

## 严重度
P2（性能/健壮性）

## 审查结论（2026-11-04）

**判定：合理，部分实现；剩余项拆分为 55b / 55c 留在 todo。**

逐条核验（以实际源码为准）：

- `Store::gc`（`crate/engine/fkv_store/src/lib.rs`）确实只删 data/segment 文件后返回，无 manifest 剪枝、无目录收尾。
- 「旧 term manifest 累积」在本地/云端都有落点：
  - 本地 `CompioStore` 的活跃 manifest 固定为 `manifest_main_0`，`main` 不会因 term bump 产生新文件；
  - 但 `Store::open_branch` 用 `manifest_file_name(branch, term)` 生成文件名，`term > 0` 时会留下旧 term 文件；云端 `CloudStore::manifest_key` 直接按 `manifest_<branch>_<term>` 上传远端，term bump 会留下远端旧对象。
- core-23（in-flight guard）与 eng-78（garbage 列表）已落地；本轮改动不改 retained 判定，只做删除后的收尾，因此不与 guard 冲突。
- 剪枝顺序必须放在数据文件删除之后：本轮 retained set 仍包含旧 manifest 引用，下一轮 GC 再回收只被旧版本引用的文件，避免删除窗口。

实现范围（55a）：

- `ManifestIo::prune_superseded_manifest_files` 默认实现：仅同 active branch、无 tag、`term < 当前 term`，且必须先看到当前 term manifest 才删；`main` 与 `term == 0` 直接跳过。云端 `CloudStore` 覆盖为 no-op（远端删除另议）。
- `FileIo::try_remove_empty_partition_dir` 默认 no-op；`CompioStore` 实现为「目录有任一条目就保留，只 `remove_dir` 不递归」。
- `Store::gc` 尾部调用上述两步。

未完成（拆分保留）：

- `sto-55b`：云端旧 term manifest 的远端对象剪枝（涉及跨实例可见性与误删风险）。
- `sto-55c`：truncate 后活跃 manifest 仍在，目录不会被视为空；删除活跃 manifest 需同步重置 `Store` 的 manifest 状态，超出本轮保守删除范围。

## DONE（2026-11-04）

`sto-55a` / `sto-55b` / `sto-55c` 均已完成：本地旧 term manifest 剪枝 + 空目录清理、
云端被取代 manifest 剪枝、truncate 后空表目录回收。父 issue 归档。

## 复审（2026-11-05 r6）

通过（父条目）。55a/55b/55c 均已按当前源码复核，见各自 r6 复审小节。
