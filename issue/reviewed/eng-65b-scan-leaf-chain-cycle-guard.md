# eng-65b 扫描叶子链环检测（O(1) 上界）

## 位置
- `crate/engine/fkv_task_scan/src/lib.rs:180-273`（叶子链 loop）
- `crate/core/fkv_mapping/src/lib.rs`（新增 `PageMapper::page_cnt`）
- `crate/engine/fkv_task_scan/tests/scan.rs`

## 现状
叶子推进只看 `next_page_id == PageId::MAX` 终止。陈旧 next 链成环时无限循环，
比串页更难定位；`MAX_TREE_DEPTH` 只约束下降，不约束叶子链。

## 方案
1. `PageMapper` 新增只读 `page_cnt() -> usize`（`tbl.len()`，逻辑页槽位数，含空闲槽）。
   链访问的逻辑页互异，槽位数即合法链长上界。
2. 扫描叶子 loop 维护 `leaf_cnt`：
   - 每轮 `+= 1`，`> mapper.page_cnt()` 返回 `KvError::Corrupted`；
   - `next_page == page_id`（自环）直接返回 `Corrupted`。
   - 提前返回发生在 loop 顶部，上一轮 prefetch 已 drain，无在飞 IO。
3. 正常路径只多一次计数与比较，无分配、无集合。

## 验收
- 新增测试：`p0 -> p1 -> p0` 成环，扫描返回 `Err(Corrupted)` 而非挂死；
  正常链结果不变。
- `cargo nextest run -p fkv_task_scan` 全绿。
- `cargo clippy -p fkv_task_scan --no-deps --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

### 实现要点
- `crate/core/fkv_mapping/src/lib.rs`：新增 `PageMapper::page_cnt()`，返回
  `tbl.len()`（逻辑页槽位数，含空闲槽），作为互异逻辑页访问序列的长度上界。
- `crate/engine/fkv_task_scan/src/lib.rs`：
  - 叶子链 loop 顶部维护 `leaf_cnt`；`leaf_cnt > mapper.page_cnt()` 或
    `next_page == page_id`（自环）返回 `KvError::Corrupted`，替代无限推进。
  - 换页后补 `page_id = next_page`，使自环判定与当前叶子一致。
  - 提前返回发生在 loop 顶部，上一轮 prefetch 已 drain，无在飞 IO / 借用冲突。
  - 正常路径仅一次计数与比较，零分配。

### 测试
- `crate/engine/fkv_task_scan/tests/scan.rs::scan_leaf_chain_cycle_detected`
  - `p0 -> p1 -> p0` 成环，`scan_range` 返回 `KvError::Corrupted`（此前会挂死）。
  - 既有 `scan_chain` / `scan_page_entry_limit_semantics` 覆盖正常链不变。

### 验证
- `cargo nextest run -p fkv_store -p fkv_task_scan` → 72 passed / 0 failed
- `cargo clippy -p fkv_task_scan -p fkv_mapping --no-deps --all-features --all-targets -- -D warnings` → 零警告
- `cargo fmt -p fkv_task_scan -p fkv_mapping -- --check` → 通过

## 复审（2026-11-05 r6）

通过。实现与测试均在：

- `fkv_task_scan/src/lib.rs:176-185`：`leaf_cnt_max = snapshot.data_tbl().len()`（逻辑页
  槽位数，与 `PageMapper::page_cnt()` 同源 `tbl.len()`；27b 快照化后改从 snapshot 取），
  每页 `leaf_cnt += 1`，`> leaf_cnt_max` 或 `next_page == page_id`（自环）返回
  `KvError::Corrupted`；判定在 loop 顶部、prefetch 已 drain。
- `PageMapper::page_cnt()` 仍在 `fkv_mapping/src/lib.rs:871`（`self.tbl.len()`）。
- 测试 `fkv_task_scan/tests/scan.rs:217-254`：`p0 -> p1 -> p0` 成环断言
  `err == KvError::Corrupted`（:253）。

遗留（不阻塞）：27b 之后 scan 用 `snapshot.data_tbl().len()`，`PageMapper::page_cnt()`
当前全仓零调用，成为孤儿 pub API（见 lane 报告遗留风险）。
