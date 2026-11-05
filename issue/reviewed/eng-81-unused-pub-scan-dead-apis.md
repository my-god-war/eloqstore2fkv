# eng-81 unused.js 扫描：9 个零引用 pub 方法可直接删除，`as_uninit` 为 trait 误报

## 来源

`./sh/unused.js`（2026-11-04）输出“未引用 pub 方法”。脚本配置
`cargo.allTargets=false`、`cfg.setTest=false`，因此扫描**不含测试**；
对下表每一项又做了全仓（含 `tests/`、`tool/`）`grep`，确认连测试也没有调用。

## 可直接删除（9 项）

| 符号 | 位置 | 证据 / 删除理由 |
|---|---|---|
| `PooledFilePages::free_cnt` | `crate/core/fkv_mapping/src/lib.rs:324` | 仅返回 `free_li.len()` 的诊断 getter；全仓 0 引用。Pooled 当前只在 core 测试构造，且测试也不用它 |
| `prepare_standalone` | `crate/format/fkv_compress/src/lib.rs:151` | 与 `DictCompression::prepare` 的超阈值分支重复；写路径只调 `prepare`，`compress_raw` 已直接被 `prepare` 使用。删掉后无行为变化 |
| `DictCompression::clear_samples` | `crate/format/fkv_compress/src/lib.rs:233` | `build_dictionary` 已用 `take(&mut sample_data)` / `take(&mut sample_size_li)` 释放样本，不存在“训练后样本残留”；该 helper 0 引用 |
| `varint64_size` / `varint32_size` | `crate/format/fkv_manifest/src/lib.rs:360` / `:364` | 纯转发 `fkv_codec::varint64_len` / `varint32_len`；调用方直接用 codec 即可 |
| `read_varint64` / `read_varint32` | `crate/format/fkv_manifest/src/lib.rs:368` / `:372` | 纯转发 `fkv_codec::get_varint64` / `get_varint32`；manifest 自身解析走 `Cursor`，不用这两个包装 |
| `put_data_header` | `crate/format/fkv_page/src/data.rs:538` | 组合了 `set_page_type + set_prev/next`；写路径的实际调用是 `set_prev_page_id` / `set_next_page_id` / `set_checksum`（`fkv_task_write/src/lib.rs:689-691,757-759`），没有地方用整包 helper |
| `put_data_checksum` | `crate/format/fkv_page/src/data.rs:557` | 只有 `set_checksum(page)` 一行；直接调用 `set_checksum` 更清晰 |

删除时同步清理 `fkv_page/src/lib.rs` 的 re-export（`put_data_header` /
`put_data_checksum` 当前未导出，若后续导出需一并删）。

## 已由 eng-78 覆盖，不重复

扫描同时命中 `MappingTbl::tbl_mut`（`:554`）、`PageMapper::take_dirty_page_li`
（`:571`）、`PageMapper::allocate_file_page`（`:622`），这 3 项已在
`eng-78` 的死代码清单中，按该条处理即可。

## 误报（保留）

- `DirectBuf::as_uninit`（`crate/io/fkv_io_compio/src/lib.rs:348`）：
  是 `IoBufMut` trait 的实现方法，由 compio 的 `read_at` / `write_at` 通过
  trait 泛型调用；rust-analyzer 的 SCIP 引用图看不到这种单态化调用。
  删除会直接破坏 O_DIRECT 读路径，属于扫描器误报。

## 建议

1. 按表删除 9 个符号；删除后跑 `./sh/clippy.sh`（`-D warnings`）与
   `./test.sh`，确认没有导出/文档引用残留。
2. `fkv_manifest` 的 4 个 varint 包装删除前检查 `fkv_ledger` / fixture 工具是否
   通过 crate 路径引用（全仓 grep 已确认 0）。
3. 在 `sh/unused.js` 的说明里补一句“trait impl 方法可能误报”，避免后续
   把 `as_uninit` 这类实现当死代码删掉。

## 严重度

P3（纯清理，无行为风险；减少 API 面与误导性入口）

## 核验记录（2026-11-04）

`./sh/unused.js` 原始输出已保存比对；9 个符号逐一全仓 `grep -rn`（含 tests/tool）确认 0 引用；
`prepare` / `build_dictionary` / 写路径 `set_*` 调用点实读，确认删除不改变行为。
`as_uninit` 的 trait 调用路径按 `IoBufMut` 实现与 compio `read_at`/`write_at` 用法判定为误报。

## DONE（2026-11-04）

已按表删除 9 个零引用符号，`as_uninit` 判定为 trait impl 调用保留：

- `PooledFilePages::free_cnt`（`crate/core/fkv_mapping/src/lib.rs`）
- `prepare_standalone`、`DictCompression::clear_samples`（`crate/format/fkv_compress/src/lib.rs`）
- `varint64_size` / `varint32_size` / `read_varint64` / `read_varint32` 及对应 `fkv_codec` 导入（`crate/format/fkv_manifest/src/lib.rs`）
- `put_data_header` / `put_data_checksum`（`crate/format/fkv_page/src/data.rs`，同步移除未用的 `set_page_type` 导入）

保留项（审查结论）：

- `DirectBuf::as_uninit`：`IoBufMut` trait 实现，由 compio `read_at`/`write_at` 单态化调用，非死代码。
- `eng-82` 范围的 `shard_metrics` / `inner_mut` / `preload_tables` / `segment_file_size` / `file_page_offset_mask` / `list_remote` / `with_rsync_bin` 未触碰。
- `eng-78` 范围的 `tbl_mut` / `take_dirty_page_li` / `allocate_file_page` 未触碰。

验证：`cargo check -p fkv_mapping -p fkv_compress -p fkv_manifest -p fkv_page --all-targets --all-features` 通过；
上述 crate `cargo clippy ... -D warnings` 零警告；`cargo nextest run` 61 tests 全通过。
全仓 `cargo check` 期间 `fkv_shard` 处于其他 agent 的并发改动中（`tx`→`tx_hi`/`tx_lo` 迁移未完成），失败与本 issue 无关。

遗留：`sh/unused.js` 说明追加“trait impl 方法可能误报”一句未做，属于脚本文档，留待脚本负责人处理。

## 复审（2026-11-05 r6）

通过。9 个符号（PooledFilePages::free_cnt、prepare_standalone、clear_samples、varint32/64_size、read_varint32/64、put_data_header、put_data_checksum）在 fkv_mapping/fkv_compress/fkv_manifest/fkv_page 全仓 grep 零命中，删除属实。
