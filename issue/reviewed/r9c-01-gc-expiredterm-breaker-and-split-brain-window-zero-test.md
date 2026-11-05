# r9c-01 P3：r8c-01 修复代码闭环但测试面欠账——GC ExpiredTerm 熔断零覆盖、分裂脑窗口无端到端用例

## 现象（亲读）

r8c-01 的三层护栏**代码**已落地且与 C++ 逐项对齐：

1. `InflightGuard.process_term`（`crate/engine/fkv_task_gc/src/lib.rs:107`）+
   `retain_variant` 的 `term > self.process_term → 保留`（:494-496），data / segment
   共用，对齐 C++ `file_gc.cpp:973-977`（data 循环）与 `:1426-1428`（segment 循环）。
2. `seed` / `seed_segment` / `set_process_term`（:238/:246/:252）由
   `Store::gc_with_scope`（`crate/engine/fkv_store/src/lib.rs:3928/:3941/:3948`）
   显式注入；默认 `u64::MAX`（task_gc :132）保证直连 task_gc 的旧判定不变。
3. `term_above`（task_gc :258-260）+ `gc_with_scope` 的整轮熔断
   （fkv_store :3951-3953 `return Err(KvError::ExpiredTerm)`），对齐 C++
   `ExecuteCloudGC` `:1554-1560`。

但**测试**只交付了 r8c-01 验收清单的一半：

- 唯一新用例 `future_term_variant_is_always_retained`
  （`fkv_task_gc/tests/variant.rs:343-350`）是判定函数级单测：
  - 用 `set_process_term(5)` 注入，而 r8c-01 验收原文写的是「**seeded** 的进程
    term」——`seed` 路径（分配器高水位 + term 同源注入，生产入口
    fkv_store:3928）未被覆盖；
  - 没有「file id 低于本地高水位」形态（用例里 guard 无任何 bound，
    `variant_inflight` 本就返回 false，防护是否真由 term 门控生效只能靠
    `assert!(retain)` 的反差推断，边界形态未固化）；
  - 没有「一轮 GC 删除数为 0」的端到端断言——验收原文明确要求。
- 第三层熔断**全仓零测试**：`grep -rn ExpiredTerm crate/` 的全部测试命中
  （`fkv_shard/tests/lifecycle.rs:170/174/212/225`、`cloud_reopen.rs:447`、
  `fkv_cloud/tests/cloud.rs:95/149/643/670/701`、`fkv_io_compio/tests/term.rs`、
  `fkv_io_mem`、`fkv_io_sim`）均为写路径 fencing（`put_with_term` / term CAS /
  attach 拒旧 term）与 reopen 场景；`fkv_store/tests/` 的 GC 用例
  （gc_store / gc_cleanup / gc_inflight / gc_snapshot / local_gc / branch_gc）
  无一条构造「manifest term > 本进程 term」的盘面，`file_gc` / `local_gc` 的
  `is_err` 断言为零。
- 分裂脑窗口（r8c-01 的立项动因场景：更高 term 写者的文件 id 低于本地
  manifest 高水位）因此只剩单测级防护。回归形态可举两例：有人把
  fkv_store:3951 的 `term_above` 检查挪到 `is_damaged` 早退之后（damaged 分支
  `Ok(0)` 会先返回，云端熔断静默失效）；或 `term_above` 的 `>` 误改为 `>=`
  （正常单写者每轮 GC 直接报错）。现有测试都不会红。

## C++ 对照

C++ 三层护栏同样没有单测直证（`file_gc.cpp` 无 term 护栏专项用例），但 C++
侧本条是既有行为、不需要回归防护；fkv 是新落地的对齐修复，按本仓
「关键路径与边界测试存在性」口径（SKILL.md 测试规范、r6~r8 各修复条目均
带断言级用例），该缺口应随修复补齐而不是留给后续轮次。

## 目标

1. `fkv_task_gc/tests/variant.rs` 增端到端用例：`guard.seed("main", 5, 10 << shift, shift)`
   后构造候选 `data_7_main_7`（term 7 > 5、id 7 < 高水位 10）→
   `retain_data_variant` 为真且一轮 `gc_files_with_retained` 删除数为 0；
   segment 同构断言一次。
2. `fkv_store` 侧用例（gc_store.rs 或新建）：store term=5，盘面写入
   `manifest_main_7`（term 7 的 branch snapshot）→ `file_gc` 返回
   `Err(KvError::ExpiredTerm)` 且 data / segment 文件零删除；
   `local_gc` 的行为顺带断言固化（现实现两种 scope 共用熔断，见核验记录 4）。
3. 可选：`gc_zero_budget_disables_yield_points` 同款的防回归敏感度——把
   fkv_store:3951 临时挪位 / 改比较符，确认用例变红（开发期验证，不入库）。

## 验收

- 上述用例入库并在 `./test.sh` 全绿；
- 把 `term_above` 检查注释掉重跑，新增用例必须失败（证明测试真的防护该层）。

## 核验记录

- 亲读 `fkv_task_gc/src/lib.rs` 全文（process_term 注入点 :238/:246/:252/:258/:494、
  默认 `u64::MAX` :130-132）与 `fkv_store/src/lib.rs:3904-4048`（gc_with_scope 全文）。
- 亲读 C++ `file_gc.cpp:955-1000`（data 循环护栏 :973）、`:1410-1445`（segment
  护栏 :1426）、`:1530-1580`（ExecuteCloudGC 熔断 :1554-1560）、`:239-330`
  （ExecuteLocalGC，含注释「No need to check term expired for local mode」——
  Rust 现实现把熔断同时用于 `GcScope::Local`，本地 / 内存后端因 attach CAS
  拒低 term、manifest term ≤ self.term 恒成立而无行为差；云端 stale 实例的
  LocalGc 会因远端更高 term manifest 报 `ExpiredTerm`，属安全侧偏离，本轮
  不单独立项，仅在此登记）。
- 测试面 grep：`ExpiredTerm` 全仓命中逐条归类（无一属于 GC 路径）；
  `fkv_task_gc/tests/gc.rs` 7 用例、`variant.rs` 5 用例、`inflight.rs` 7 用例
  标题与断言主题核对，无未来 term 端到端用例；
  `fkv_store/tests/gc_*.rs` / `local_gc.rs` / `branch_gc.rs` 无
  「manifest term > store term」盘面构造。
- 查重：r8c-01 的 DONE 段仅声明代码修复（「InflightGuard 增加 process_term
  门控与 term_above 检查」），未声明测试项完成；`issue/new` 现存 r7a/r7b/r7c
  三份并行发现文件无重叠；`issue/reviewed/` 无本条承接。


## DONE（2026-11-05，第三轮复审修复）

DONE：新增 seeded 未来 term 单测（删除数 0）与 Store 级 `file_gc → ExpiredTerm` 端到端用例。

## 复审（2026-11-05 v）

通过，归档。亲读新增用例：

- `fkv_task_gc/tests/variant.rs:343-350`
  `future_term_variant_is_always_retained`（`set_process_term(5)`，term 7 保留、
  term 5 常规判定）。
- `fkv_task_gc/tests/variant.rs:354-385` `gc_seeded_future_term_deletes_nothing`：
  `guard.seed("main", 5, 10<<1, 1)` + `seed_segment`，候选 `data_7_main_7` /
  `segment_7_main_7`（term 7 > 5、id 7 < 高水位 10）→ 两 `retain_*_variant` 为真，
  `gc_files_with_retained` 删除数 == 0，`remain_li` 双文件仍在。补齐了「seeded
  路径」与「一轮删除数为 0」两项欠账。
- `fkv_store/tests/gc_store.rs:266-319` `gc_aborts_on_manifest_term_above_process_term`：
  store term 5、盘面写 `manifest_main_7` → `file_gc` 返回 `KvError::ExpiredTerm`
  且 `list_file_id_li` 前后不变（data/segment 零删除）。
- 熔断检查位置（fkv_store:4109 在 damaged 早退前）与 r9c-01 点名的回归形态一致，
  改比较符/挪位会使上述两用例变红（开发期敏感度项，属可选，未入库）。
- 备注：核验记录自认的「LocalGc 也走同一熔断 vs C++ 本地无此检查」为安全侧
  偏离，已在 issue 内登记，不另立项。
