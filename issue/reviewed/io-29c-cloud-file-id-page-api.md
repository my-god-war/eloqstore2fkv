# io-29c CloudStore 暴露分页 file id 列举

## 位置
- `crate/engine/fkv_cloud/src/lib.rs`：
  `list_manifest_file_name_li`（约 939 行）、`remote_file_id_li_of`（约 989 行）。

## 现状
两处已经是 `list_page(..., 1000)` 循环，但分页逻辑内嵌在「收集全量
`Vec`」的函数里，prewarm 只能拿到全量列表，无法边列举边下载。

## 方案
1. 抽出 `FileIdPage { file_id_li, next_token }` 与
   `pub async fn remote_data_file_page` / `remote_segment_file_page`
   （入参 token + max_keys），把名字过滤逻辑收进分页函数。
2. `remote_file_id_li_of` 改为循环调用分页函数（行为不变）。
3. 1000 提取为常量 `CLOUD_LIST_PAGE_KEYS`，供 prewarm 复用，消除魔法数字。

## 验收
- `cargo nextest run -p fkv_cloud` 通过。
- `cargo clippy -p fkv_cloud --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

- `crate/engine/fkv_cloud/src/lib.rs` 新增 `CLOUD_LIST_PAGE_KEYS = 1000`、
  `FileIdPage { file_id_li, next_token }`、`remote_file_id_page` 及
  `remote_data_file_page` / `remote_segment_file_page` 包装；
  名字过滤收进分页函数，`remote_file_id_li_of` 改为逐页循环，
  `list_manifest_file_name_li` 的 1000 换常量。
- 测试：`crate/engine/fkv_cloud/tests/cloud.rs::remote_file_id_page_paginates`
  （5 个 id、每页 2 条，断言不重不漏）。
- 验证：`cargo nextest run -p fkv_cloud` 23 passed；
  `cargo clippy -p fkv_cloud --all-features --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（符号名有演进，功能等价）。

- `fkv_cloud/src/lib.rs:325 CLOUD_LIST_PAGE_KEYS = 1000`；分页 API 演进为
  `remote_file_entry_page` + `remote_data_file_page`（`:2039`）/
  `remote_segment_file_page`（`:2050`），返回 `FilePage { entry_li:
  Vec<RemoteFileEntry>, next_token }`（`RemoteFileEntry:331`、`FilePage:339`），
  取代 md 中的 `FileIdPage`/`remote_file_id_page` 命名；名字过滤（kind 前缀 +
  branch/term 匹配）收进分页函数（`:2018-2032`）。
- `list_manifest_file_name_li` 分页循环消费常量（`:1852`、`:1951`）。
- 测试 `tests/cloud.rs:567 remote_file_id_page_paginates`：5 个对象每页 2 条，
  断言页大小上限、`entry.size`、跨页无重复无遗漏（`:584-601`）。
