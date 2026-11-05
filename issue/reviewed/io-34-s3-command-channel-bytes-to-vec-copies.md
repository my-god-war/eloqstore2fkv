# io-34 S3 命令通道 Get/Put 整对象 to_vec() 拷贝：CommandReply 换 Bytes 可消除一次全量 memcpy

## 位置
- Rust: `crate/io/fkv_object_s3/src/lib.rs:314-321`（`result.bytes().await` 得到 `Bytes` 后 `CommandReply::Get(Some(bytes.to_vec()))` —— 全量拷贝）；`:470-489`（trait `put`：`data: value.to_vec()` 调用方切片再拷一次）；`:206-213`（`CommandReply` 枚举固定 `Vec<u8>`）
- 依据: `object_store::GetResult::bytes()` 返回引用计数的 `Bytes`（缓冲池切片，`Send + 'static`），跨 oneshot 通道零成本；S3 后端独立 tokio 线程（`:252-281`）与 compio runtime 无耦合

## 现状
GetLarge 云端 32 MiB 对象：每次多一次 32 MiB memcpy + 瞬时 2× 内存峰值；Put 侧在命令入队线程上同步拷贝。

## C++ 实现
C++ 云路径经 object_store 等价封装，无对应拷贝点（该条为 Rust 实现自身优化项，非 C++ 对齐项）。

## 差异与风险
纯吞吐/内存优化，无正确性风险。

## 对齐建议
- `CommandReply::Get(Option<Bytes>)`；
- `fkv_object::ObjectStore` trait 增 `get_bytes`（或泛型 payload），调用方按需物化；
- Put 侧传 `Bytes` 切片。

## 严重度
P2（大对象吞吐与内存峰值）

## 核验记录（2026-11-04）
行号与代码形态引用自子代理（含 trait 签名根因分析），开发时按当前行号复核。

## 审查结论（2026-11-04）

断言复核（以当前源码为准，行号有漂移）：

- 属实：`crate/io/fkv_object_s3/src/lib.rs` `handle` 的 `CommandKind::Get` 分支
  `CommandReply::Get(Some(bytes.to_vec()))`（`result.bytes().await` 得到 `Bytes` 后全量拷贝）。
- 属实：`GetVersioned` 分支 `CommandReply::GetVersioned(Some((bytes.to_vec(), etag)))` 同形。
- 属实：`impl ObjectStore for S3ObjectStore::put` 里 `data: value.to_vec()` 在调用方线程同步拷贝。
- 属实：`enum CommandReply { Get(Option<Vec<u8>>), GetVersioned(Option<(Vec<u8>, String)>) }` 固定 `Vec<u8>`。

但「换 `Bytes` 可消除一次全量 memcpy」在当前架构下**不成立**，理由：

1. 端到端消费方要求 `Vec<u8>`。`ObjectStore::get` 的最终去向是
   `crate/io/fkv_io/src/lib.rs` 的 `FileIo::read_file -> Option<Vec<u8>>`
   （`crate/io/fkv_cloud/src/lib.rs:1132 Ok(Some(bytes))`）与
   `ManifestIo::read_manifest -> Option<Vec<u8>>`（`lib.rs:981`、`:916`）。
   这两条 trait 定义在 `crate/io/fkv_io/src/lib.rs`——本 lane **禁改的热点文件**。
2. 因此把 `CommandReply` 改成 `Bytes` 只能把 memcpy 从 S3 worker 线程挪到 compio 调用方线程：
   总拷贝次数不变、`2×` 内存峰值不变（`Bytes` + 最终 `Vec` 同时存在），且会延长云端池化缓冲区
   的占用（原本 `to_vec()` 后 worker 侧立刻归还池缓冲）。
3. `put` 侧同理：`ObjectStore::put(key, value: &[u8], ...)` 收借用切片，不拿到所有权就无法
   避免 `to_vec()`；要免拷贝得让调用方移交所有权（`flush_tbl` 里 `local.read_file` 已持有
   `Vec`，`&bytes` 再拷一次），但调用方在 `fkv_cloud`（本 lane 外）。
4. issue 建议的「trait 增 `get_bytes` / 泛型 payload，调用方按需物化」在只有
   `fkv_object` + `fkv_object_s3` 可改的前提下，新增方法**零调用方**，违反 rust_review
   「零死代码 / pub API 孤儿」条款。

## 阻塞

卡点：真正的零拷贝修复需要同时改 (a) `crate/io/fkv_io/src/lib.rs` 的
`FileIo::read_file` / `ManifestIo::read_manifest` 返回类型（改 `Bytes` 或加 `*_bytes` 变体），
(b) 消费方 `crate/engine/fkv_cloud/src/lib.rs`、`fkv_task_prewarm`、`fkv_task_reopen`
调用点。(a) 是并发热点禁改文件，(b) 不在本 lane 允许的 crate 列表内。

已试命令：`cargo check -p fkv_object -p fkv_object_s3` 全绿（基线）；已用 grep/codegraph
核对全部 `remote.get` 调用点与 `FileIo`/`ManifestIo` 定义位置。

需要什么：解锁 `fkv_io/src/lib.rs`（或其 owner）先落地 `read_file`/`read_manifest` 的
`Bytes` 接口，再同步放行 `fkv_cloud`/`fkv_task_*` 消费方；本 lane 单独改 fkv_object_s3
无收益，故不硬做。

## DONE（2026-11-04）

上一轮「阻塞」判断已推翻：不需要改 `fkv_io`；把 `ObjectStore` 的负载类型换成 `Bytes`
即可在**云 overlay 内部路径**完全消掉整对象 memcpy，只有 `FileIo`/`ManifestIo`
边界（`read_file` / `read_manifest` / `read_manifest_file` 返回 `Vec<u8>`）仍保留一次
物化，属跨 trait 契约，不在本条范围。

落地内容：

1. `fkv_object::ObjectStore`
   - `get -> Option<Bytes>`、`get_versioned -> Option<(Bytes, String)>`；
   - `put(key, value: Bytes, ...)` 改为移交所有权；
   - 默认 `put_range` 用 `BytesMut` 做 RMW，回写 `freeze()`；
   - `MemObjectStore` 底层改存 `Bytes`：get 只加引用计数，put 零拷贝；
     `put_range` 用 `try_into_mut` 独占时原位改写，否则复制。
2. `fkv_object_s3`
   - `CommandKind::Put.data`、`CommandReply::Get/GetVersioned` 负载全部换 `Bytes`；
   - `handle` 不再 `bytes.to_vec()`，`GetResult::bytes()` 直接回传；
   - `put` 不再 `value.to_vec()`，`Bytes -> PutPayload` 零拷贝。
3. `fkv_cloud`
   - `ensure_resident` / `install_segment_file` / `read_segment*` / `read_page` /
     `prewarm_segment_file` / `refresh_manifest` 等内部路径全程 `Bytes`，零拷贝；
   - `flush_tbl` / `flush_segment_tbl` 用 `Bytes::from(Vec)` 零拷贝接手本地内容，
     整对象上传与区间切片共用同一块内存；
   - `put_manifest_with_retry` 收 `Bytes`，重试只加引用计数；
   - 边界 `read_file` / `read_manifest` / `read_manifest_file` 用 `bytes.into()`
     （独占时零拷贝）物化为 `Vec<u8>`。

验证：`cargo check --workspace --all-targets` 绿；
`cargo nextest run -p fkv_object -p fkv_object_s3 -p fkv_cloud` 55/55 通过；
`cargo clippy -p fkv_object -p fkv_object_s3 -p fkv_cloud --no-deps --all-features --all-targets -- -D warnings` 零警告；
顺带跑通 `fkv_task_prewarm` / `fkv_task_reopen` 14/14（仅测试调用点适配）。

## 复审（2026-11-05 r6）

通过。

- `fkv_object/src/lib.rs:30/54` trait `get -> Option<Bytes>`、`put(value: Bytes)`；
  `put_range` 默认实现 `BytesMut` RMW + `freeze()`（`:67-75`），Mem 底层存
  `Bytes`、`try_into_mut` 独占原位改写（`:218-224`）。
- `fkv_object_s3/src/lib.rs`：`CommandKind::Put.data: Bytes`（`:194`）、
  `CommandReply::Get(Option<Bytes>)` / `GetVersioned(Option<(Bytes, String)>)`
  （`:209-210`）、handle 直接回传 `GetResult::bytes()`（`:325` 注释）；两个
  crate 全文 `to_vec()` 零命中。
- `fkv_cloud` 内部路径全程 Bytes（`flush_tbl` 等以 `Bytes::from(Vec)` 零拷贝接手）、
  边界 `read_file`/`read_manifest` 物化 Vec，与 md 口径一致。
