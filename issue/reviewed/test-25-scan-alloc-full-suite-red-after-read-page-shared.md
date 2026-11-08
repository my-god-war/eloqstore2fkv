# test-25 缓存重构新增 `read_page_shared` 后全量红：`fkv_task_scan::alloc` 两用例整页分配数超标（无缓存后端每层整页分配）

- 领域：测试质量 / 性能回归（既有整页缓冲复用守卫被重构打破；建议性能域会签）
- 优先级：P2（全量 `--workspace` 门禁确定性红：1504 中 3 失败，其中 2 条为本条；
  属生产行为回归被既有守卫捕获，非 flaky）
- 置信度：高（两用例确定复现；同一断言在 `ff63590`（重构 squash 基线）以完全
  相同的数字复现 ⇒ 由缓存重构引入，非 f6f08c8 残余小补丁）

## 现象（fkv `52650d6`，worktree `/tmp/r29/fkv`）

```text
$ cargo nextest run --workspace --exclude fkv_nvme_test --all-features --no-fail-fast
    Summary [  42.328s] 1504 tests run: 1501 passed, 3 failed, 1 skipped
        FAIL fkv_task_scan::alloc scan_reuses_page_buffers
        FAIL fkv_task_scan::alloc scan_single_page_skips_spare_buffer
        FAIL fkv_shard::queue_backpressure dropped_pending_low_request_rolls_back_depth
```

两条 `alloc` 失败明细（`crate/engine/fkv_task_scan/tests/alloc.rs`，计数分配器
按 `PAGE_SIZE=512` 精确统计整页分配）：

1. `scan_reuses_page_buffers`（`:184`）：预热两次后单次 scan 仍有
   `page_sized=2`（期望 `0`；总分配 17），`assert_eq!(page_alloc_cnt, 0)` 失败；
2. `scan_single_page_skips_spare_buffer`（`:226`）：单页扫描分配 `3` 块整页
   （期望 `1`），`assert_eq!(page_alloc_cnt, 1)` 失败。

第 3 条 `queue_backpressure::dropped_pending_low_request_rolls_back_depth`（15.0s
守卫触发）由 **io-32** 跟踪，**不计入本条**。

## 根因（并行会话已在 `4d1f11c` 定位并修复）

- 重构（`f6f08c8` / 并入 `ff63590`）新增 `PageIo::read_page_shared` 返回
  `Result<std::sync::Arc<[u8]>>>`：缓存命中零拷贝，但**未命中要
  `vec![0u8; page_size]` 再 `.into()` 转 Arc**；无缓存后端（内存 / 差分 / sim
  / failpoint 用例）每层下降都整页分配，`alloc.rs` 的「预热后零整页分配」直接
  被击穿。
- 同一缺口即 **struct-46** 那两处 `absolute_paths` 违规的同源 API
  （`read_page_shared` 的签名与新读路径）。
- 已验证与 `f6f08c8` 残余补丁无关：在 `ff63590`（其父）跑
  `cargo nextest run -p fkv_task_scan`，两用例以**相同数字**失败
  （`scan_single_page_skips_spare_buffer`：`left: 3 / right: 1`）。

## 关系 / 现状

- 并行会话在 **`4d1f11c`**（main HEAD，晚于本轮判定 HEAD `52650d6`）已把
  `read_page_shared` 换成 `read_page_ref -> PageRef{Shared,Borrowed}`（未命中读进
  调用方池化缓冲、零额外分配），并在提交说明里明确承认「`alloc.rs` 的『预热后
  零整页分配』直接失败」。故本条在 `4d1f11c` 上预计已消解；在 `52650d6` 上仍红。
- 建议：本轮判定若以 `52650d6` 为 HEAD，则本条计入门禁红；若 HEAD 前移到
  `4d1f11c`，请复跑 `fkv_task_scan` 复核后归档本条。

## 验收方式

```bash
cargo nextest run -p fkv_task_scan --all-features   # alloc 两用例通过
cargo nextest run --workspace --exclude fkv_nvme_test --all-features
  # 除 io-32 背压守卫外全绿
```

## 处置记录

- 2026-11-08 第 35 轮结构 / 测试域门禁全量发现（fkv `52650d6`）。归属并行缓存
  重构在途面；不重复计入 io-32（背压守卫）。

## 实现记录（2026-11-08）

- 由并行会话的 `4d1f11c`（`read_page_shared` → `PageRef` 零拷贝）修复；
  协调者在冻结快照 `4d1f11c`/`6524c4b` 复跑
  `cargo nextest run -p fkv_task_scan --test alloc --all-features` →
  **2/2 passed**；全量 **1505 passed / 1 skipped / 0 failed**。⇒ 闭环。

## 复审记录（2026-11-08，第 36 轮结构域）

- 判定：**达标**，移入 reviewed/。冻结快照复跑 `fkv_task_scan --test alloc`
  **2/2 passed**；全量两遍 1505/1505 + 1 skipped。闭环。
