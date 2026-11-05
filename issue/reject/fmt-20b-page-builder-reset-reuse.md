
## 审查结论（2026-11-04 复审）

**不做（no-go，非缺陷）。** 原文断言「C++ 单 builder `Reset()` 复用缓冲，Rust 每页新建」属实，但作为「对齐项」在本仓库不成立：

1. **C++ 复用缓冲的代价是每页一次拷贝。** `data_page_builder.cpp:38-49` `Reset()` 用 `buffer_.clear(); buffer_.resize(HeaderSize(), 0)` 保住容量；但 `Finish()` 返回的是指向 `buffer_` 的 `string_view`（头文件注释明写「valid until Reset() is called」），所以调用方必须**在 Reset 前把页字节拷走**：`batch_write_task.cpp:1067` `std::string_view page_view = data_page_builder_.Finish();`，随即 `:1095` / `:1110` 都有 `memcpy(new_page.PagePtr(), page_view.data(), page_view.size())`。即 C++ 每页省 1 次 alloc，换来 1 次 `page_size` memcpy。
2. **Rust 是零拷贝移交。** `finish(self) -> Result<Vec<u8>>` 直接移出 `Vec` 所有权（页进 `page_li`/写队列），无 memcpy，代价是每页 1 次 alloc。两边都是「每页 O(page_size) 的量级工作」，不存在 Rust 多付一页拷贝的劣势；强行引入 `reset()` 复用页缓冲必然要改成 `finish_into(&mut Vec)` 拷贝，反而引入 memcpy。
3. **真正可复用的只有 `restart_li` / `last_key`**（每页 2 个小 Vec 分配），收益微小；且核心调用点 `PageSink`（`fkv_btree/src/lib.rs:479`）是禁改热点文件，`fkv_task_write` 的 index 循环每页只建 1 个（页数远少于 data 页），投入产出不成立。
4. 页缓冲复用的正解是**页缓冲池**，即 `core-18-pages-pool-fake-no-reuse-chunk-arena-missing.md` 的范畴，而不是 builder `Reset()`。

### 已落地的相邻收益

`fmt-20a` 已消掉构造期整页 memset（`Vec::with_capacity` + 按 header 长度初始化，`finish` 仍补零），
并用 C++ oracle 金标证明产物逐字节不变（`fmt-16a`）。

结论：本子项关闭，不再开发。
