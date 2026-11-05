# eng-53 rebalance_data_pages 按条目数中点切分而非字节均值，且空表首写与索引页缺重平衡

## 位置
- Rust: `crate/engine/fkv_task_write/src/lib.rs:453-458, 731-822`（`rebalance_data_pages`、`probe_split_point` 从 `half = n / 2` 条目数中点开始探测，且 `has_overflow` 时直接跳过）、`:522-566`（`write_new_data_page` 未调用 `rebalance_data_pages`）、`:916-948`（`build_index_page_li` 无重平衡）
- C++: `eloqstore/src/tasks/batch_write_task.cpp:982-1019`（`FinishIndexPage`）、`:1064-1095`（`FinishDataPage`）、`:1118-1265`（`Redistribute` 数据页与索引页均按 `average_len = (prev_page_len + cur_page_len) >> 1` 字节长度在 restart region 边界切分）

## 现状
复审 `done/22-split-heuristic-divergence.md` 发现 4 处偏差：
1. **切分点按条目数 `n / 2` 而非字节长度均值**：`probe_split_point`（`:795`）以 `half = n / 2`（总条目数的一半）作为起点。当键或值大小不均匀（例如前 30 条各 20 B，后 2 条各 1500 B）时，`n / 2 = 16` 会使第 1 页仅 ~320 B（仍不足 1/4 页），第 2 页超过 3/4 页。
2. **含 overflow 条目的页完全跳过重平衡**：`rebalance_data_pages`（`:748`）在 `has_overflow[idx - 1] || has_overflow[idx]` 时跳过重平衡；而 C++ 中 overflow 页指针只是普通的 4 字节/列表值，照常参与 `Redistribute`。
3. **空表首次批量写入（`write_new_data_page`）未调用 `rebalance_data_pages`**：`root_id == PageId::MAX` 时走的 `write_new_data_page`（`:522-566`）直接调用 `build_data_pages_from_batch`，尾页不足 1/4 时不触发重平衡。
4. **索引页（`LeafIndex` / `NonLeafIndex`）缺少 `Redistribute`**：C++ `FinishIndexPage`（`batch_write_task.cpp:990-1003`）在分裂出的尾索引页 `< 1/4` 且前一页 `> 3/4`、`restart_num > 1` 时同样执行 `Redistribute`；Rust `build_index_page_li` 未实现。

## C++ 实现
C++ 对所有新建/分裂的数据页与索引页统一按 `average_len = (prev_len + cur_len) >> 1`（字节数均值）结合 `PageRegionIter` 重分配，且不因包含 overflow 指针而跳过。

## 差异与风险
- 变长 value 负载、空表大批量导入、含 overflow 值或大索引树分裂时，Rust 仍会产生 `< 1/4` 的稀疏尾页，与 C++ 树结构及页数不一致。

## 对齐建议
- `probe_split_point` 改为按累计编码字节数寻找接近 `(prev_len + cur_len) / 2` 的切分点（配合 `fmt-17` 直接在 restart region 边界切分）。
- 在 `write_new_data_page` 中同样调用 `rebalance_data_pages`，并支持携带 `OverflowSlot` 跟随条目迁移。
- 在 `build_index_page_li` 中补齐索引页尾页 `< 1/4` 且前页 `> 3/4` 时的重分配逻辑。

## 性能优化建议
基于累计字节与 restart region 边界直接切分，可避免在 `probe_split_point` 中反复调用 `build_single_data_page` 试探编码。

## 严重度
P2（C++ 盘面结构对齐与空间利用率）

## 审查结论（2026-11-04）

**总结论：部分采纳。** 第 1/2/3 条属实且未被他处覆盖，采纳并进 `todo/`；第 4 条（索引页 Redistribute）与 `issue/new/core-15-index-layer-rebalance-missing.md` **完全重叠**，本单不做，交 core-15 统一实现，避免重复开发。另外 3 处事实需修正（函数名/行号/引用文档）。

### 与既有 issue 的重叠判定

| 本单条目 | 重叠 issue | 判定 |
|---|---|---|
| 4 索引页缺 Redistribute | `core-15` | **完全重复**。core-15 已给出 `FinishIndexPage:982-1019` + index `Redistribute:1189-1265` 的定位与对齐方案；本单删除该项，仅在 core-15 落地后复查 |
| 1 切分点按条目数 | 无 | 不重叠。core-15 第 10 行明确把 data 层 rebalance 视为“已对齐 C++”，未讨论切分度量；eng-49 只谈索引页重建/分隔键 |
| 2 overflow 页跳过 | 无 | 不重叠。core-15 未提及 `has_overflow` 守卫 |
| 3 空表首写未 rebalance | 无 | 不重叠。eng-49 不涉及 `root_id == PageId::MAX` 分支 |
| 性能建议“避免反复试探编码” | `fmt-17`（region 原语） | 仅技术手段相邻，非同一问题。fmt-17 交付 `PageRegionIter` 后可把切分点收敛到 restart 边界，作为可选后续，不阻塞本单 |

### 逐条核验

| # | issue 论断 | 代码实况 | 结论 |
|---|---|---|---|
| 1 | `probe_split_point` 按 `n / 2` 条目数中点 | **函数不存在**（全仓 `rg probe_split_point` 零命中）。真实实现是 `rebalance_data_pages`（`crate/engine/fkv_task_write/src/lib.rs:778-839`），`let half = n / 2` 在 **:813**，`delta` 向外探测在 :815-832，每次 `build_raw_page` 全量重编码 | 论断**本质成立**，定位需改写 |
| 1b | 变长条目下 1/4 页失衡 | 成立，且比 issue 描述的更严重：当大 entry 落在后半段时，`k ∈ (0, n)` 的**所有**切分都放不下后半页，`replaced` 保持 `None`，**整对页静默放弃重平衡**（不是“切得不均”，是“完全不重平衡”） | 成立，补强 |
| 2 | `has_overflow` 时跳过重平衡 | `:787-792` 建 flag，`:795-797` 直接 `continue` | 成立 |
| 2b | overflow 页本可参与重分配 | 成立且**安全**：调用点在 `rewrite_data_page:500`，早于 `write_overflow_values:519`；页内 overflow value 只是等长占位字节（`DataPageBuilder::add` 仅置 `stored_val_len` 的 overflow 位，`data.rs:140-145`），`collect_entries` 原样带回 `ptr` 与占位字节，`find_value_range` 按 key 定位 | 可改，但需同步迁移 `OverflowSlot.page_idx` |
| 3 | `write_new_data_page` 未 rebalance | `:569-612` 只有 `merge_data_pages_with_options` → `write_overflow_values` → 直接写页；唯一调用点 :142（`root_id == PageId::MAX`） | 成立。`DataPageBuilder` 满了才开新页，尾页可小到 1 条 |
| 4 | `build_index_page_li` 无重平衡 | `:963-995` 确认只有“放不下就 finish 新页”，无合并/再分配；`build_index_pages:899-931`、`build_new_index:933-961` 均如此 | 成立，但归 core-15 |
| — | 引用 `done/22-split-heuristic-divergence.md` | `issue/done/` 下无此文件 | 失效引用，删除 |
| — | 行号 `453-458 / 731-822 / 522-566 / 916-948` | 实际 `500-505 / 778-839 / 569-612 / 963-995` | 全部漂移，需修正 |

### 采纳部分的开发计划（仅 1/2/3，第 4 条不做）

**步骤 A：切分点按字节中点（`rebalance_data_pages` :813-832）**

1. 取 `prev_len = content_len(&page_li[idx-1])`、`cur_len = content_len(&page_li[idx])`，`target = (prev_len + cur_len) >> 1`（对齐 C++ `average_len`）。
2. 单次前向 `DataPageBuilder` 累加，逐条记录 `current_size_estimate()`，选出使 `|size(k) - target|` 最小的 `k0`（`1 <= k0 < n`）。这一步 O(n)，替代现在围绕 `n/2` 的多轮全量重建。
3. 沿用现有 `delta` 探测循环，但以 `k0` 为原点向两侧展开（`k0 ± delta`，上界 `max(k0, n-k0)`），首个两页都成功的 `k` 即为结果；失败则保持原页不变。
4. 保留 `restart_num(prev) > 1`、`cur < 1/4`、`prev > 3/4`、`n >= 2` 四道门槛不变。

**步骤 B：overflow 页参与重平衡（:787-797、:500-505）**

1. `rebalance_data_pages` 签名改 `overflow_li: &mut [OverflowSlot]`，删除 `has_overflow` 跳过逻辑。
2. 重分配成功时，对 `slot.page_idx ∈ {idx-1, idx}` 的 slot 重新定位：在 `entries`（已按 key 升序）上 `binary_search_by_key(&slot.key)` 得 `i`，新 `page_idx = if i < k { idx - 1 } else { idx }`；找不到返回 `KvError::Corrupted`。页数不变，索引范围安全。
3. 调用点 `rewrite_data_page` 改传 `&mut overflow_li`；顺序保持“先 rebalance，后 `write_overflow_values`”。

**步骤 C：空表首写接重平衡（`write_new_data_page` :587 之前）**

1. `merge_data_pages_with_options` 之后、`write_overflow_values` 之前插入 `rebalance_data_pages(&mut page_li, layout.page_size, layout.restart_interval, &mut overflow_li)?`。
2. 因 A/B 已就绪，此处无需额外守卫，行为与 `rewrite_data_page` 一致。

**步骤 D：测试与验收**

1. 单元（`rebalance_tests`）：新增“前页 30 条小 entry + 后页 1 条 1.5KB entry”用例，断言重平衡后两页均落到 `[1/4, 3/4]`（当前实现会静默跳过，用例应先红后绿）；再补一条带 `OverflowSlot` 的用例，断言 slot `page_idx` 跟随迁移且 `find_value_range` 仍可定位。
2. 集成（`crate/engine/fkv_task_write/tests/write.rs`）：`page_size=128` 下空表批量导入变长 value，读回全量校验 + 统计 data 页内容长度分布无 `< 1/4` 尾页（末页除外）。
3. 验收：`./sh/clippy.sh` 零警告、`./test.sh` 全绿；`fixtures/` 若含页布局 golden，需同步复核（见风险 1）。

### 风险

1. **盘面边界变化**：A 会改变 data 页切分，`fixtures/` 或字节级差分用例可能与旧盘面不一致。需确认这些 fixture 是否要求与 C++ 逐字节一致；若一致，先确认 C++ 侧确实按字节均值切分（本地 `cpp/eloqstore` 为空目录，无法直接复核，只能采信 issue 与 core-15 引用的 `batch_write_task.cpp:1118-1265`）。
2. **与 eng-49 耦合**：eng-49 要求 `idx == 0` 保留父索引原分隔键，而重平衡/分裂会真实移动首条 key。两者必须协调：仅“未分裂且未重平衡”时保留父键，否则必须用新首 key，否则路由错乱。
3. **B 的指针语义**：占位字节长度由 `first_pointer_count(value.len(), ...)` 决定，与所在页无关，迁移安全；但任何“重平衡后才写 overflow”的顺序错误会导致 `find_value_range` 定位到新页失败，必须保持 rebalance 在 `write_overflow_values` 之前。
4. **收益上限**：B/C 只影响含 overflow 与空表首写的路径，常规更新收益主要来自 A 的 O(n) 单遍切分；不要把本单当写放大主项（那是 eng-49）。

## DONE（2026-11-04）

按审查结论实现步骤 A/B/C；第 4 条（索引页 Redistribute）按结论不动，交 `core-15` 统一实现。

实现要点（均在 `crate/engine/fkv_task_write/src/lib.rs`）：

- **A 字节中点切分**：弃用条目数中点 `half = n / 2`。新增 `split_by_bytes`：单遍 `DataPageBuilder` 累加，取首个使首页 `current_size_estimate() >= target = (prev_len + cur_len) / 2` 的条目数为起点 `k0`（O(n) 单遍，替代原“围绕 n/2 多轮全量重编码”）；再从 `k0` 向两侧 delta 探测首个两页都放得下的切分（保持“先偏后页”的探测顺序）。`n/2` 在变长 entry 下会把前页切到 < 1/4（新用例复现：旧逻辑 len=390 < 512）。
- **B overflow 参与重分配**：删掉 `has_overflow` 守卫，`overflow_li` 参数改 `&mut [OverflowSlot]`；重分配成功时对落在本对两页的 slot 按新切分键重定位 `page_idx`。因 entry keys 严格升序，用 `slot.key < entries[k].key` 判定所属页（等价于 binary_search，省掉一条不可能触发的 `Corrupted` 分支）；占位长度只由 `first_pointer_count(value.len(), ...)` 决定，迁移不改编码。调用顺序保持 rebalance 早于 `write_overflow_values`。
- **C 空表首写接重平衡**：`write_new_data_page` 在 merge 之后、`write_overflow_values` 之前调用 `rebalance_data_pages`（其分隔键由调用者按真实首键生成，此处的重分配标记无需使用）。

与结论的偏差：B 用 key 比较替代 binary_search（见上）；delta 循环上界由 `0..=half` 改为 `0..n` + 双侧越界即 break（覆盖同一 k 集合）。

改动文件：
- `crate/engine/fkv_task_write/src/lib.rs`（实现 + 3 个单元测试）
- `crate/engine/fkv_task_write/tests/write.rs`（1 个集成测试）

测试命令与结果：
- `cargo nextest run -p fkv_task_write` → 22 passed, 0 failed（新增 `redistribution_splits_by_byte_midpoint`、`redistribution_migrates_overflow_slot`、`fresh_write_rebalances_tail_page`、`bulk_import_empty_store_variable_values`）
- `cargo nextest run -p fkv_store` → 72 passed, 0 failed（集成回归）
- 反向验证：还原 `k0 = n / 2` 时字节中点用例失败（len=390 < 512）；注释掉 `write_new_data_page` 的重平衡调用时空表首写用例失败（尾页 31 < 32 未被合并）。
- `cargo clippy -p fkv_task_write --all-targets --no-deps -- -D warnings` → 零警告；`cargo fmt -p fkv_task_write -- --check` → 干净。

未做/遗留：索引页 `Redistribute`（`build_index_page_li`）→ `core-15`。

## 复审（2026-11-05 r6）

结论：**通过**（步骤 A/B/C 均落地；索引页 Redistribute 按 md 记录归 core-15/sto-67，不在本单验收）。

- A 字节中点：`fkv_task_write/src/lib.rs:1182` `target = (prev_len + cur_len) >> 1`；`:1252-1273` `pick_split` 单遍前缀累加选 k0（`current_size_estimate` 口径 :2479 附近另有 split_lens 逐字节一致性测试）。
- B overflow 参与：`:1160-1165` 签名 `overflow_li: &mut [OverflowSlot]`，无 has_overflow 守卫；`:1195-1203` slot 按新切分键 `slot.key < split_key` 重定位 page_idx。
- C 空表首写：`write_new_data_page`（:872）在 merge 后、`write_overflow_values` 前调 `rebalance_data_pages`。
- 测试：`redistribution_splits_by_byte_midpoint`（:2361，断言两页均落 [1/4, 3/4] :2387-2393）、`redistribution_migrates_overflow_slot`（:2525）、`fresh_write_rebalances_tail_page`（:2559）、集成 `tests/write.rs:903` `bulk_import_empty_store_variable_values`。断言真实覆盖。
