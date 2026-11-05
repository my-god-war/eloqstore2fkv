# fmt-21 index 页 page_id delta 的 i32 域：C++ 有 assert 拦截，Rust `as i32` 静默回绕

## 位置
- Rust: `crate/format/fkv_page/src/index.rs:97`（`let delta = page_id as i32 - last_page_id as i32;` —— `as` 回绕无检查）；`crate/foundation/fkv_codec/src/lib.rs:152-158`（`encode_i32_delta` 对 `i32::MIN` 在 debug 下 `-delta` 溢出 panic）
- C++: `src/storage/index_page_builder.cpp:145-147`（`assert(page_id <= INT32_MAX); int32_t delta = (int32_t) page_id - last_page_id_;`）

## 差异与风险
`page_id ≥ 2^31` 时 Rust 静默编出错误 delta（debug 可能 panic），C++ debug 断言拦截（release 同样回绕）。当前实际页数远未到界，触发条件极端；但 Rust 其余边界（value_len、restart offset）都是显式 `InvalidArgs`，此处风格不一致且无提示。

## 对齐建议
`IndexPageBuilder::add` 对 `page_id > i32::MAX` 返回 `InvalidArgs`（与其余边界检查风格统一）。

## 严重度
P3（防御一致性；触发概率极低）

## 核验记录（2026-11-04）
Rust `as i32` 回绕点实读确认；C++ assert 引自子代理摘录。

## 审查结论（2026-11-04）

**采纳（属实）。**

核验：

- Rust `crate/format/fkv_page/src/index.rs` `IndexPageBuilder::add`：`let delta = page_id as i32 - last_page_id as i32;`，`page_id` 为 `u32`，`as i32` 超域静默回绕。
- `crate/foundation/fkv_codec/src/lib.rs` `encode_i32_delta`：`delta < 0` 分支 `(-delta) as u32`，`i32::MIN` 在 debug 下 `-delta` 溢出 panic（`fkv_codec/tests/codec.rs:76` 刻意用 `i32::MIN + 1` 规避，说明该域是约定）。
- C++ `/Users/z/git/db/kv/eloqstore/src/storage/index_page_builder.cpp:145`：`assert(page_id <= INT32_MAX); int32_t delta = (int32_t) page_id - last_page_id_;`。该 assert 只在**非首条**路径（`IsEmpty()` 早返回之后），release 同样回绕。
- 域内安全性：一旦 `page_id <= INT32_MAX` 且 `last_page_id ∈ [0, INT32_MAX]`，delta ∈ `[-(2^31-1), 2^31-1]`，永不 `i32::MIN`，`encode_i32_delta` 不再可能溢出。

## DONE（2026-11-04）

### 实现要点

- `IndexPageBuilder::add` 在乱序/空 key 检查之后、`restart` 计算之前加 `page_id > i32::MAX as u32 → Err(InvalidArgs)`，
  与其余调用方错误（乱序 key 等）风格一致，且与 C++ assert 位置对齐（首条 leftmost 指针路径不设限，u32 原样存储）。

### 测试

- 新增 `crate/format/fkv_page/tests/format_semantics.rs::index_page_id_beyond_i32_is_rejected`：
  `i32::MAX + 1` 报 `InvalidArgs`，`i32::MAX` 正常写入。
- `cargo nextest run -p fkv_page` → 17 passed / 0 failed。
- `cargo clippy -p fkv_page --all-targets --no-deps -- -D warnings` → 0 警告；`cargo +nightly fmt -p fkv_page` clean。

### 附带说明（未改）

`fkv_codec::encode_i32_delta` 对 `i32::MIN` 仍会 debug panic，但专属调用点只有 `IndexPageBuilder`（已加域守卫），
测试也以 `i32::MIN + 1` 为界；不扩大改动面。

## 复审（2026-11-05 r6）

通过。`crate/format/fkv_page/src/index.rs:119` `page_id > i32::MAX as u32 → Err(InvalidArgs)`；测试 `tests/format_semantics.rs:202-211`（`i32::MAX+1` 拒绝 :205-208、`i32::MAX` 正常 :210）。
