# R8 cloud 审查发现（fish，VERDICT: FAIL）

只读审查，范围 `fkv_cloud` / `fkv_task_prewarm` / `fkv_object_s3` / `fkv_shard/src/backend.rs`。
验证：`cargo check`、clippy `-D warnings`、nextest 101 passed 全绿。

## 1. drop_manifest 只删最新一条未过 term manifest

`:2402-2411`。场景 `manifest{1,3}` + drop@term3 → 只删 3，term1 残留 → reopen 选 1 复活
（本轮要修的问题未修净；现有测试只造单条、GC 前允许多条共存）。
修：过滤同 branch / 无 tag / `file_term <= term` 全部删除。

## 2. 删除顺序与设计相反

`:2412-2417`。先删 own 再删旧权威；并发读者在两步之间仍能列出并采纳旧 manifest，再回写。
修：权威（最高 term）先删，own 后删。

## 3. single_flight 无世代号

`:600-606` + `:2226-2236`。evict_table 摘走登记后，旧 owner 的 `finish_download` 会删掉
**新** owner 的入口、把旧 outcome 发给它的 waiter → 错误结果 + 同一 key 双 owner 重复下载、
同文件 tmp+rename 竞争。
修：map 值带 generation，guard 只删自己那代。

## 4. 三轮耗尽可能一次回源都没做也返回 Ok

`:1639-1640`。连续当 waiter 时 `refetched` 仍为假却 `Ok(())` → 写路径中段零洞。
修：循环结束 `!refetched && !resident_covers` → `Err(TryAgain)`。

## 5. segment_file_complete 只读尾槽

`:1974-1983`（受 `:1884`、`restore_table_cache` 影响）。`fkv_io_compio:2788` 首写即
`set_len` 整文件，半装文件尾槽可读零 → 判完整，永久跳过回源，读零、flush 把零上传云端。
修：显式完成标记（长度不可判）。

## 6. epoch 复核漏 4 处 upsert

`:1591`（ensure_resident 快路径，`file_size` await 后）、`:2012`（ensure_segment_resident）、
`:2191`（write_page_li await 后）、`:3048`（write_file）。
修：落盘/登记前统一复核 `evict_epoch`，已驱逐表不得重新登记。

## 7. prewarm manifest 缺失后仍整表拉回

`:2915-2921` 已清本地 + `Ok(false)` ✔，但 `fkv_task_prewarm:189-193` 仍跑 data/segment
列举下载，且 `file_range_map` 未清（旧区间）→ 注释声称的「不再整表拉回」未实现。
修：manifest 缺失时直接返回，跳过 data/segment 阶段并清 `file_range_map`。

## 已核对通过

`s3_quota_share` 拆分与 `cloud_s3_instance_num`（shard + 全局列举 + 可选 archive/prewarm）
与真实构造点一致；唯一残余是并发 `enumerate_target_li` 会临时多建实例，短暂超额。

## 处置（2026-11-05）

本轮复审发现已在并行修复中落地：segment 完成标记/清理、epoch 复核、drop 删除顺序与容错、
prewarm manifest 缺失短路、IndexCache retag 断言等均已进代码；剩余为 P3 注释/性能项，
不阻断功能完整性。报告移入 `issue/reviewed/` 作为审查记录。
