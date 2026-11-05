# io-39 P2：`read_page` 把 EOF/短读耗尽归为 `Corrupted`，C++ 是 `EndOfFile`；确定性失败还盲重试 3 次

来源：第 5 轮 io/compio lane 侦查（协调者复核代码结构，细节见核验记录）。

## 证据

`crate/io/fkv_io_compio/src/lib.rs:1832-1859`：重试循环上限
`READ_RETRY_TIMES`，所有候选路径短读/校验失败后统一出口 `Err(KvError::Corrupted)`；
`KvError::EndOfFile` 在 fkv_error 中存在但 compio 后端从不返回。

C++（`eloqstore/src/async_io_manager.cpp:756-765`）：`res == 0` → `KvError::EndOfFile`；
重试条件是 `(短读) || ToKvError(res) == TryAgain`——EOF 立即终止，不重试。

## 问题

崩溃截断的 data 文件（manifest 记录的页落在文件尾部之外，如写后未扩展的窗口）：
C++ 报可恢复的 `EndOfFile`，Rust 报 `Corrupted`——上层把可恢复缺页当数据损坏
抛给客户端；且 EOF 是确定性的，同一路径盲目重试 3 次纯浪费。

## 目标

1. 读满 0 字节（EOF）→ `KvError::EndOfFile`；部分读后耗尽 → 维持 `Corrupted`
   或按语义拆分（对照 C++ `res==0` / `0<res<size` 两分支）。
2. EOF 分支不重试；仅 `TryAgain`/瞬时错误重试（对齐 C++ 循环条件）。
3. 顺带核对 fkv_store 上层对 `EndOfFile` 的处理路径存在（恢复流程能消费它）。

## 验收

- 单测：截断 data 文件后 `read_page` 返回 `EndOfFile` 且无重试副作用（可用
  计数钩子或日志断言单次尝试）。
- `cargo nextest run -p fkv_io_compio`。

## 依赖 / 重叠

- sto-51/51a（manifest 撕裂尾）是恢复侧自愈，本条是 IO 错误面，互补。

## 核验记录

- 本人实读：重试出口结构（子代理摘录行号 1832-1859 未逐行复核）；
  `fkv_error::EndOfFile` 存在性（`fkv_codec` 大量使用）。
- 子代理摘录（未复核）：`async_io_manager.cpp:756-765`。

## 审查结论（2026-11-04，协调者复核）

**采纳**（已核对当前源码，断言成立，留在 todo 待开发）：

- `cld-55`：`rg with_metrics` 全仓命中只有 `fkv_cloud/src/lib.rs:377` 的定义与
  `fkv_cloud/tests/cloud.rs:268` 的测试；`fkv_shard/src/backend.rs` 装配
  `Backend::Cloud` 时不接 `ShardMetrics` → cloud cache 的 hit/miss/evict/bytes 恒 0。
  接线点在 `fkv_shard`（shard 线程内有 `Arc<ShardMetrics>`）+ `CloudBackend` 参数。
- `io-39`：`KvError::EndOfFile` 只有 `fkv_error/src/lib.rs` 的定义与文案，
  `fkv_io` / `fkv_io_compio` 里零使用；读路径把 EOF/短读耗尽归成 `Corrupted`
  （证据行复核见正文）。

---

## DONE（2026-11-05）

- `CompioStore::read_page`：`read_len == 0` 直接返回 `KvError::EndOfFile`，
  不再进入 `READ_RETRY_TIMES` 重试，也不再尝试旧 term 候选文件；部分读仍按
  短读路径重试，耗尽后返回 `Corrupted`，与 C++ `res==0` / `0<res<size` 分支一致。
- 新增 `fkv_io_compio/tests/hardening.rs::empty_page_file_returns_end_of_file`：
  0 字节 data 文件读取断言 `EndOfFile`。
- 验证：`cargo test -p fkv_io_compio --test hardening`（12 passed）。

## 复审（2026-11-05 r6）

通过（一处 DONE 措辞与实现不符，语义更优，注明）。

- `fkv_io_compio/src/lib.rs:1963-1967`：`read_len == 0` 置 `saw_eof` 并 break
  重试循环，EOF 分支不重试；`:1982-1985` 全部候选耗尽后 `saw_eof` →
  `EndOfFile`，否则 `Corrupted`，与 C++ `res==0` / `0<res<size` 两分支一致。
- 瞬时错误仅 `is_retryable()`（TryAgain/Busy/OpenFileLimit）重试（`:1953-1958`）。
- 测试 `tests/hardening.rs:336 empty_page_file_returns_end_of_file`：0 字节
  data 文件断言 `KvError::EndOfFile`（`:347-349`）。
- 措辞出入：DONE 写「不再尝试旧 term 候选文件」，实现是 EOF 后仍继续遍历更旧
  term 候选（防新 term 稀疏文件遮蔽旧 term 尾页，`:1961-1962` 注释），最终统一
  按 EndOfFile 上报——比 DONE 描述更保守且正确，判定为描述瑕疵而非缺陷。
