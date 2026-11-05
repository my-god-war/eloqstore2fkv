# io-37 P2（潜伏）：FixedBuffer 槽位 index 在飞 SQE 完成前可被回收复用——取消窗口数据错写

来源：第 5 轮 io/compio lane 侦查，协调者亲验。触发需取消机制（当前无），
定性同 io-32：「引入超时/取消（sto-62b）前必须修」。

## 证据（2026-11-05 核验）

正常路径归还正确——`crate/io/fkv_io_compio/src/lib.rs:1417-1426`：

```rust
if let Some(mut buf) = self.take_fixed_slot(slot_len) {
  ...
  let result = self.submit_fixed_write(file, &buf, bytes, offset).await;
  self.put_fixed_slot(buf);   // 完成后才归还
```

但 future 在 `.await` 挂起期间被 drop 时：`buf` 析构 → 归还钩子
（`fkv_io/src/lib.rs:317-324` `FixedBuffer::drop`）→ `FixedBuffers::release`
（`fkv_io_compio/src/fixed.rs:207+`）把 index 推回 free 表 → 下一次 `register`
用 `IORING_REGISTER_BUFFERS_UPDATE` **覆盖同槽位**（`fixed.rs:94-109`）。

compio 取消语义是「op 继续在后台完成」（compio-driver 文档明言 cancellation
is not reliable）：仍在内核队列、尚未执行的 SQE 在执行时按**当前**注册表解析
buf_index → 被换过槽位的旧 WRITE_FIXED 把**新缓冲**内容写到旧 offset
（READ_FIXED 同理污染新缓冲）。

对照：
- compio 上游 provided-buffer 池只在完成事件后 reset（`compio/compio-driver/src/buffer_pool.rs:271+`，`take()` 对未复位缓冲返回 `None`）。
- C++ 在完成回调（PollComplete/CQE 处理）里释放槽位：`eloqstore/src/async_io_manager.cpp:2105-2119`。

## 目标

槽位归还改为完成驱动：提交后即使 future 被 drop，index 也要等 CQE（或等价的
完成确认）后才回 free 表。可选实现：drop 钩子只标记 pending-release，由已有的
完成收割路径统一 `release`；或提交期间把 `FixedBuffer` 存进 op 自身
（`BufResult` 返回后归还）。注意 `write_page_group` 正常路径已有完成归还可复用。

## 验收

- 测试：起一次 fixed write 后立即 drop future，再注册新槽位写其它数据；
  断言原 offset 未被新缓冲内容污染（可用 fkv_io_sim 或 failpoint 注入延迟完成）。
- `cargo nextest run -p fkv_io_compio`。

## 依赖 / 重叠

- io-32（todo）：裸 RawFd 取消悬垂，同族不同资源；修复顺序上 io-32 在前不阻塞本条。
- sto-62b（todo，deadline/backpressure）落地前置条件。

## 核验记录

- 本人实读：`fkv_io_compio/src/lib.rs:1410-1440`、`fixed.rs:88-112/200-225`、
  `fkv_io/src/lib.rs:317-324`。
- 子代理摘录（未复核）：compio buffer_pool 行为、`async_io_manager.cpp:2105-2119`。

## DONE（2026-11-04）

落地方式：槽位归还改为完成驱动。

- `fixed.rs`：`guard_li: Vec<Option<Arc<AlignedBuf>>>` 换成 `slot_li: Vec<SlotState>`：
  `buf`（内核注册保活）+ `inflight`（已提交未收割 op 数）+ `released`（用户侧已归还）。
  `release` 只置 `released`，`inflight == 0` 才入 free 表；新增 `leave` 推进状态机。
  新增 `SlotLease`（`Weak<Mutex<Option<FixedBuffers>>>` + 表 token + index），
  其 `Drop` 即「完成确认」点。
- `FixedRead` / `FixedWrite` 增加 `_slot: Option<SlotLease>` 与 `with_slot()` builder；
  未注册槽位为 `None`，构造函数签名不变。
- `lib.rs`：新增 `lease_fixed_slot`（校验 `owns_current_ring` 后 `inflight += 1`），
  三处提交点（`submit_fixed_write` / `read_segment_fixed` / `write_segment_fixed`）
  在构造 op 前取租约。future 被 drop 时 op 由 driver 的 `Key` 持有到 CQE 收割，
  租约随 op drop；此时才可复用槽位，避免 `BUFFERS_UPDATE` 覆盖在飞 SQE 的 buf_index。

生命周期依据（复核 compio 源码）：

- `compio-driver 0.12.5`：`Key`/`ErasedKey` 经 `into_raw()` 泄漏进 `user_data`，
  由 `Driver::in_flight` 保存，仅在 CQE 到达或 ring 关闭时回收（`driver/iour/mod.rs`）。
- `compio-runtime 0.12.6 future/future.rs:69-76`：future drop 只发 `cancel`，
  op 本体（含 `SlotLease`）覆盖整个在飞窗口。
- `Proactor::cancel` 文档明言取消不可靠，故不能以「已发 cancel」作为归还条件。

测试：

- 单测 `fixed::tests::released_slot_waits_for_inflight_lease`：注册 0/1，持 0 号租约后
  `release(0)`，新注册必须拿到 2；租约 drop 后新注册拿回 0。
- 集成 `fixed::dropped_fixed_write_keeps_its_slot_until_completion`：预热后提交
  `WRITE_FIXED`（`poll!` 首轮 Pending）随即 drop future，drop buffer 后注册新槽位
  断言 index != 0，再写另一 segment，旧 segment 内容不被污染。变异验证：
  把 `release` 改成立即入 free 表后该测试失败（新槽位重新拿到 0），已回滚。
- 验证命令与结果：
  - `cargo check -p fkv_io_compio --target aarch64-unknown-linux-gnu --tests` 通过
    （宿主 macOS 下 fixed 模块仅 Linux 编译，必须加 Linux target）。
  - `cargo clippy -p fkv_io_compio --no-deps --all-features --all-targets -- -D warnings`
    宿主与 aarch64-unknown-linux-gnu 均 0 警告。
  - `cargo nextest run -p fkv_io_compio --no-fail-fast`（宿主，Linux-only 用例被 cfg）：38 passed。
  - Linux 运行：aarch64 交叉构建测试二进制，在 arm64 容器
    （`--security-opt seccomp=unconfined`，io_uring 可用）内 11 个二进制 47 项全通过，
    含新增 2 项。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/fixed.rs:138 slot_li: Vec<SlotState>`（`buf`/`inflight`/
  `released`，`:145-152`）；`SlotLease`（`:159`）+ Drop 推进状态机（`:165`）；
  `release` 只置 `released`、`inflight == 0` 才入 free 表（`:293-294`）；
  `lease`（`:302`）/ `leave`（`:318-324`）；`FixedRead/FixedWrite` 带
  `_slot: Option<SlotLease>` 与 `with_slot()`（`:359/388/446/475`）。
- `lib.rs:1437 lease_fixed_slot`（校验 owns_current_ring）；三处提交点均先取
  租约：`submit_fixed_write:1456`、`read_segment_fixed:2475`、
  `write_segment_fixed:2543`。
- 测试：`fixed.rs:615 released_slot_waits_for_inflight_lease`（持租约 release 后
  新注册拿 2，租约 drop 后拿回 0，`:648/:660`）；
  `tests/fixed.rs:99 dropped_fixed_write_keeps_its_slot_until_completion`
  （drop future 后旧 segment 内容不被新缓冲污染，`:140-145`）。
