# io-29a ObjectStore::list / list_page 契约文档化

## 位置
- `crate/io/fkv_object/src/lib.rs`：`ObjectStore::list`（约 82 行）、
  `ObjectStore::list_page` 默认实现（约 84-110 行）、`ListPage`（约 20 行）。
- 测试：`crate/io/fkv_object/tests/object.rs`。

## 现状
`list_page` 与 `ListPage { meta_li, next_token }` 已落地，S3 与 cloud
已接入；但 trait 未写明契约：

- key 排序（默认实现与 S3 均为字节序升序，backends 可随意实现）；
- `token` 语义（是「上一页最后 key」，比较为严格大于）；
- `max_keys == 0` 的含义；
- prefix 是递归前缀匹配，无目录/分隔符语义。

另：`list()` 未标注大 bucket 慎用。

## 方案
1. 给 `list` 补文档：自动翻页便捷方法，会物化整个 prefix，大 bucket 慎用，
   新代码优先 `list_page`。
2. 给 `list_page` 补完整契约文档（排序、token、max_keys、递归）。
3. 明确不加 `delimiter`：当前云 key 布局 `<prefix>/<table>.<partition>/<kind>_...`
   是平铺单层，递归与非递归等价；C++ `recurse_` 在迁移后的布局下无差异。
   若将来引入层级 key 再评估。
4. 测试补边界：token 超出全部 key、`max_keys == 1`、`max_keys == 0` 不限。

## 验收
- `cargo nextest run -p fkv_object` 通过。
- `cargo clippy -p fkv_object --all-features --all-targets -- -D warnings` 零警告。
- doc 注释为中文，无 `allow`。

## DONE（2026-11-04）

- `crate/io/fkv_object/src/lib.rs`：`list` 标注"物化整个 prefix，大 bucket 慎用"；
  `list_page` 写明排序（字节序升序）、token（严格大于，取上一页最后 key）、
  `max_keys == 0` 不限、prefix 递归、`next_token` 结束条件；默认实现逻辑未变。
- 明确不加 `delimiter`：当前云 key 布局平铺单层，递归/非递归等价。
- 测试：`crate/io/fkv_object/tests/object.rs::list_page_token_and_bounds`
  覆盖 token 越过末尾、`max_keys == 0`、恰好取满一页。
- 验证：`cargo nextest run -p fkv_object` 6 passed；
  `cargo clippy -p fkv_object --all-features --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。

- `fkv_object/src/lib.rs:88-91` `list` 标注「物化整个 prefix，大 bucket 慎用，
  新代码优先 list_page」；`:96-105` `list_page` 写明字节序升序（与 S3
  ListObjectsV2 一致）、token 严格大于、`max_keys == 0` 不限、prefix 递归、
  `next_token` 结束条件；默认实现 `:112-127`。
- 测试 `tests/object.rs:141 list_page_token_and_bounds` 覆盖 token 越过末尾、
  `max_keys == 0`、恰好取满一页。
- 不加 `delimiter` 的决策（平铺单层布局递归/非递归等价）与现状一致。
