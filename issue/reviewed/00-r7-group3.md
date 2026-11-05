# r7 第 3 组复审报告（eng-93 / io-31 / rev-39 / rev-42a，2026-11-05）

复核范围：另一会话移入 `issue/done/` 的 4 条，逐条按 DONE 声称到当前源码核对
（行号均亲读；复核时点 04:20–04:35）。热文件 fkv_store/src/lib.rs（04:18）、
fkv_io_compio/src/lib.rs（04:17）、fkv_task_gc（04:17）在 5 分钟在途窗口内，
本轮所有「声称 vs 代码」核对均以内容锚点定位且全部命中——在途改动未触碰
本组条目的实现面，无「在途待下轮」判定。

## 结论一览

| 条目 | 判定 | 去向 |
|---|---|---|
| eng-93 COMPAT_BASELINE buffer pool 登记行 + meta 刷新 + README 路径 | 完善 | reviewed/ |
| io-31 read_page 短读终局化 + 回退打点 | 完善 | reviewed/ |
| rev-39 GC (branch,term) 变体粒度删除 | 完善（含 1 条过期遗留声称，更正入 rev-47） | reviewed/ |
| rev-42a RootMetaMgr 字节预算 LRU | 完善 | reviewed/ |

新立项 1 条（去重后）：`issue/new/rev-47-space-stats-still-id-level-after-variant-gc.md`
（P3）：rev-39 自认 lane 外遗留的统计侧（file_live_count / 空间统计仍 id 级）
无条目承接，DONE 明言「另行立案」；顺带更正其遗留段「fkv_metrics / fkv_shard
wrapper 未透传变体接口」的过期声称（亲读：均已透传，泄漏修复在生产路径已生效）。

## 逐条证据（重点核对项）

### eng-93 — 完善

- **38/38 meta.txt 与 COMPAT_BASELINE 逐字节一致（重点项）**：按
  `tool/fkv_ledger/src/fixture.rs:342-354`（`lines` → `trim` → 滤空 →
  `" | "` 连接）复刻 flatten，全量比对 `fixtures/**/meta.txt` 的 `baseline=`：
  `distinct_values=1`、`mismatch=0`（1503 字节）。上轮 eng-91 登记的
  「加行必须立即烤语料」已兑现。
- `COMPAT_BASELINE` 全文 26 行亲读：buffer pool 偏离登记行 :14-19（口径 =
  migration.md §8.1：IndexCache / 内核 page cache + `with_direct_io`（io-21）
  替代、`enable_data_page_cache` 死旋钮 sto-57、引用 reviewed/rev-41 :19）；
  :13 已改指 `issue/reviewed/02-…`；证据行居末 :20-26。
- `fixtures/README.md:154`（eng-91）、:157（02）均指 `issue/reviewed/`；
  `grep "issue/done/" COMPAT_BASELINE fixtures/README.md` 0 命中。
  buffer pool grep 实际 :14/:15/:18 三处命中（DONE 列举 :14/:15 为不完全
  列举，非失实）。

### io-31 — 完善（重点项：短读终局 Corrupted 上抛；回退打点 / 受限）

- `fkv_io_compio/src/lib.rs` `read_page_candidate`（:1518-1569）：短读
  `0 < read_len < page_size`（:1553-1557）只同路径 `continue` 重试
  `READ_RETRY_TIMES=3`（:91/:1531），循环耗尽（只有短读可达）:1567-1568
  `Err(KvError::Corrupted)`——**终局上抛，不落旧 term 候选**；瞬时错误
  （`is_retryable` :1540-1544）耗尽上抛原错误不换候选；EOF（:1549-1552）与
  checksum 失败（:1561-1565）保留回退（迁移 / 稀疏洞机制未误杀），全候选耗尽
  后 `EndOfFile`/`Corrupted` 判定不变（read_page :2298-2302）。
- 调用方 `read_page` :2283-2285 `…await?` 直接传播终局错误；批量
  `read_page_li` :2316-2322 `try_join_all` 逐页委托，继承同一语义。
- 打点：`stale_read_num`（:708/:996）仅在候选 `index > 0` 的 `Hit` +1
  （:2287-2291）；`read_at_num`（:705/:990/:1532）计每次尝试——回退窗口
  由全程静默变为可观测且回退受限（EOF / checksum 两分支专属）。
- 测试 4 条亲读断言（tests/hardening.rs :356-457）：短读终局 `Corrupted` +
  恰 3 次尝试 + `stale_read_num==0` + dst 保持 0xEE（:370-377）；稀疏洞 /
  EOF 回归仍回退且计数 +1（:396-400/:418-421）；注入 `TryAgain` 2 次第 3 次
  成功读新页、注入 3 次上抛 `TryAgain`（:439-456；`inject_read_try_again`
  :1010 → `read_page_chunk` WouldBlock :1482 → `map_io_err` :723）。
- 残余（checksum 失败仍回退、本地 backend 无 `BranchFileRange` 唯一解析、
  linux-gated 实跑）DONE 自认留 owner 决策，与源码一致。

### rev-39 — 完善（重点项：(branch,term) 变体精确删除；跨 branch 同 id 测试在位）

- trait 面 `fkv_io/src/lib.rs`：`FileVariant{file_id, identity}` :493-499；
  变体方法 8 个均带默认实现（list :525-539 / remove :633-641 / segment /
  local×4 :644-713），默认回退 id 级行为；旧接口全保留。
- compio：`OtherVariant{branch,term,path}` :111-115，`scan_dir` :791-812 存
  other 表不丢 branch；`list_file_variants_by_kind` :1995-2026（primary +
  other 三元组，sort+dedup）；`remove_file_variant_by_kind` :2055-2077 经
  `variant_paths_of` :1272-1303 **(branch, term) 精确匹配**删除；
  `forget_variant` :1329 只清对应索引条目。
- cloud：`remove_file_variant` :2855-2871 本地半精确删 + 远端半
  `data_file_name(file_id,branch,term)` 直拼 key；LocalGc :2921-2934 只动本地。
- GC 判定链 `fkv_task_gc/src/lib.rs`：`RefKnowledge{ref_ids,attr_failed,
  variants}` :63-74；`apply_manifest` :574-593 per-manifest 归属（对齐 C++
  `GetRetainedFiles`）；判定序 damaged→pin→归属命中→attr_failed→外部引用→
  in-flight（:444-470）；`variant_inflight` :480-489 按文件自身 (branch,term)
  对齐 C++ `file_gc.cpp:1207-1208`；GC 循环 :869-889 按变体驱动，无身份保持
  id 级（mem 回退）。归属歧义采保守剪枝 (b)，与 sto-54a 一致。
- **跨 branch 同 id 测试在位**：tests/variant.rs `gc_deletes_only_dead_
  variant_of_referenced_file_id` :95-182——main/5 与 b-1/3 同 id 2049，断言
  逐变体 retain 真假、data+segment 各删 1、盘面恰剩 b-1 两文件；另 4 条
  （全死全删 / term+水位双条件 / 旧 term 高水位之上回收 / Mem id 级）断言
  真实；协同用例 gc.rs :218/:326 在位。
- 过期声称（非失实，时序性）：DONE 遗留段「fkv_metrics / fkv_shard wrapper
  未透传」——亲读 fkv_metrics :784/:845（04:03）、shard backend :447/:535
  （04:07）、roots :289/:333（04:03）均已透传且有
  `list_file_variants_keeps_identity` 测试。已并入 rev-47 更正，不扣分。

### rev-42a — 完善（重点项：字节预算真接线 + 上限测试）

- 记账：`TableState.bytes` :193-197 + `table_state_bytes` :226-253（结构体 +
  mapping chunk + 字典 + `IndexCache::byte_len` + branch 元数据）；刷新点
  touch :628 / set_dictionary :762 / put_at 收尾 :1556 / install_replay_state
  :2114 / evict 扣减 :575 / reset 清零 :1850。
- 驱逐：`touch_table` :624-660 双上限共用 LRU 队列；`evict_over_root_meta`
  :681-700 跳过当前表与 `root_meta_evictable` :703-711 不过关的表
  （`live_num==0` 且字典非 dirty），单轮有界不死循环。
- **接线全链路（旋钮→消费）**：fkv_config :57 `DEFAULT_ROOT_META_CACHE_BYTES=
  GB` → :183 `root_meta_cache_size`（INI parse_size）→ fkv_runtime :1190-1200
  按 `num_threads` 均分（对齐 C++ root_meta.cpp，0 不限）→ fkv_shard
  `ShardConfig.root_meta_cache_bytes` :149 → :571 `PartitionStoreMap` →
  `enforce_root_meta_budget`（partition.rs :325-363，Slot.touch 近似 LRU、跳过
  `Rc::strong_count>1`）；Store 侧 `with_root_meta_cache_bytes` 等 :555-568。
- 上限测试亲读：fkv_store/tests/root_meta_budget.rs 4 条（1 字节预算 LRU 驱逐
  + reopen 数据完整 :27-55；活快照不驱逐、drop 后回收 :59-80；index cache
  计入且随驱逐释放 :84-118；默认 0 不限 :122-133）；fkv_runtime 测试
  `root_meta_cache_size_reaches_store`（:56-105）经 KvOption 1 字节预算全链路
  验证 `index_cache_bytes` 下降 + reopen 数据完整——真接线非字段摆设。
- DONE 自认限制（纯读不复查、单表超预算接受超限、shard 并行 lane 落地后需
  补跑 --all-targets）与源码一致。

## 新立项

1. `issue/new/rev-47-space-stats-still-id-level-after-variant-gc.md`（P3）：
   rev-39 遗留统计侧（`fkv_gc/src/lib.rs:12-21` `file_live_count`、
   fkv_store :3388-3389/:3731-3756 均 id 级，死变体物理占用在放大率口径不可见）
   + wrapper 声称更正。去重：new/todo grep 0 命中，无既有承接。

## 备注

- 本组无「在途待下轮」：三个热文件的核对全部以内容锚点命中且即读即验；
  mv 归档时 done/ 已清空（并行会话同窗口归档了其余条目，无交叉冲突）。
- 按惯例未动 `issue/new/00-index.md` 与 `doc/progress.md` 计数（new/ 由并行
  会话新增 rev-02 等，协调者统一刷新更安全）。
- 本轮未运行 cargo/node（构建锁约束）；判定全部基于源码实读 + flatten 脚本
  纯文本比对（python3 -I，只读仓库文件）。
