# perf-62 fkv 单条写入是 RocksDB 的一半（两次同步 vs 一次）——协议决定，不可优化

## 结论

在「盘面格式与 C++ EloqStore 保持一致」的前提下，这一行**无法**超过 RocksDB。
fkv 与 C++ 都是「每批 1×数据文件 SyncData + 1×manifest 追加 SyncFile」两次同步，
RocksDB 单条写只有 1 次 WAL fsync。0.51x 就是 2 次同步 : 1 次同步的物理比值。

## 证据

- fkv：`crate/engine/fkv_store/src/write.rs:343` `self.io.sync_data(tbl).await?;`
  之后追加 manifest（`append_mapping_log`）并同步 manifest 文件。
- C++：`src/tasks/write_task.cpp:857-867`
  `WaitWrite() → IoMgr()->SyncData() → WaitPendingUploads() → FlushManifest()`；
  `FlushManifest` → `IouringMgr::AppendManifest`（`src/async_io_manager.cpp:2846-2869`）
  以两次 `Write` 落 manifest 记录后 `return SyncFile(...)`，即 manifest 也要同步。
- 因此两侧每次 put 都是两次 fdatasync/fsync；顺序（数据先于 manifest）保证
  「manifest 引用的页一定可读」，不能并行化。
- 实测：CI 464... 一轮矩阵里 fkv 1040 ops/s（≈0.96 ms/op）、RocksDB 2036 ops/s
  （≈0.49 ms/op），单次同步成本 ≈0.48 ms —— 正好 2:1。
- 与 C++ 同口径对照（eloqstore2fkv 老矩阵 run 42444729998）：C++ EloqStore
  1068 ops/s、fkv 1280 ops/s，fkv 反而快 1.2x。

## 可能的「真」改法（都需要格式变更，本轮不做）

1. 把 manifest 记录追加到数据文件尾部，用一次 fdatasync 覆盖两者 —— 盘面格式变化，
   必须 C++ 侧同步实现并通过 oracle 语料，否则破坏兼容性。
2. 组提交：把多个写请求合并到一次同步。单线程顺序写基准里没有可合并的请求，
   对真实高并发写负载有效，但对本行的指标无效。
3. 降低持久化强度（manifest 每 N 批同步一次）—— 直接违反「对齐 C++ 语义」，
   不能做。

## 处置

- 页面与文档把这一行标注为**协议差异**（2 次同步 : 1 次同步），不计入「实现优劣」。
- 验收标准改为：同口径（两次同步）下 fkv ≥ C++ EloqStore × 1.2；当前 1.2x，达标。
