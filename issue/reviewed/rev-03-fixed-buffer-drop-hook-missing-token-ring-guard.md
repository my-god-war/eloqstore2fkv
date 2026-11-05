# rev-03 runtime 重建后 FixedBuffer drop hook 无 token 校验、register 无 ring 归属检查,可污染新注册表/错误 ring

## 位置

- Rust(hook,缺陷主体): `fkv/crate/io/fkv_io_compio/src/lib.rs:2811-2819`(`register_fixed_buffer` 构造的 drop hook:weak upgrade → lock → `buffers.release(index)`,不校验 registry `token`)
- Rust(同类缺口): `lib.rs:2795-2810`(`register_fixed_buffer` 惰性建表:`slot.is_none()` 才 `FixedBufferRegistry::current()`,对已存在的旧 ring registry 无 `owns_current_ring` 检查即继续 `register` → `update_buffer(self.ring_fd)`)
- Rust(对照,正确做法): `fixed.rs:159-175`(`SlotLease::drop` 用 `Arc::ptr_eq(&buffers.token, &self.token)` 区分新旧表)、`lib.rs:1656-1665`(`reset_fixed_ring`:置 `slot=None`、清空池)
- Rust(状态机): `fixed.rs:132-141`(`FixedBufferRegistry.token`)、`fixed.rs:281-299`(`release`)、`fixed.rs:318-338`(`leave`/`push_free`)
- C++ 基线: `eloqstore/src/async_io_manager.cpp:212-562`(`BootstrapRing` 一次成型:ring 与 `io_uring_register_buffers` 同生命周期,不存在 ring 换代后的跨表回收问题)

## 现状

io-37 为「在飞 op 与槽位复用」建立了正确看护:op 完成(CQE 收割、op drop)才允许槽位进 free 表,且 `SlotLease` 携带 `token` 防止旧表租约误伤新表。但**用户侧 `FixedBuffer` 的归还路径没有同等防护**:

1. `register_fixed_buffer` 的 drop hook 只按 `index` 调 `release(index)`,不携带、不校验 `token`。
2. `register_fixed_buffer` 只在 `slot.is_none()` 时建新表;若 slot 中仍是旧 ring 的 registry(runtime 已重建但 `reset_fixed_ring` 尚未被 `write_page_group` 触发),直接对旧 `ring_fd` 执行 `IORING_REGISTER_BUFFERS_UPDATE`。

触发链(ring 换代是代码自认可能发生的事件,lib.rs:1751-1754 专门处理):

- 旧 FixedBuffer(如 shard 启动时注册的长生命周期大缓冲,`fkv_shard/src/lib.rs:550` 起)跨 runtime 重建存活;
- 某次 `write_page_group` 检出 `!owns_fixed_ring` → `reset_fixed_ring`(`slot=None`,池清零);
- 后续 `register_fixed_buffer` 在**新** registry 上重新注册若干槽位(写槽位池最多可把 `slot_li` 扩到 16,fixed.rs `MAX_FIXED_BUFFERS=64`);
- 旧 FixedBuffer 之后被 drop(表 evict / 重新注册替换)→ hook → `release(old_index)` 落在**新表**同序号槽位:若该槽位 `buf=Some`(正被使用)且 `inflight==0`,置 `released=true`;该槽位下一个 op 完成时 `leave → push_free`,index 进 free 表并被下一次 `register` 用 `BUFFERS_UPDATE` 覆盖 —— 而原持有者仍以为自己的 `buf.index()` 有效,继续以该 index 提交 `READ_FIXED/WRITE_FIXED`:内核按**新**注册表解析,读写落到别的缓冲。无错误码,silent 数据损坏。
- 变体:slot 仍指旧表时直接 `register`,旧 `ring_fd` 若已被另一线程(shard runtime 线程)新 ring 复用,`update_buffer` 会把本表 iovec 写进**兄弟 ring** 的 sparse 表,同样污染他人槽位。

## C++ 实现

eloqstore 的 registered buffer 与 ring 一对一(`BootstrapRing` 里随 ring 初始化注册,析构 line 185-189 随 ring 注销),fd 表/缓冲表都无跨 ring 生命周期,`CloseDirect/FreeRegisterIndex` 只在同 ring 内回收,不存在本问题域。

## 差异与风险

- 实现但不正确:token 防护只覆盖了 `SlotLease` 一侧,`FixedBuffer` drop hook 与 `register` 两处入口漏网;`release` 对「index 相同但身份不同」的槽位无辨别能力。
- 概率低(需要 ring 重建 + 旧缓冲延迟 drop + index 撞上新表在用槽位),后果是静默写错数据,与 io-37 防御的正是同一类 corruption,属防护不完整。
- 现上层(fkv_shard 每 runtime 线程一次注册、进程内不重建)踩中概率更小,但该 crate 的既定设计(`owns_current_ring` 回落、`reset_fixed_ring`)明确表示要支持换代场景,防护应闭环。

## 对齐建议

- `register_fixed_buffer` 创建 hook 时捕获当前 `token: Arc<()>`(从 registry 取),hook 内 `Arc::ptr_eq(&buffers.token, &captured)` 后才 `release(index)`——与 `SlotLease::drop`(fixed.rs:165-175)对齐。`FixedBufferRegistry` 需暴露只读 token 访问。
- `register_fixed_buffer` 在复用 `slot` 前先 `owns_current_ring()`:false 则等同 `reset_fixed_ring`(置 `slot=None`)再 `current()` 建新表,杜绝向 stale `ring_fd` 发 `io_uring_register`。

## 严重度

P2(触发条件苛刻;命中即 silent 数据损坏,建议随 io-37 系列补防)

## 核验记录

- 逐行读 fixed.rs 状态机:`release`(:281-299)无 token 参数;`SlotLease`(:159-175)有;`push_free`(:331-338)清空 `slot.buf` 并入 free 表;`register`(:223-277)对 free index 直接 `update_buffer` 覆盖。
- 读 lib.rs:`register_fixed_buffer`(:2795-2826)hook 闭包只捕获 `weak + index`;`reset_fixed_ring`(:1656-1665)仅 `write_page_group`(:1757-1760)触发;`write_segment_fixed_li/read_segment_fixed_li` 以 `owns_current_ring` 门控提交(:2894、:2965-2970),确认在飞防护存在而归还防护缺失。
- 读上层生命周期:`fkv_shard/src/lib.rs:550-572` FixedBuffer 随 shard runtime 长生命周期持有,确认「跨重建存活 + 延迟 drop」有真实载体;`fkv_cloud/src/lib.rs:2103` 每次调用注册(短生命周期)风险更低。
- 读 C++ `BootstrapRing` 与析构(:181-204):单 ring 单注册表,无对应缺陷。
- 与既有 issue 不重复:r8 系列无涉及 fixed ring 换代归还链路的条目。

## DONE（2026-11-05）

`register_fixed_buffer` 在复用 registry 前检查 `owns_current_ring()`，换代即置空重建；
drop hook 捕获 registry token，释放前 `Arc::ptr_eq` 校验，与 `SlotLease` 对齐。

## 复审（2026-11-05 v，lane B）

通过。亲读 `crate/io/fkv_io_compio/src/lib.rs:2844-2883`：复用前
`slot.as_ref().is_some_and(|b| !b.owns_current_ring())` → `*slot = None` 重建
（:2854-2859）；drop hook 先取 `let token = buffers.token();`（:2864），闭包内
`Arc::ptr_eq(&buffers.token(), &token)` 通过才 `release(index)`（:2868-2876）。
`fixed.rs:349-353` `token()` 返回 `self.token.clone()`（同一 Arc 分配，
`ptr_eq` 有效），`fixed.rs:140` token 字段、`:159-175` `SlotLease::drop` 以
`ptr_eq` 对照。DONE 声称属实，与 `SlotLease` 防护闭环。该换代 + 旧缓冲延迟
drop 场景无专项测试（P2 潜伏、触发苛刻），未单独立项（宁缺毋滥），在此记录。
归档 `issue/reviewed/`。
