# struct-46 页缓存重构引入的 2 处 clippy `absolute_paths` 违规：HEAD 门禁红（`std::sync::Arc` 未导入）

- 领域：结构 / 门禁（并行会话 `refactor(cache)` 引入）
- 优先级：P3（机械修复；但 CI lint job 在 HEAD 确定性红）
- 置信度：高（`cargo +nightly clippy --workspace --all-targets --all-features
  --keep-going -- -W clippy::absolute_paths` 实测两处）

## 现象

fkv HEAD `52650d6`（= 并行会话 `f6f08c8 refactor(cache)` + 本轮 struct-45 修复）
树内两处 `clippy::absolute_paths`：

1. `crate/io/fkv_io/src/lib.rs:78`：新增 trait 默认方法
   `read_page_shared` 的签名返回 `impl Future<Output = Result<std::sync::Arc<[u8]>>>`
   ——`std` 前缀 + `Arc` 未在文件头导入；
2. `crate/engine/fkv_task_read/src/lib.rs:244`：同类（页缓存读路径新代码）。

两处均属 `refactor(cache): 删除 index 逻辑页缓存，收敛为单一 PageCache` 的
新 API 面，非 struct-45（`queue_backpressure` 守卫，已修）遗留。

## 建议（一行级机械修复）

两文件头部 `use std::{...}` 增补 `sync::Arc`（或按既有风格追加独立
`use std::sync::Arc;`），签名改为 `Result<Arc<[u8]>>`。

## 验收方式

```bash
cargo +nightly clippy --workspace --all-targets --all-features --keep-going \
  -- -D warnings -W clippy::absolute_paths   # exit 0、零输出
```

## 处置记录

- 2026-11-08 第 34 轮结构域发现（struct-45 复核时的连带门禁全表扫描；归属
  并行会话在途重构面，按 struct-29/38/42 先例立项）。若并行会话在其下一笔
  提交内自行修复，则本条随其提交归档。

## 实现记录（2026-11-08，第 35 轮结构域）

- 修复 fkv commit `5b4ecd8`。复测发现实际为 **5 处**（并行 `4d1f11c` 的 PageRef
  新 API 又引入 3 处）+ `fkv_task_scan/src/lib.rs:271` 1 处（此前被 fkv_io
  编译失败遮蔽）：全部改为各文件已导入的短名（`Arc` / `fmt::*` / `Deref`；
  `fkv_task_scan` 补 `sync::Arc` 导入）。
- 验证：clippy 原文命令 **exit 0、零输出**；`fkv_io + fkv_task_read +
  fkv_task_scan` 22/22；fmt 双通道 exit 0。
- 勘误：原立项称「`Arc` 未在文件头导入」对 `fkv_io` 不准确（已导入，只是
  签名写了全限定路径）。
