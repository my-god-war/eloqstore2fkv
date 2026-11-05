# sto-57a `parse_size` 单位文法对齐 C++（只认 KB/MB/GB/TB 两位尾缀）

## 位置
- `crate/foundation/fkv_config/src/lib.rs::parse_size`
- C++：`src/kv_options.cpp:20-84`（`ParseSizeWithUnit`）

## 现状
Rust 接受 `b/k/m/g/t` 单字母与 `kb/mb/gb/tb`；C++ 只匹配末两位 `KB/MB/GB/TB`（大小写不敏感），
`"400M"` 在 `from_chars` 残留字符处失败并静默返回 0。

## 方案
1. 单位集合收窄为 `""`（字节）+ 两位尾缀，用 `eq_ignore_ascii_case` 逐项比较，零分配（去掉 `to_ascii_lowercase` 的 `String`）。
2. 非法单位保持 `InvalidArgs`（不采纳 C++ 的静默 0）：宁可启动失败也不静默降级，注释里写明是有意差异。

## 验收
- `buffer_pool_size=400MB` / `1gb` / 纯数字按字节均正确。
- `400M`、`400 B`、`400XB` 返回 `InvalidArgs`。
- `tests/config.rs::ini_size_units_match_cpp` 覆盖以上字段级用例。

## DONE（2026-11-04）

- `parse_size` 改为 `eq_ignore_ascii_case` 比较 `kb/mb/gb/tb`，无单位按字节；单字母与其它尾缀 → `InvalidArgs`；去掉 `to_ascii_lowercase` 的 `String` 分配。
- 测试：`tests/config.rs::ini_size_units_match_cpp`（400MB/1gb/128 字节 + 3 个非法单位）。
- `cargo nextest run -p fkv_config` 12 passed；clippy `-D warnings` clean；nightly fmt clean。

## 复审（2026-11-05 r6b）

通过。

- `parse_size`（`crate/foundation/fkv_config/src/lib.rs:573-596`）：单位仅 `""`/`kb`/`mb`/`gb`/`tb` 的 `eq_ignore_ascii_case` 比较（`:583-593`），非法单位 `InvalidArgs`（`:594`），无 `to_ascii_lowercase` 分配；`:577-580` 注释写明对齐 `ParseSizeWithUnit` 且保留 InvalidArgs 的有意差异。
- 测试：`crate/foundation/fkv_config/tests/config.rs:278 ini_size_units_match_cpp` 在盘。
