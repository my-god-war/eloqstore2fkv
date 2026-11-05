# fmt-20a fkv_page 构造缓冲：整页清零再截断 → 按 header 长度懒初始化

源自 fmt-20（已拆分）。只做「免重复分配」中的**构造清零**部分，落在本 lane 允许的 `fkv_page`。

## 原问题
`header::new_page` 分配 `vec![0u8; page_size]`（整页 memset）后立刻 `truncate(DATA_HEADER_SIZE)`，
`IndexPageBuilder::new` 还额外重复调用一次 `set_page_type`（`new_page` 内已设）。
`finish_inner` 的 `resize(page_size, 0)` 会把尾部再补零一次，故构造期整页 memset 纯属浪费。

## 审查结论（2026-11-04）

**采纳。** `crate/format/fkv_page/src/data.rs:89-90`、`index.rs:34-50` 实读确认；溢出页 `overflow.rs:98-99` 亦重复 `set_page_type`。

## DONE（2026-11-04）

### 实现要点

- `header::new_page(page_size, page_type, len)`：改为 `Vec::with_capacity(page_size)` + `resize(len, 0)`，
  只初始化前 `len` 字节（builder 传 header 长度，overflow 传 `page_size`），尾部零填充延迟到 `finish`。
  新增 `len > page_size` 不变量检查，避免 `finish` 静默截断。
- `DataPageBuilder::new`：`new_page(page_size, Data, DATA_HEADER_SIZE)`，去掉 `truncate`。
- `IndexPageBuilder::new`：`page_type` 先算一次，`new_page(..., INDEX_HEADER_SIZE)`，删掉重复 `set_page_type`。
- `overflow::build_inner`：`new_page(page_size, Overflow, page_size)`，删掉重复 `set_page_type`（保持整页置零，checksum 覆盖全页）。
- 输出逐字节不变（`finish` 仍 `resize(page_size, 0)` 补零）。

### 测试

- 新增 `crate/format/fkv_page/tests/format_semantics.rs::builder_padding_is_zero_filled`：断言 data/index 页 `content_len` 之后 padding 全为 0。
- `cargo nextest run -p fkv_page` → 16 passed / 0 failed。
- `cargo nextest run -p fkv_btree -p fkv_large`（下游 builder 消费方）→ 35 passed / 0 failed。
- `cargo clippy -p fkv_page --all-targets --no-deps -- -D warnings` → 0 警告；`cargo +nightly fmt -p fkv_page` clean。

### 复核：与并发 WIP 的隔离

`cargo nextest run -p fkv_task_write` 期间出现 1 个失败（先 `fresh_write_rebalances_tail_page`，后 `redistribution_splits_by_byte_midpoint`），
但 `crate/engine/fkv_task_write/src/lib.rs` 当时**处于并发会话的未提交编辑中**（`git status` 显示 ` M`）：

- A/B 实验：把 `new_page` 临时改回「整页清零 + truncate」，同一条测试仍随对方文件状态在通过与失败间翻转；
- 恢复懒初始化后，`fresh_write_rebalances_tail_page` 连续 3 次全过，随后失败点换成了另一个 redistribution 单测，测试总数也从 22 变 21（对方在改文件）。

结论：与本改动无关。本改动只影响构造期内存初始化，`finish` 仍 `resize(page_size, 0)` 补零，产物逐字节一致；`fkv_btree`（35 tests 含 fkv_large）与 `fkv_page`（17 tests）全绿。

## 复审（2026-11-05 r6）

通过。`crate/format/fkv_page/src/header.rs:95-103`：`Vec::with_capacity(page_size)` + `resize(len, 0)` 懒初始化，含 `len > page_size` 不变量检查；测试 `tests/format_semantics.rs:163 builder_padding_is_zero_filled`（data/index 页 padding 全 0 断言）。
