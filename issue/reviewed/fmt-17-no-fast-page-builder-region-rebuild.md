# fmt-17 缺 PageRegionIter / FastPageBuilder，index 页重建逐 entry 重编码

## 位置
- Rust: `crate/format/fkv_page/` 无 region 相关模块；`crate/core/fkv_btree/src/lib.rs:376-433`（index 页重建走 `IndexPageBuilder::add` 逐条 varint 重编码）
- C++: `eloqstore/src/storage/data_page_builder.cpp:248-308`（`FastPageBuilder::AddRegion`：region 字节直接 `memcpy` 进页）、`eloqstore/src/storage/data_page.cpp:539-584`（`PageRegionIter`：按 restart 数组切 region）、调用点 `eloqstore/src/tasks/batch_write_task.cpp:1128-1174, 1202-1255`（重建 leaf/non-leaf index 页时复用旧页的 region 字节）

## 现状
Rust 重建 index 页（COW 合并后重建 leaf/非 leaf 层）时，把旧页每条 entry 解码成 `(key, page_id)` 再逐条 `IndexPageBuilder::add` 重新 varint 编码。

## C++ 实现
C++ `FastPageBuilder` 把旧 index 页按 restart 数组切成 region（region = 从某 restart 点到下一 restart 点的原始字节串），重建时 `AddRegion` 直接 `memcpy` 原字节进新页，只对新增/变更 entry 做编码。region 字节与逐条编码产物逐字节一致（restart 点处 `shared=0`，region 边界即 entry 边界）。

## 差异与风险
- 输出字节等价（region memcpy ≡ 逐条编码），不破坏兼容；纯性能差异。
- Rust 逐条重编码：每条 entry 多次 `put_varint32` + `extend_from_slice` + `common_prefix` 计算；C++ memcpy 一整段。大 index 页（几百 entry）重建时 Rust 慢一个数量级量级的常数。
- 影响写路径 p99（验收指标：批量写吞吐 ≥ 0.85×、冷点读 p99 ≤ 1.2× —— 重建只在写路径，读不受影响）。

## 对齐建议
- `fkv_page` 增加 `region` 模块：`PageRegionIter`（按 restart 数组切 region）+ `FastIndexPageBuilder`（`add_region` memcpy + `finish` 补 restart 数组/content len/checksum），`fkv_btree` 重建时对未变更的连续 entry 用 region 直拷。
- region 边界要求 restart 点处 `shared=0`（builder 保证），重建时校验 `shared == 0` 否则回退逐条编码。

## 性能优化建议
就是本条：region memcpy 替代逐条 varint 编码，写吞吐对齐 C++。

## 严重度
P2（性能/健壮性）

## 审查结论

结论：性能方向成立，但 issue 的代码定位有两处事实错误；按本次约束（只改 `fkv_page` / `fkv_btree`）先交付 region 原语，端到端接线留作后续。

1. **位置论断错误**：`crate/core/fkv_btree/src/lib.rs:376-433` 是 `merge_data_pages_with_options` 的 **data page** 合并/重编码（L377-395 batch 覆盖、L397-480 `DataPageBuilder` 逐条写），与 index 页无关。`fkv_btree` 全文（544 行）只有 `seek_index`、`IndexNode`、`IndexCache`、`merge_data_pages*`，**没有 index 页重建函数**。
2. **Rust 真实重建点在别的 crate**：`crate/engine/fkv_task_write/src/lib.rs:963-995` 的 `build_index_page_li`，L978-992 对每个 child 逐条 `IndexPageBuilder::add`；调用方 `build_index_pages`（L899-931，用 `read_index_children` L875-884 解码旧页后替换一个 child）和 `build_new_index`（L933-961）。COW 写只替换一个 child，其余 child 全部重编码，因此 region 直拷确有收益。
3. **C++ 调用点论断偏差**：`batch_write_task.cpp:1128-1174` 是 data 页 `Redistribute`，`1202-1255` 是 index 页 `Redistribute`，都是**页分裂/重分配**，只对 prev 页保留的 restart 前缀 region 直拷（L1152-1174、L1248-1259），并非“COW 合并后重建整页”。结论仍成立：C++ 有 region 直拷路径，Rust 没有。
4. **字节等价条件成立**：restart 点 `shared=0`（`index.rs:89-95` 的 restart 分支、`index.rs:107-112` 记录 restart offset），`index.rs:143-151` 写 restart 数组；因此 region 边界即 entry 边界，region 直拷与逐条重编码逐字节一致（checksum 除外，`finish()` 再补）。
5. **范围**：issue 要求 `fkv_btree` 重建时用 region，但 `fkv_btree` 无重建入口；真正接线需改 `fkv_task_write`，超出本次允许范围。本轮交付 `fkv_page` 的 `PageRegionIter` + `FastIndexPageBuilder` 与字节等价回归测试，`build_index_page_li` 采用 region 作为后续项。

## 开发计划

1. `crate/format/fkv_page/src/region.rs`
   - `PageRegionIter`：按 `content_len` / `restart_num` 计算 restart 数组位置，构造时校验 offset 严格递增且落在 `[header, restart_offset)`；`valid/next/region`，末尾 region 以 `restart_offset` 收尾。
   - `FastIndexPageBuilder`：`new(page_size, leaf)` / `from_page(old)`（保留 header 的 leftmost ptr 等）、`add_region`（容量检查 + `extend_from_slice` 直拷 + 记录 offset）、`current_size`、`finish` / `finish_without_checksum`（补 restart 数组、content_len、checksum）。
   - `crate/format/fkv_page/src/lib.rs` 导出 `region` 模块与两个类型。
2. 回归测试 `crate/format/fkv_page/tests/region.rs`
   - region 拼回 == 页内 `[header, restart_offset)`；每个 region 首字节 varint `shared == 0`。
   - `FastIndexPageBuilder` 逐 region 重建 == `IndexPageBuilder` 逐条编码（多种 restart_interval、leaf/non-leaf），含 `IndexPageIter` 序列一致与 checksum 校验。
   - 空 index 页（只有 leftmost）region 数为 0；容量不足 `add_region` 返回 `false`；data 页也能被 `PageRegionIter` 正确切分。
3. 验证：`cargo fmt`；`cargo test -p fkv_page -p fkv_btree -p fkv_task_write`。

## DONE（2026-11-04）

按审查结论交付 region 原语（第 1/2 步）。**第 3 步「`build_index_page_li` 端到端接线」独立拆到 `issue/todo/fmt-17b-index-rebuild-region-reuse.md`**（原结论即把它留作后续；本 lane 虽允许改 `fkv_task_write`，但接线需给快速重建器补 entry 编码与旧页 region 对齐逻辑，属独立一件事，单列更清晰）。

改动文件：
- `crate/format/fkv_page/src/region.rs`（新增）
- `crate/format/fkv_page/src/lib.rs`（导出 `PageRegionIter` / `FastIndexPageBuilder`）
- `crate/format/fkv_page/tests/region.rs`（新增 6 个测试）

实现要点：
- `PageRegionIter`：按 restart 数组切 region 的 `Iterator<Item = &[u8]>`。构造期完成全部校验（页类型 data/index、`content_len`/`restart_num` 边界、offset 首项 == 页头、其余严格递增且 `< restart_offset`、0 region 时 `restart_offset == 页头`），故 `next` 为 O(1) 且不可能失败；空 index 页 region 数为 0。
- `FastIndexPageBuilder`：`new` / `from_page`（复用旧页页类型 + leftmost 指针）/ `set_leftmost` / `current_size` / `add_region`（容量不足返回 `false`，对齐 C++ `FastPageBuilder::AddRegion` 的 `current_size + len + 2 > page_size` 判定）/ `finish(_without_checksum)`。复用现有 `data_restart_offset` 计算 restart 数组位置，未复制公式。

测试命令与结果：
- `cargo nextest run -p fkv_page` → 23 passed, 0 failed（新增 `regions_partition_entry_area`、`data_page_regions_partition_entry_area`、`region_rebuild_is_byte_identical`、`empty_index_page_has_no_regions`、`add_region_enforces_capacity`、`rejects_corrupt_restart_array`）。关键断言：region 直拷重建的 index 页与 `IndexPageBuilder` 逐条重编码产物**逐字节一致**（含 checksum），`IndexPageIter` 序列一致。
- `cargo clippy -p fkv_page --all-targets --no-deps -- -D warnings` → 零警告；`cargo fmt -p fkv_page -- --check` → 干净。
- 回归：`cargo nextest run -p fkv_task_write` → 22 passed；`-p fkv_store` → 72 passed（未触及这些 crate 的调用路径）。

### 复核补强（2026-11-04，第二会话）

在原交付上补三处健壮性与测试：

- `PageRegionIter`：空 data 页（`restart_num=1` 且唯一 offset 为页头）归一为 0 个 region，此前会被 `offset >= restart_offset` 判为 `Corrupted`；非空页仍要求 offset 严格递增且 `< restart_offset`。
- `FastIndexPageBuilder::add_region`：拒绝空 region（`InvalidArgs`）与非 restart 首字节（`shared != 0` → `Corrupted`），避免拼出非法 restart 数组。
- 新增测试：`empty_data_page_has_no_regions`、`add_region_rejects_non_restart_bytes`。

复验：`cargo nextest run -p fkv_page` 25 passed；`-p fkv_btree -p fkv_task_write` 41 passed；`cargo clippy -p fkv_page -p fkv_btree --no-deps --all-features --all-targets -- -D warnings` 零警告；`cargo fmt --check` 干净。盘面字节不变（`region_rebuild_is_byte_identical` 逐字节一致）。

## 复审（2026-11-05 r6）

通过。`crate/format/fkv_page/src/region.rs`（231 行）实现 `PageRegionIter`/`FastIndexPageBuilder`，`src/lib.rs:27` 导出（含 `region_entry_cnt`）；`tests/region.rs` 8 用例齐全（regions_partition:32、byte_identical:83（`assert_eq!(rebuilt, page)` + checksum + iter 序列）、capacity:116、corrupt_restart:152、补强用例 empty_data:168 / rejects_non_restart:177）。端到端接线归 fmt-17b（已另核）。
