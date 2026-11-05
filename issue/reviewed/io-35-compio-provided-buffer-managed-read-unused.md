# io-35 compio provided-buffer 能力（File::read_managed_at + RuntimeBuilder 缓冲池）完全未用：定长 segment 读是现成适配场景

## 位置
- Rust: `crate/io/fkv_io_compio/src/lib.rs:1609-1620`（`read_segment` 自管 `BufPool`：`HashMap<usize, Vec<Vec<u8>>>` 按 len 分桶 + copy）；`:250-300`（BufPool 实现）；`crate/engine/fkv_shard/src/lib.rs:105`（用默认 `Runtime::new()`，未经过 `Runtime::builder()`）
- compio（0.19.2 即已可用，均为实际依赖版本源码）:
  - `compio-fs 0.12.1 src/file.rs:226-237`：`File` 实现 `AsyncReadManagedAt::read_managed_at(len, pos) -> Option<BufferRef>`
  - `compio-driver 0.12.5 src/sys/op/managed/iour.rs:46-62`：`IOSQE_BUFFER_SELECT + buf_group` 从内核 provided-buffer ring 取缓冲；非 iouring 后端有 fallback
  - `compio-driver 0.12.5 src/lib.rs:573-608,801-812`：`buffer_pool_size` / `buffer_pool_buffer_len` 可配，经 `RuntimeBuilder::with_proactor` 生效

## 现状
segment 定长读（256 KiB）可配置一个内核缓冲环：省掉自管池的 per-size 分桶、反复 alloc/free；仍是「一次 copy 到 dst」，与现状持平（不省 copy）。

## 局限
每 runtime 仅一个池、单一尺寸 —— 页路径（4-64 KiB）与段路径（256 KiB）只能取其一，故只建议先覆盖段读。0.20-beta 无新增相关能力（已比对公共接口），不构成升级理由。

## 对齐建议
`fkv_shard` 构造 `Runtime::builder().with_proactor(ProactorBuilder::new().buffer_pool_buffer_len(segment_size)...)`；`read_segment` 改 `read_managed_at`。与 io-26（ring 配置）、io-25（ring 亲和性）落地时合并评估。

## 严重度
P3（收益中低：省池管理开销，不省 copy）

## 核验记录（2026-11-04）
compio 侧 API 存在性与版本口径引用自子代理（其声明已实读 0.19.2 registry 源码并与 0.20-beta 比对）；fkv 侧行号开发时复核。

## 审查结论（2026-11-04）

**判定：合理，落地实现（仅定长 segment 读，带完整回落）。**

逐条核验：

| 项 | 核验结果 |
|---|---|
| `compio-fs 0.12.1` | 属实。`src/file.rs:224-237` `impl AsyncReadManagedAt for File`，`Buffer = BufferRef`，返回 `Option<BufferRef>`；非 `cfg` 门控，所有后端可用。 |
| `compio-driver 0.12.5` | 属实。`src/sys/op/managed/iour.rs:46-62` 走 `BUFFER_SELECT + buf_group`；`src/sys/op/managed/fallback.rs:14-21` polling 后端用池内缓冲 + `ReadAt` 回落，池空时 `pop()` 报错。 |
| `ProactorBuilder` 配置 | 属实。`src/lib.rs:801/819` `buffer_pool_size` / `buffer_pool_buffer_len`，默认 8 块 × 8192 B，首次 `buffer_pool()` 才惰性分配。 |
| fkv 行号 | 有漂移：`BufPool` 定义在 `src/lib.rs:400`，`read_segment` 在 `:2135`；`fkv_shard` 已改用 `Runtime::builder().with_proactor(...)`（`src/lib.rs:104-126`），不是裸 `Runtime::new()`。 |

收益修正（原 issue 写「只省池管理」偏保守）：
- io_uring 侧是**内核注册的 provided-buffer ring**（等价读路径的 fixed buffer）：缓冲常驻、不再每次 read 做 get_user_pages/pin，收益大于单纯省 HashMap 分桶；
- 仍然是一次 `BufferRef -> dst` copy，与现状一致（issue 自述属实）。

限制确认：
- 每 runtime 单池单尺寸，故只覆盖 segment 读，page 读保持自管 `BufPool`；
- `io-21` O_DIRECT 段读走 `DirectBuf`（要求 4 KiB 对齐），managed 缓冲不保证对齐，**保留 direct 分支不变**，两者不冲突；
- `io-27` 是写路径 fixed slot，与读路径无交集，不冲突；
- 池耗尽 / 池尺寸不足 / 不在 runtime 上下文时 managed 返回 `Err` 或短读，必须回落自管池，不能把优化变成正确性风险。

落地范围（见下方 todo）：
1. `fkv_io_compio`：`CompioStore::with_managed_segment_read()` + `read_segment` 非 O_DIRECT 分支先试 `read_managed_at`，`buf.len() == len` 才采用，否则回落。
2. `fkv_shard`：`build_runtime` 按 `segment_size` 配 `buffer_pool_buffer_len`，backend 同步启用开关；段尺寸超过 4 MiB 不启用（避免池常驻内存失控）。
3. 测试：池配置正确时读路径不触碰自管 `read_buf`（证明走了 managed）；池过小时仍能读对（证明回落）。

## DONE（2026-11-04）

### 落地

1. `crate/io/fkv_io_compio/src/lib.rs`
   - 新增 `managed_segment_read_supported(segment_size)`：`0 < size <= 4 MiB` 才启用
     （runtime 池单尺寸常驻 8 块，过大段尺寸常驻内存与收益不成比）。
   - `CompioStore` 新增 `managed_seg: Cell<bool>` 与 `with_managed_segment_read()`；
     须在 `with_segment_config` 之后调用。
   - `read_segment` 非 O_DIRECT 分支先试 `File::read_managed_at(len, offset)`：
     `Ok(Some(buf)) && buf.len() >= len` 才 `copy` 到 `dst` 返回；池未配置 / 耗尽 /
     短读 / 不在 runtime 上下文时静默回落自管 `BufPool`，正确性不受开关影响。
   - O_DIRECT（io-21）分支保持原 `DirectBuf` 不动：managed 缓冲不保证 4 KiB 对齐。
2. `crate/engine/fkv_shard/src/lib.rs`
   - `build_runtime(io_queue_size, segment_size)` 在两个分支（tuned ring / capacity-only
     回落）都按 `segment_size` 配 `ProactorBuilder::buffer_pool_buffer_len`，且仅在
     `managed_segment_read_supported` 为真时配置。
   - 池大小沿用 compio 默认 8 块，恰好匹配 `SEGMENT_BATCH_NUM = 8` 的批内并发。
3. `crate/engine/fkv_shard/src/backend.rs`：Local / Cloud 的 `CompioStore` 链上
   `.with_managed_segment_read()`。
4. 新增 `crate/io/fkv_io_compio/tests/managed_read.rs`
   - `managed_segment_read_skips_self_pool`：池 64 KiB 时读 64 KiB 段，数据一致且
     `buf_alloc_num` 不变（证明未走自管读池）。
   - `managed_segment_read_falls_back_when_pool_too_small`：默认 8 KiB 池读 64 KiB
     段仍数据正确，且自管池发生一次分配（证明回落生效）。

### 验收结果

- `cargo check -p fkv_io_compio -p fkv_shard`：通过。
- `cargo nextest run -p fkv_io_compio -p fkv_buf -p fkv_shard --no-fail-fast`：
  51/51 通过。
- `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`：
  零警告；`fkv_shard` 同口径亦零警告。
- 新测试 `cargo fmt --check` 无差异；本 issue 触碰的代码块无格式漂移。

### 遗留

- page 读（4-64 KiB 多档）不接 managed：单尺寸池与之不匹配，收益不成立。
- io-26（ring 配置）/ io-25（ring 亲和性）落地时复核池参数；若 shard 并发读段超过
  8 路，多余请求回落自管池（正确但有双读开销），届时再评估加大 `buffer_pool_size`。
- 未测 io_uring 真机路径（本机 macOS 走 polling 回落）；Linux 的
  `IOSQE_BUFFER_SELECT` 分支由 compio 自身覆盖，fkv 侧只依赖其返回长度语义。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:80-82 managed_segment_read_supported`
  （0 < size ≤ 4 MiB）；`managed_seg: Cell<bool>`（`:641`）+
  `with_managed_segment_read()`（`:888-893`，按最终 segment_size 判定）。
- `read_segment` 非 direct 分支先试 `read_managed_at`，`buf.len() >= len` 才
  采用，否则静默回落自管池（`:2352-2358` → `take_read_buf:2359`）；direct
  分支保持 DirectBuf（`:2325-2348`）。
- `fkv_shard/src/lib.rs:248/261` 两个 build_runtime 分支均按 segment_size 配
  `buffer_pool_buffer_len`；`backend.rs:25 .with_managed_segment_read()`。
- 测试 `tests/managed_read.rs:18 skips_self_pool`（buf_alloc_num 不变）、
  `:41 falls_back_when_pool_too_small`（回落后数据正确）。
