# sto-51 Replayer 损坏尾部从不覆盖：崩溃后一次写即永久 Corrupted

## 位置
- Rust: `crate/engine/fkv_task_reopen/src/lib.rs:17-58`（`split_manifest`，44-49 行「损坏后又见完好 record → Err(Corrupted)」）、`crate/engine/fkv_store/src/lib.rs:1289`（`install_replay_state` 把 `manifest_size_map` 记为物理 `bytes.len()`）
- C++: `src/replayer.cpp:96-114`（checksum 失败记录 `file_size_before_corrupted_log_`）、`src/replayer.cpp:69-72`（Replay 结束回退 `file_size_`）、`src/tasks/write_task.cpp:819`（`AppendManifest(blob, manifest_size)` 定点写）、`src/async_io_manager.cpp:2791-2846`（`IouringMgr::AppendManifest` 在 `offset` 处 `Write`，非 EOF append）

## 现状
Rust replay 正确识别并跳过 checksum 损坏的 record（`split_manifest:33-43`），torn tail（body 完整缺 padding）也正确丢弃（44-47 行）。但 `install_replay_state` 记录的 manifest 大小是**物理文件长度**（含损坏区），且 `CompioStore::append_manifest`（`crate/io/fkv_io_compio/src/lib.rs:1394-1427`）在缓存/物理 EOF 处追加，**从不回写、从不覆盖损坏区**。

后果链：崩溃留下 torn/损坏尾部 → reopen 成功（丢弃尾部）→ 下一次写把新 record 追加在损坏区**之后** → 再 reopen：`split_manifest` 读到损坏 record（checksum 失败、跳过、置 `corrupted=true`），紧接着读到新写入的完好 record → 返回 `Err(KvError::Corrupted)`（`fkv_task_reopen/src/lib.rs:44-49`）→ `Store::reopen` 报错、`ensure_table_ready`（`crate/engine/fkv_shard/src/lib.rs:278-298`）对 Put 不兜底 → **该分区永久不可写/不可读**。若 torn 区的垃圾 header 解析出的 payload_len 超大，则循环在损坏处 `break`，后续新 record 被完全忽略 —— 更糟：reopen 静默成功但**丢失最近一次已确认写**。

## C++ 实现
C++ Replayer 记录 `file_size_before_corrupted_log_`（`replayer.cpp:99-100`），Replay 结束把它回退给 `file_size_`（69-72 行），上层用它重置 `RootMeta::manifest_size_`。下一次 `AppendManifest(tbl, blob, manifest_size)`（`write_task.cpp:819`）按该 offset **定点覆盖写**（`async_io_manager.cpp:2846` `Write(fd_idx, log.data()+written, batch, offset+written)`），损坏尾部被下一次写就地修复，reopen 永远不会再看到「损坏 + 完好交替」。

## 差异与风险
- Rust append 位置 = 物理 EOF；C++ append 位置 = 逻辑 replay size。单进程崩溃语义不等价。
- Rust 的「torn tail + 一次写」场景：第二次 reopen 有两种结局——`Err(Corrupted)` 永久打不开，或静默丢最近一次提交。两者都违反验收标准 4（「任意 kill point 后 replay 不丢已确认写」）。
- 该缺陷在本地单 writer 每次崩溃都可能触发（写 manifest 是每批写必经路径），不是云模式边缘路径。

## 对齐建议
- `split_manifest` / `ReplayState` 增加 `valid_file_size`（最后一个完好 record 对齐后的偏移），等价 C++ `file_size_before_corrupted_log_` 语义。
- `install_replay_state` 用 `valid_file_size` 更新 `manifest_size_map`。
- `CompioStore::append_manifest` 以 `manifest_size`（逻辑偏移）为 write_at 基准，且先 `ftruncate` 到该偏移（丢弃尾部垃圾），对齐 C++ 定点覆盖。
- 增加 crash harness 测试：torn payload / 缺 padding / 中段 checksum 坏三种尾部，kill 后 reopen → 再写 → 再 reopen，断言不报错且提交可见。

## 性能优化建议
- ftruncate 后沿用现有 write buffer；offset 已知时跳过 `metadata()` 探测（已有 `manifest_offset` 缓存可复用）。

## 严重度
P0（语义不等价/数据丢失）

## 审查结论（2026-11-04）

**已覆盖（core-20，重复部分，按实际行号）**

- `split_manifest_with_len` 返回 `valid_len`（最后一个完好 record 的对齐结束偏移）：`crate/engine/fkv_task_reopen/src/lib.rs:22`
- `reopen_with_size` 返回 `(ReplayState, valid_len)`：`crate/engine/fkv_task_reopen/src/lib.rs:80`
- main manifest 的 `Store::reopen` 用 valid_len install 并调用 `reset_manifest` 截断：`crate/engine/fkv_store/src/lib.rs:1458-1463`
- `CompioStore::reset_manifest` 重置 `manifest_offset` 后 `set_len`：`crate/io/fkv_io_compio/src/lib.rs:2031`

→ issue 描述的 main manifest「torn / 损坏尾 → 一次写 → 永久 Corrupted」在本地后端已修复，该部分为重复。

**未覆盖（非重复，保留为缺口）**

- branch manifest：`open_branch` 仍用 `replay_manifest_bytes` + `bytes.len()`（`crate/engine/fkv_store/src/lib.rs:1333-1385`），且截断条件 `manifest_file.is_none()` 在此时恒为 false，分支从不截断。
- `Store::reopen` 的 branch 分支同样以物理长度 install（`crate/engine/fkv_store/src/lib.rs:1447-1463`）。
- `ManifestIo` 没有具名 manifest 截断接口；`MemStore` 连 main 的 `reset_manifest` 都未实现。

→ 剩余部分拆到 `issue/todo/sto-51a-branch-manifest-corrupted-tail-reset.md` 并已实现。

## DONE（2026-11-04）

branch manifest 尾部自愈已实现：`replay_manifest_bytes_with_len`、`ManifestIo::reset_manifest_file`、`CompioStore`/`MemStore` 截断实现、`Store::open_branch`/`reopen` 按 valid_len 截断。测试与 clippy 通过。

## 复审（2026-11-05 r6）

通过。main 路径修复在位：`fkv_task_reopen/src/lib.rs:21`（`split_manifest_with_len` 返回
`valid_len`，:57 `valid_len = next` 只记完好 record）、`:79-92`（`reopen_with_size`）；
`fkv_store/src/lib.rs:1726-1727`（`Store::reopen`：branch 走 `reset_manifest_file`、main 走
`reset_manifest`，按 valid_len 截断）。branch 侧见 sto-51a。测试
`fkv_store/tests/compio_manifest.rs:89`（`compio_reopen_truncates_corrupted_tail`）存在。
