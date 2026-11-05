# lane D 验证轮报告（2026-11-05 v）

范围：store / shard / write / 文档一致性。复核 `issue/done/` 中本 lane 的 9 条 DONE，
逐条亲读在盘源码 / 文档 / 测试（未运行 cargo，按本仓复审口径以在盘代码为准）。
只读源码；仅写 `issue/new/`、`issue/reviewed/` 及从 `issue/done/` mv。

## 逐条去向

| 条目 | 判定 | 去向 | 关键证据 |
|---|---|---|---|
| eng-32c 读并发验收与指标 | 通过 | → reviewed/ | 指标字段全在（`fkv_metrics/src/lib.rs:38/:289/:370-378/:530/:538/:627-631/:670-678/:731-751`）；shard 接线（`fkv_shard/src/lib.rs:65/:2516/:2633/:2707`）；4 验收测试（`tests/read_concurrency.rs:143/190/251/305`）+ metrics 3 条（`fkv_metrics/tests/metrics.rs:146/159/222`） |
| eng-90 合并零 clone / 两级语义 | **在途，待下轮** | 留 done/ | 见下「eng-90 专项」 |
| r8d-01 write_gate 文档 | 通过 | → reviewed/ | 模块头 `write_gate.rs:8-10` 已写 `WriteFlight` 跨分区真并发；禁用词双 grep 零命中 |
| r9a-01 config partition_filter 文档 | 通过 | → reviewed/ | `fkv_config/src/lib.rs:11-13`「不做 1:1（sto-61b 范围外）」；`fkv_runtime/src/lib.rs:2468`「已按范围外关闭」；`06-gaps.md:24` / `COMPAT_BASELINE:20` 三处一致；`随 sto-61` 零命中 |
| r9b-02 branch segment 预压缩零测 | 通过 | → reviewed/ | exact 传测 `tests/branch_store.rs:388`（断言 :431-489/:504-509/:534-546）+ factor=0 镜像 `:591`；产品 `compact_before_branch` :3640、segment 臂 :3672-3674、`file_gc` :3679；eng-60b 缺口已补记 |
| r9e-03 bench key-interval 措辞 | 通过 | → reviewed/ | `fkv_bench/src/main.rs:606-614` 改网格 vs 随机步进；`doc/metrics.md:169-170` 增偏离条目；`perf-74` 末尾增 `## 更正` 段 |
| rev-44 GRM 段 slab / 批量 fixed 读 | 通过 | → reviewed/ | `read_segment_fixed_li`（`fkv_io/src/lib.rs:525`）+ `read_batch_fixed`（`fkv_large/src/batch.rs:89`）+ `managed.rs:169`；测试 `fixed.rs:223/:278`、`managed.rs:253`；裁剪理由在 `COMPAT_BASELINE:27-33` |
| rev-45 快照 O(1) 指针表共享 | 通过 | → reviewed/ | `chunk_li: Arc<Vec<Arc<Chunk>>>`（`fkv_mapping/src/lib.rs:145`）、`chunk_li_mut` make_mut（:167-171）、`prune_len`（:1108/:1140-1142）；语义不变由 `tests/snapshot.rs:99/:131/:159` 钉死 |
| rev-48 写并发文档滞后 | 通过 | → reviewed/ | `doc/migration.md:206`（M3）/`:207`（M4）改写；全 `doc/` 禁用词 grep 零命中 |

## eng-90 专项（在途，未归档）

**结论：DONE 叙述与在盘代码不符，且 `fkv_shard/src/lib.rs` 正在被并行会话秒级改写
（mtime 07:48→07:59:54 连续变动），按「5 分钟写入区」规则判在途，不归档。**

- DONE / r7 复审称两级语义为「同一请求内后写**无条件**胜；跨请求 `timestamp >=` 胜」。
  在盘实现却是**请求内也按 timestamp 折叠**：`if next_entry.timestamp >= cand.timestamp`
  （`fkv_shard/src/lib.rs`，函数每几十秒漂一次行号，07:59 版为 :2199）。
- 佐证在盘实现才是仓内**规范语义**：perf-79 落点 `fkv_task_write/src/lib.rs:218-233`
  同批重复 key 即按 `cur.timestamp >= kept.timestamp` 取较新者（相等后者胜），
  `Store::put` 旧页过滤亦 `entry.timestamp >= slot.timestamp`（`fkv_store/src/lib.rs:3124`）。
  若 shard 按「无条件后写」折叠，则**组提交路径与单请求 dispatch 路径语义分叉**
  （单条 Put 不经 merge，直接 `store.put`），故现行为更正确，DONE 叙述反而是旧口径。
- 测试 `merge_put_batches_keeps_cross_request_timestamp_semantics`（`lib.rs:3318` 区）
  现断言请求内 ts=5 胜 ts=4（value=first），与 DONE「ts=5 先写被 ts=4 后写覆盖」相反，
  即该测试已被并行会话改过；`merge_put_batches_request_inner_rep_before_cross_request_compare`
  （:3366 区）断言期望值 ts=9 / value=fresh，与甄别反例叙述（期望 ts=7）也不一致。
- 同源滞后记录：`issue/reviewed/20-write-batching.md:236` 仍写「单请求内后写覆盖」。
- 处置：不动 `done/`（在途）；**未新立项**（并行会话正在就地改，避免噪声与重复）。
  建议下轮：以在盘（应已收敛的）perf-79 折叠语义为准，重写 eng-90 DONE / r7 叙述，
  并同步 `20-write-batching.md:236`；届时按「两级语义：请求内 ts 折叠（相等后写胜）/
  跨请求 `ts >=` 胜」验收，而非「无条件后写胜」。

## 特别核对小结

- **eng-90 零 clone 折叠法**：结构确为 perf-79 引用排序 + 单趟折叠、仅幸存者 clone 一次
  （`Vec<(req_idx, entry_idx, &BatchEntry)>` + 容量预估 + `sort_unstable_by(key, req, entry)`
  + 两级折叠 + `out.push(...clone())`）——**零 clone 主张成立**；但两级语义叙述与代码不符
  （见上），整体不予归档。
- **rev-44 裁剪**：裁剪为「批量 fixed 读接口」属实，且裁剪理由（不做 1:1 的三条依据）已
  在 `COMPAT_BASELINE:27-33` 在案。通过。
- **rev-45 快照语义**：线性 clone 优化后 27a「快照持取出时刻完整映射视图」不变
  （`mapper_mutation_copies_shared_chunk_dir_once` 断言旧值不随写改变）。通过。

## 新立项

无。9 条中 8 条通过归档，1 条（eng-90）因在途留 done/ 并已在上节给出下轮处置建议；
无「不实需整改」的独立条目，按「宁缺毋滥」不新开 `issue/new/`。

## 备注

- 本轮只读源码未运行 cargo（并行会话在写），全部以在盘代码 + 测试断言为证。
- `fkv_shard/src/lib.rs`、`fkv_store/src/lib.rs`、`fkv_runtime/src/lib.rs`、`fkv_metrics/src/lib.rs`
  在本轮期间持续被并行会话改动（mtime 07:37–07:59）；上述 8 条通过项的锚点均在各自
  稳定文件或与内容锚点一致，未受漂移影响。
