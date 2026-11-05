# core-29 P3：MappingTbl 收缩后再越界 set，Rust 用 INVALID 回填间隙，C++ 保留 stale 值——记录或对齐

来源：第 5 轮 core lane 侦查（子代理摘录；可达性低，主要是盘面字节一致性决策）。

## 证据

Rust `crate/core/fkv_mapping/src/lib.rs:179-181/231-239`：越界 `set` 触发
`resize(index+1, INVALID_VALUE)`，增长路径把 `[old_len, len)` **无条件**填 INVALID。

C++ `page_mapper.cpp:281-295`（`ResizeInternal`）：只对**新分配**的 chunk 填
Invalid，已存在 chunk 尾部的 stale 槽不重填（保留收缩前的旧值，可能形如
FilePageId，被 `ToFilePage` 视作活跃页进 retained set）。

## 问题

abort_batch 回滚（收缩）后稀疏写远端页：两侧 serialize 出的 manifest 字节
不同（oracle 字节差分会挂），GC retained 口径不同。Rust 行为更安全
（INVALID 不会被当活跃页），但按「与 C++ 盘面一致」的验收口径需明确记录。

## 目标

二选一：(a) 记录为有意偏离（doc/migration.md 偏离清单，理由：INVALID 更安全，
且 C++ 的 stale 保留依赖未定义的「恰好没被覆盖」）；(b) 完全对齐 C++ 的
chunk 粒度回填。倾向 (a)——C++ 行为本身像是实现巧合而非契约。

## 验收

- doc/migration.md 偏离清单新增条目（含双方行为描述与理由），或对齐实现 +
  字节级差分测试通过。

## 依赖 / 重叠

- sto-52（reject，replayer 重建口径）不同点；fmt 族（编码格式）不涉及。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上。

## 审查结论（dragon 复核，2026-11-04）

**成立，取方案 (a)：记录为有意偏离 + 用测试锁住 Rust 行为。**

实核（两侧逐行）：

- Rust：`fkv_mapping/src/lib.rs` 的 `set` 走 `resize(index+1, INVALID_VALUE)`；
  `resize` 增长分支显式 `values[offset..end].fill(value)` 覆盖 `[old_len, len)`
  （`:228-239`）；收缩分支**保留 chunk 满长**（注释即写「含 len 之后的陈旧 slot」），
  所以「stale 槽还在、只是不再被增长填」这件事在 Rust 里不成立。
- C++：`MappingTbl::ResizeInternal`（`src/storage/page_mapper.cpp:266-300`）收缩只
  释放整块尾 chunk；增长只 `base_[i]->fill(InvalidValue)` **新分配的 chunk**
  （`init_new_chunks` 循环 `current_chunks..required_chunks`），**同一个未释放 chunk
  的尾部 stale 槽不回填**。
- 因此偏好（同一逻辑状态 → 不同盘面字节）：`abort_batch` 收缩到 5 后再
  `set(39)`，C++ 会把槽位 5..38 里回滚前的 `FilePageId` 序列化出去（对端 replay
  后按活跃页计入 retained，多保留本可回收文件），Rust 写 `Invalid`。
- 可达性确认为真（不是纯理论）：`resize` 收缩分支的注释点明调用方是
  `abort_batch` 回滚；稀疏写回同一 chunk 是常见形态（39 < CHUNK_SIZE=512）。

## DONE（2026-11-04）

- `doc/migration.md` §8.1「代码推断差异」新增一行：**收缩后稀疏写入的回填值（有意偏离）**，
  写清触发场景、两侧行为、后果（字节 + retained 口径）与判定理由（C++ 的 stale
  保留是实现副产物，把已回滚页当活跃页是错误方向）。
- 新增 `crate/core/fkv_mapping/tests/resize_gap_fill.rs`（2 项）锁住行为：
  - `shrink_then_sparse_set_fills_gap_with_invalid`：**自判别**——先断言槽位 6 在
    收缩前是 `FilePageId(7)`，收缩+稀疏增长后再断言 `get(6) == INVALID` 且
    `!= FilePageId(7)`（序列化 roundtrip 同样断言）。若哪天改成 C++ 语义（只填新
    chunk），这组断言必然读到 `FilePageId(7)` 而 FAIL，无需另做行为注入。
  - `shrink_across_chunk_boundary_then_grow_fills_invalid`：对照组，说明偏离范围
    仅限「未释放的 chunk」，跨 chunk 收缩后增长两侧本来就一致。
- `cargo nextest run -p fkv_mapping` **25 passed**；
  `cargo clippy -p fkv_mapping --all-targets --no-deps -- -D warnings` EXIT=0。

## 备注

原「目标」里 (b) 完全对齐 C++ 的 chunk 粒度回填**不采纳**：那会把已回滚页重新
变成 retained（空间泄漏 + 与 Rust 自己的 GC 语义冲突），收益为负。

## 复审（2026-11-05 r6）

通过（有意偏离，勿在后续对齐中"修复"）。`doc/migration.md:219` §8.1 已记录（双方行为 + 判定理由 + 跟踪 core-29）；自判别测试 `fkv_mapping/tests/resize_gap_fill.rs:13 shrink_then_sparse_set_fills_gap_with_invalid`（收缩前槽位=FilePageId(7) → 收缩+稀疏增长后 == INVALID，roundtrip 同断言）与 `:57` 跨 chunk 对照组在位。
