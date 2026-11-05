# fmt-17b 用 region 直拷接线 index 页重建（`build_index_pages` 复用旧页 region）

## 来源
`issue/done/fmt-17-no-fast-page-builder-region-rebuild.md` 的后续项（原审查结论第 5 点 + 计划第 3 步）。
上游已交付原语：`fkv_page::{PageRegionIter, FastIndexPageBuilder}`（`crate/format/fkv_page/src/region.rs`），
且已用测试确认「region 直拷重建 == 逐条重编码」逐字节一致。

## 目标（只做这一件事）
把 `crate/engine/fkv_task_write/src/lib.rs` 的 `build_index_pages` / `build_index_page_li` 从
「对旧页每个 child 逐条 `IndexPageBuilder::add` 重编码」改为「未变更的连续 region 直接 `add_region` 直拷，
仅对变更区段重编码」，消除 COW 写路径对未变 index entry 的重复 varint 编码。

## 现状（源码事实）
- 重建入口：`build_index_pages`（约 :1144）→ `build_index_page_li`（约 :1208），后者对 `children[1..]` 逐条 `builder.add(&child.first_key, child.page_id)`。
- `build_index_pages` 仅在 `child_li != frame.children` 时被调用（`apply_index_changes`，约 :485 判等）；调用方持有旧子项快照 `frame.children`、旧页 `page_id`、新子项 `child_li`，但当前**未把旧子项/旧页字节传入** `build_index_pages`。
- index 页 entry i ↔ children[i+1]（children[0] 是 header 里的 leftmost，不编码）；restart 点每 `layout.page.restart_interval` 条 entry 一次，故 region r 覆盖 children 下标 `[r*ri + 1, (r+1)*ri + 1)`（末区段为余数）。

## 实现方案（须与现有字节产物逐字节一致）
1. `build_index_pages` 增加 `old_children: &[Child]`（或直接传 `&Frame`）与旧页字节（可从 `io.read_page(old_page_id)` 读，或在 `apply_index_changes` 里已读到的 `page` 传入）。
2. 单页快路径（`build_index_page_li` 只产出 1 页，且旧页也是 1 页、leaf 一致）：
   - 用 `PageRegionIter::new(old_page)` 枚举旧 region。
   - 对第 r 个 region，取其在**新** children 上的对应 entry 切片 `children[r*ri + 1 .. min((r+1)*ri + 1, children.len())]`：
     - 与旧 `old_children` 同下标切片**元素逐一相等**（`Child` 的 `first_key` + `page_id`）→ `FastIndexPageBuilder::from_page(old_page)` + `add_region(旧 region 字节)`；
     - 否则需重编码该区段并 `add_region`（见下）。
   - 若新增/删除导致下标漂移，越往后越不匹配，自动退化为重编码，正确性不受影响（只是复用率下降）。
3. 重编码区段：当前 `FastIndexPageBuilder` 只能 `add_region`。两种落地方式二选一：
   - (推荐) 给快速重建器补 `FastIndexPageBuilder::add(key, page_id)`：进入新 region 后首条强制 `shared=0`、`page_id` 绝对值（delta 基数 0），其后沿用 `IndexPageBuilder` 的共享前缀 + i32 delta 编码；`add_region` 与 `add` 可交错，`add_region` 后重置 run 状态。**注意避免与 `IndexPageBuilder::add` 逻辑重复**：应把 entry 编码抽成两者共用的内部函数/结构（`fkv_page` 内）。
   - 或提供 `fkv_page` 内部 `encode_entry_region(entries: &[(key, page_id)]) -> Vec<u8>`（同一编码函数的薄封装）。
4. 多页（分裂）或任何不一致情形一律回退到现有逐条重编码路径，保证若非 fast path 命中则行为完全不变。
5. 不改变 `Child.first_key`/分隔键语义（分隔键仍是真实首键，见 fmt-19）。

## 验收
- 新增单元测试：构造旧 index 页 → 替换其中一个 child（page_id 变化、key 不变）→ 断言 fast path 产物与「逐条重编码」产物**逐字节一致**，且 `IndexPageIter` 序列一致、checksum 通过。
- 断言命中 fast path（例如统计 `add_region` 次数 > 0 / 重编码条目数 < 总条目数），证明优化生效。
- `cargo nextest run -p fkv_page -p fkv_task_write -p fkv_store` 全绿；`cargo clippy -p fkv_page -p fkv_task_write --all-targets --no-deps -- -D warnings` 零警告；`cargo fmt`。

## 约束
- C++ 依据：`eloqstore/src/storage/data_page_builder.cpp`（`FastPageBuilder::AddRegion`）、`data_page.cpp`（`PageRegionIter`）、`tasks/batch_write_task.cpp:1152-1174 / 1248-1259`（保留前缀 region 直拷）。
- 与并发会话热点文件无交集（本项改 `fkv_page` + `fkv_task_write`，均在本 lane 允许列表内）；若 `build_index_pages` 已被其它会话改动，先重读再改。

## DONE（2026-11-04）

接线完成：COW 写路径重建 index 页时，未变更的整段 region 直接 `memcpy` 直拷，仅变更区段重新 varint 编码。产物与逐条重编码**逐字节一致**（region 边界即 restart 点，self-contained）。

改动文件：
- `crate/format/fkv_page/src/index.rs`：`IndexPageBuilder` 新增 `reset()` / `encode_region()`，抽出 `encode_entry()` 供 `add` 与 `encode_region` 共用（不重复编码逻辑）。
- `crate/format/fkv_page/src/region.rs`：新增 `region_entry_cnt()`（走 varint 跳步数一个 region 的 entry 数，用于对齐校验）。
- `crate/format/fkv_page/src/lib.rs`：导出 `region_entry_cnt`。
- `crate/engine/fkv_task_write/src/lib.rs`：`Frame` 增存本批开始时的旧页字节；`build_index_pages` 入参收敛为 `IndexRebuild`（顺带消掉 clippy `too_many_arguments`），新增 `build_index_page_region` 快路径；`build_index_pages_prefers_region_reuse` / `write_batch_uses_region_reuse` / `region_reuse_matches_entry_rebuild` / `region_reuse_rejects_restart_interval_mismatch` 四个测试。

实现要点：
- **缓冲复用（正是 fmt-20b reject 里说的“finish 后丢弃缓冲”场景）**：`IndexPageBuilder::encode_region` 内部 `reset()` 复用页缓冲，变更区段重编码不再每段新分配。
- **旧页字节来源**：`RouteCursor` 读 index 页时把整页字节存进 `Frame`（一次 `page_size` 拷贝），避免重建时二次读盘。
- **对齐校验（安全关键）**：直拷前验证 ① `region 数 == ceil(旧 entry 数 / ri)`、② 首 region 的 entry 数 `== min(ri, 旧 entry 数)`，二者共同证明旧页按当前 `restart_interval` 均匀切 region；否则 region 与 chunk 错位会串页 → 返回 `None` 回退。末 chunk 额外核对 entry 数（新页变短时尾区段可能短于旧 region）。
- **回退**：页型/尺寸不符、间距不符、单页放不下（分裂）一律回退原 `build_index_page_li` 路径，未命中时行为与改动前完全相同。

测试命令与结果：
- `cargo nextest run -p fkv_page -p fkv_btree -p fkv_task_write -p fkv_store` → 153 passed, 0 failed。
  - `region_reuse_matches_entry_rebuild`：全不变 / 改 page_id / 改分隔键 / children 变短变长，4 种情形下快路径产物 == `build_index_page_li` 产物的**逐字节**比对（含 `read_index_children` 回读校验）。
  - `region_reuse_rejects_restart_interval_mismatch`：旧页按 ri=3、当前 ri=4（region 数巧合相同）必须返回 `None`，挡住错位串页。
  - `write_batch_uses_region_reuse`：真实 `write_batch`（第二批插入致末位 data 页分裂）命中快路径（`cfg(test)` 计数器验证），端到端证明接线生效。
- `cargo clippy -p fkv_page -p fkv_task_write --all-targets --no-deps -- -D warnings` → 零警告；`cargo fmt --check` 干净。

备注：`region.rs` 期间被并发会话加固（空 data 页归一、`add_region` 校验 `shared==0`、`offset_at` 改不可失败），本次改动为纯追加，无冲突。

## 复审（2026-11-05 r6）

通过。消费端在位：`fkv_task_write/src/lib.rs:1587-1756`（`build_index_page_region`：region 数 == ceil(entries/ri) 与首 region entry 数 == min(ri, n) 双重对齐校验 :1716-1727，`FastIndexPageBuilder::from_page` + `add_region` 直拷 :1733-1756）；fkv_page 侧 `index.rs:65/99/113`（`reset`/`encode_region`/`encode_entry`）；4 个测试在 `src/lib.rs:1902/1973/2011/2073`。落点主要在 task_write（write lane 热点文件），本轮仅核对其与 fkv_page 原语的契约。
