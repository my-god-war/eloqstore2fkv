# io-29b S3 backend list_page 用 list_with_offset 下推 token

## 位置
- `crate/io/fkv_object_s3/src/lib.rs`：actor 的 `CommandKind::ListPage` 分支
  （约 378-405 行）。
- 测试：`crate/io/fkv_object_s3/tests/s3_mock.rs`。

## 现状
`list_page` 每次都从 prefix 起点 `store.list(...)` 拉流，再在本地
`meta.key <= token` 跳过。翻到第 k 页时要重扫前 k-1 页的对象，
全量遍历退化为 O(n²)；S3 服务端也无法利用 `start-after`。

## 方案
token 非空时改用 `store.list_with_offset(prefix, offset)`：
`object_store` 的语义就是「严格大于 offset」，S3 后端会下推为
`start-after`，且与我们 `next_token = 上一页最后 key` 完全一致。
保留本地 `<= token` 跳过作为防御（未下推 offset 的实现仍正确）。

## 验收
- `cargo nextest run -p fkv_object_s3` 通过。
- mock 测试断言第二页请求行带 `start-after=<token>`。
- `cargo clippy -p fkv_object_s3 --all-features --all-targets -- -D warnings` 零警告。

## DONE（2026-11-04）

- `crate/io/fkv_object_s3/src/lib.rs` 的 `CommandKind::ListPage` 分支：
  token 非空改走 `store.list_with_offset(prefix, token)`，S3 下推为
  `start-after`，不再每页从 prefix 起点重扫；保留本地 `<= token` 跳过作防御。
- 测试：`crate/io/fkv_object_s3/tests/s3_mock.rs::list_page_pushes_token_as_start_after`
  断言第二页请求行含 `start-after=obj1`，且越界 token 返回空页。
- 验证：`cargo nextest run -p fkv_object_s3` 17 passed；
  `cargo clippy -p fkv_object_s3 --all-features --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。

- `fkv_object_s3/src/lib.rs:394-398`：token 非空改走
  `store.list_with_offset(Some(&prefix_path), &OsPath::from(token.as_str()))`，
  注释写明下推 start-after 消除整桶 O(n²) 重扫；保留本地 `<= token` 跳过防御
  （`:400` 一带 ListPage 分支内）。
- 测试 `tests/s3_mock.rs:467 list_page_pushes_token_as_start_after` 断言第二页
  请求行含 `start-after=obj1`（`:495-496`）。
