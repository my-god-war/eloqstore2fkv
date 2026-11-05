# r8e-03 P3：compio `open_segment_file` 把非 NotFound 打开错误折叠成 `ResourceMissing`，违反 io-41 错误码契约并使云层把本地瞬时错误误判为 cache miss

## 现象（亲读，2026-11-05 05:55 版）

`crate/io/fkv_io_compio/src/lib.rs` `open_segment_file`（:1963-1984）：

```rust
match self.open_read_file(&path).await {
  Ok(file) => Ok(file),
  Err(err) if err.kind() == ErrorKind::NotFound => { …清缓存重扫一次… }
  Err(_) => Err(KvError::ResourceMissing),        // :1983
}
// 重试臂结尾：
.open_read_file(&path)
.await
.map_err(|_| KvError::ResourceMissing)            // :1981
```

- 非 `NotFound` 的打开失败（`EACCES` → `NoPermission`、`EMFILE`/`ENFILE` →
  `OpenFileLimit`、`EIO` → `IoFail`、目标路径是目录（`EISDIR`）→ `IoFail`）
  全部折叠为 `ResourceMissing`。`map_io_err`（:719-735）建立的 errno → 错误码
  分类（含可重试判据 `is_retryable`）在这一路径被丢弃。
- 契约冲突：`fkv_io` `SegmentIo::read_segment` 的错误码契约（io-41，
  `crate/io/fkv_io/src/lib.rs:529-541`）明文「段文件不存在 → `ResourceMissing`；
  底层 IO 错误 → `IoFail`」。打开阶段的非 NotFound IO 错误属于后者。
- 同 crate `open_data_file`（:1987-2007 附近）同场景折叠为 `IoFail`——两路径
  互不一致，且 data 侧至少保住了「底层 IO 错误 ≠ 资源缺失」的契约底线。
- 上游放大（cloud 层）：`crate/engine/fkv_cloud/src/lib.rs` `SegmentIo::
  read_segment`（:1984-2009）以 `Err(KvError::ResourceMissing)` 作为 cache miss
  信号回源远端；本地 fd 耗尽（`OpenFileLimit`，可重试瞬时错误）会被当成
  「本地没有该段」发起一次无谓的远端 GET + 整文件安装（安装仍需打开写句柄，
  大概率再次失败），把可重试的本地瞬时错误变成一次远端往返 + 最终仍以
  `ResourceMissing` 形态上抛。`read_segment_fixed` 的 cloud 包装（:2033-2067）
  同构受影响。
- 消费方语义：`ResourceMissing.is_retryable() == false`；`OpenFileLimit` /
  `TryAgain` 可重试。折叠后 store / task 层的重试分类全部失真。

## C++ 对照

- C++ 打开失败的错误码保留 errno 分类（`ToKvError` 同源语义，见
  `map_io_err` 的对齐注释）；`OpenOrCreateFD` 的 `EMFILE/ENFILE` 路径上报
  `OpenFileLimit` 由上层重试，不会伪装成「文件不存在」。fkv 的 page 读路径
  （`read_page_candidate` :1566-1570）同样经 `map_io_err` 保留分类——本条
  是 segment 打开路径独有的收敛。

## 目标

1. `open_segment_file` 两个失败出口改经 `map_io_err`（或至少 `IoFail`）：
   只有「索引与重扫后都确认路径不存在」（`NotFound` 耗尽）才返回
   `ResourceMissing`。
2. 顺手把 `open_data_file` 的 `Err(_) => Err(KvError::IoFail)` 同步改为
   `map_io_err(err)`（`NoPermission` / `OpenFileLimit` 等可重试分类不再被
   摊平；`ResourceMissing`/`NotFound → Ok(None)` 的既有语义不动）。

## 验收

- 新增用例：把 segment 期望路径替换为同名目录（open 返回 `EISDIR`），
  `read_segment` 返回 `IoFail`（修复前为 `ResourceMissing`）；data 侧同构
  断言返回 `NoPermission`/`IoFail` 而非 `ResourceMissing`。
- `contract.rs` 三镜像、`hardening.rs`、cloud cache miss 用例
  （`tests/cloud.rs` 回源链路）不回归；`./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- R8 独立审查（r8e，2026-11-05）亲读 `fkv_io_compio/src/lib.rs` 全文；锚点
  行号（:1963-1984、:719-735）为 05:51 版读数，以内容锚点为准。
- 契约出处亲读：`crate/io/fkv_io/src/lib.rs:523-556`（`SegmentIo` 契约注释，
  io-41 三件套之一）；cloud 消费侧亲读 `fkv_cloud/src/lib.rs:1984-2009`、
  :2033-2067。
- 查重：io-39（EOF/短读错误分流）、io-41（跨后端契约）、r6 io lane 报告
  遗留清单、r7 组 2/3 复审均未涉及打开路径错误折叠；`issue/` 全目录 grep
  `open_segment_file|ResourceMissing` 无承接条目。io-31 复核记录确认
  read_page 路径错误分类经 `map_io_err` 保留——反证 segment 路径是特例。


## DONE（2026-11-05，第二轮复审修复）

DONE：`open_segment_file` 非 NotFound 打开错误按 `map_io_err` 映射，重查后仍 NotFound 才回 `ResourceMissing`。

## 复审（2026-11-05 v，lane B）

通过（代码），残余测试拆 v-05。亲读 `crate/io/fkv_io_compio/src/lib.rs:1970-1998`：
首查臂 `Err(err) => Err(map_io_err(err))`（:1996），NotFound 臂重扫后
`:1990` 仍 NotFound 才 `ResourceMissing`、`:1993` 其余 `map_io_err(err)`——
DONE 声称属实。**残余**：本 issue 验收要求的 EISDIR 用例（segment 同名目录
→ `IoFail` 而非 `ResourceMissing`）及 data 侧同构断言全仓零覆盖
（`grep -rn "EISDIR\|NoPermission\|OpenFileLimit" crate --include=*.rs` 排除
`_cpp` 无测试命中，`contract.rs:57-74` 只钉 NotFound），已拆出
[`v-05`](../new/v-05-open-error-errno-classification-tests-missing.md)。
归档 `issue/reviewed/`。
