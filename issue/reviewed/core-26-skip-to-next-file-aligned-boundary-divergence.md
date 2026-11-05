# core-26 P2：`skip_to_next_file` 在 max_fp_id 已对齐文件边界时比 C++ 多烧一整个文件的页段

来源：第 5 轮 core lane 侦查，协调者亲验（Rust 侧逐行、C++ 侧对照子代理摘录）。

## 证据（2026-11-05 核验）

`crate/core/fkv_mapping/src/lib.rs:418-427`：

```rust
pub fn skip_to_next_file(&mut self, pages_per_file_shift: u8) {
  if self.space_size(pages_per_file_shift) == 0 { return; }
  let next = ((self.max_fp_id >> pages_per_file_shift) + 1) << pages_per_file_shift;
  self.max_fp_id = next;
  self.min_file_id = next >> pages_per_file_shift;
```

C++（`background_write.cpp:200-212` → `page_mapper.cpp:665-681` `UpdateStat`）：

```cpp
const uint32_t mask = (1u << pages_per_file_shift) - 1;
const FileId next_file = (allocator->MaxFilePageId() + mask) >> pages_per_file_shift;
// UpdateStat：
const FilePageId new_min_fp_id = FileId(min_file_id) << pages_per_file_shift_;
if (new_min_fp_id > max_fp_id_) { max_fp_id_ = new_min_fp_id; }  // 仅严格大于才 snap
```

C++ 是 ceil：max 未对齐时两侧等价（max=5,shift=2 → 8）；**max 已对齐时**
（max=4,shift=2）C++ 得 `4 > 4` 不成立 → max 保持 4、min_file=1；Rust 无条件
`+1<<shift` → max=8、min_file=2。下一次 `allocate` 起步文件号与 C++ 持久分叉
（盘面文件编号/布局不一致，oracle 字节差分会挂）。

现有测试 `skip_to_next_file_zeroes_space`（`fkv_mapping/tests/mapping.rs:63-76`）
只测未对齐（max=6），对齐分支无覆盖。

## 目标

改为 ceil 语义：`next = (max_fp_id + mask) & !mask`，仅当 `next > max_fp_id`
时推进（对齐时保持不动、min_file 同步 max 所在文件）；补对齐边界测试。
注意 `next_file_boundary`（fkv_store，term bump）的 `+1<<` 写法对 term 场景是
对的（replayer.cpp:358-361 同款），不要顺手改。

## 验收

- 测试：max 对齐（4, shift=2）→ 不动；max 未对齐（5）→ 8；空表调用幂等。
- `cargo nextest run -p fkv_mapping`。

## 依赖 / 重叠

- core-22/22a（term bump / allocator 边界）是相邻函数不同语义；sto-56a/b 不覆盖。

## 核验记录

- 本人实读：`fkv_mapping/src/lib.rs:410-435`。
- 子代理摘录（未复核）：C++ 两处行号。

## DONE（2026-11-04）

- `AppendAllocator::skip_to_next_file` 改为 ceil 语义（`(max + mask) & !mask`），
  仅严格大于时推进 `max_fp_id`，`min_file_id` 恒同步为 `max_fp_id >> shift`、
  `empty_file_cnt = 0`；删除 `space_size == 0` 提前返回（新逻辑天然幂等）。
  对齐 C++ `DoCompactDataFile` 空 `fp_ids` 分支 + `UpdateStat` 的 `>` 判定。
- 测试：`skip_to_next_file_zeroes_space` 补幂等断言；新增
  `skip_to_next_file_aligned_keeps_max`（max=4, shift=2 → max 不动、min_file=1、
  下一次分配为 4）。
- 未改 `fkv_store::next_file_boundary`（term bump 场景语义不同）。
- 验证：`cargo check -p fkv_store -p fkv_mapping -p fkv_gc` 通过；
  `cargo nextest run -p fkv_store -p fkv_mapping -p fkv_gc -p fkv_runtime
  --no-fail-fast` 179 通过 / 1 跳过；
  `cargo clippy -p fkv_store -p fkv_mapping -p fkv_gc --no-deps --all-features
  --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_mapping/src/lib.rs:417-425`：ceil 语义 `(max + mask) & !mask`，仅 `next_fp_id > max_fp_id` 才推进，对齐时不动；`min_file_id = max_fp_id >> shift` 同步。测试 `tests/mapping.rs:71`（幂等断言已补）与 `:95 skip_to_next_file_aligned_keeps_max`。`fkv_store::next_file_boundary` 的 `+1<<` 写法确未误改（:148）。
