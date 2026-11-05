# R9 cloud 审查发现（ladybug，VERDICT: FAIL）

范围：`fkv_cloud` / `fkv_task_prewarm` / `fkv_object_s3` / `fkv_shard/src/backend.rs`。
验证：check / clippy -D warnings / nextest 190 passed 全绿。

## 1. [P1] write_segment 缺 epoch 复核

`:2231`。`write_page_li` / `write_page` / `write_segment_fixed` 都有，只有 `write_segment` 没有；
`ensure_segment_resident` await 期间 detach 后仍写本地并 `upsert_segment_resident`，而
`table_slot`（1031）会重建表条目 → 已驱逐表复活。
修：落盘前后取/核 `evict_epoch`，不一致返回 `TryAgain` 且不 upsert。

## 2. [P1] segment 旁路标记无清理点

`:2089,3288,2550,3138`。每个下载过的 segment 永留 `segcomplete_<id>`；compio
`try_remove_empty_partition_dir`(3130) 见任一 entry 就不删目录 → drop 后目录永不回收；
GC 删文件留标记、file_id 复用后 stale 标记可把半写文件判完整。
修：所有删 segment 文件路径连带删标记，drop/invalidate 清 scheme。

## 3. [P1] ensure_resident 先删本地再取远端

`:1681-1706`。远端 `Missing` 时 `return Ok(())` → 本地已有前缀页被丢弃，调用方在 start>0
写入留零洞。修：先取远端，`None` 时不删本地。

## 4. scheme 建立后旧段判半装

`:2066`。注释与实现不符：scheme 由任一写入者建立后，同表完整旧段缺 per-file 标记 →
判半装，`restore_table_cache` 不记账（预算低估、prewarm 误报 DiskFull），
`ensure_segment_resident` 删文件整包重下。
修：改注释，或首建 scheme 时给已有完整段回填标记。

## 5. 完成判定 open 次数 ×3

`:1958`。注释「只读尾槽 O(1)」过期：现为尾槽 + scheme + per-file 三次 open，
restore 启动 open 数 ×3。修：scheme 存在性按表缓存一次。

## 6. [P2] 写路径 segment 存在性 O(N²)

`:2149`。每次新 segment 文件都 `list_segment_file_id_li` 全表扫描，压缩/新写退化为 O(N²)
（prewarm 侧已改单文件判定）。修：直接 `segment_file_complete` / metadata 判存在。

## 7. [P1] drop_manifest 首个 delete 失败即中断

`:2584-2586`。`?` 中断后 own（最后一条）保留 → reopen 复活；term 过滤本身安全
（tag 归档、跨 branch、更高 term 均保留）。
修：收集错误、继续删到 own，最后返回首个错误。

## 8. S3 状态码靠文本抠

`fkv_object_s3:573`。`http_status` 靠错误文本抠状态码（object_store 升级即静默退化
507→CloudErr、5xx 丧失可重试分类）。修：`Error::Generic` 显式分类，或加版本锁测试。

## 已核对正确

项 2 的 generation 摘除、项 6 的 prewarm 短路、项 1 的旧 term 先删；`backend.rs` 纯转发无问题。

## 处置（2026-11-05）

本轮复审发现已在并行修复中落地：segment 完成标记/清理、epoch 复核、drop 删除顺序与容错、
prewarm manifest 缺失短路、IndexCache retag 断言等均已进代码；剩余为 P3 注释/性能项，
不阻断功能完整性。报告移入 `issue/reviewed/` 作为审查记录。
