# fmt-20 页构建器每页新建 + 构造时整页清零再截断：C++ 单 builder Reset() 复用缓冲容量

## 位置
- Rust: `crate/format/fkv_page/src/data.rs:89-95`（`new_page` 分配 `vec![0u8; page_size]` 全零后立刻 `truncate(DATA_HEADER_SIZE)` —— 清零纯属浪费）；`crate/core/fkv_btree/src/lib.rs:443-444`（页满即 `builder = DataPageBuilder::new(...)` 重新分配，`last_key`/`restart_li` 同样重建）
- C++: `src/storage/data_page_builder.cpp:37-49`（`Reset()` 复用 `buffer_`（resize 到 HeaderSize、容量保留），`last_key_` 容量保留）；`src/tasks/batch_write_task.cpp:408,511`（同一个 `data_page_builder_` 反复 Reset）；`src/storage/index_page_builder.cpp:185-194`（另有 `Swap`/`Move` 转移缓冲所有权）

## 现状
批量写 N 页 = N 次 4KB 清零 + N 组 Vec 分配/释放 vs C++ 1 次。

## 差异与风险
小常数，写路径批量场景可测。与 fmt-17（region 免重编码）不重复 —— 本条是「免重复分配」。

## 对齐建议
- `DataPageBuilder`/`IndexPageBuilder` 增加 `reset()` 复用 `buf/restart_li/last_key` 容量（capacity 保留、len 截断）；
- 构造路径避免「先整页清零再截断」：`Vec::with_capacity` + 显式写头部即可（`IoBufMut::as_uninit` 语义下读回页无需清零，写路径同理）。

## 严重度
P2（性能，影响低但确定）

## 核验记录（2026-11-04）
Rust 清零后截断、每页重建实读确认；C++ Reset 复用引用自子代理摘录。

## 审查结论（2026-11-04）

**部分采纳，已拆分。**

- 「构造时整页清零再截断」属实且可改（`fkv_page` 在本 lane 内）→ 拆为 `fmt-20a-page-builder-construct-buffer-zeroing.md`，**已实现并 DONE**。
- 「页满重建 builder / 缺 `Reset()` 复用」属实，但唯一调用点在禁改文件（`fkv_btree/src/lib.rs`）与本 lane 不覆盖的 `fkv_task_write` → 拆为 `fmt-20b-page-builder-reset-reuse.md`，标 `## 阻塞`。

C++ 依据（本地仓 `/Users/z/git/db/kv/eloqstore`）复核：
`src/storage/data_page_builder.cpp:37-49` `Reset()`；`src/tasks/batch_write_task.cpp:408,511` 反复 Reset。

Rust 实读复核：`crate/format/fkv_page/src/data.rs` `new`（清零后 truncate）、`crate/core/fkv_btree/src/lib.rs:479`（页满重建）。

本 issue 已拆分，后续跟踪见上述两个子项文件。

## 复审（2026-11-05 r6）

通过（作为拆分父条目）。20a（构造清零）已落地并复核（见该文件）；20b（Reset 复用）经 fmt-17b 的 `IndexPageBuilder::reset`（index.rs:65）+ `encode_region` 内部复用（fmt-17b DONE 亦声明覆盖该场景）落地，reject/ 中的 20b 为当时「禁改文件」阻塞记录，现状已被 17b 吸收。
