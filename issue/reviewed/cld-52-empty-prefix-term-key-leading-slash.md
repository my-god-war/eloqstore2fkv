# cld-52 P2：空 prefix 时 term 对象 key 多前导 `/`，与本 crate 其它 key 及 C++ 都不一致

来源：第 5 轮 runtime/cloud lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

`crate/engine/fkv_cloud/src/lib.rs:1757-1763/1775-1781`（`read_term` / `upsert_term`）：

```rust
let key = format!(
  "{}/{}",
  self.prefix.trim_end_matches('/'),
  current_term_file_name(branch, shard_id)
);
```

`split_cloud_store_path("bucket")` 返回 `("bucket", "")`（`fkv_shard/src/backend.rs:476-483`）
→ 空 prefix 时 key = `"/CURRENT_TERM_main_0"`。

同 crate 的表前缀却是空 prefix 无分隔——`fkv_cloud/src/lib.rs:476-483`：

```rust
fn tbl_prefix(&self, tbl: &TableIdent) -> String {
  let prefix = self.prefix.trim_matches('/');
  if prefix.is_empty() { tbl.to_string() } else { format!("{prefix}/{tbl}") }
}
```

C++ `ComposeKeyFromRemote`（`eloqstore/src/storage/object_store.cpp:483-507`）空组件
直接跳过不加 `/`，term 文件走同一组装（`:517/:570`）。

## 问题

- fkv 集群内自洽所以测试不炸，但同 bucket 内 data/manifest key（无前导斜杠）与
  term key（有前导斜杠）命名分裂。
- 与 C++ 集群互操作（attach fencing / term CAS 跨实现）时双方互看不见对方 term
  文件，fencing 静默失效——正是 COMPAT_BASELINE 关心的盘面/对象面兼容。

## 目标

term key 组装收敛到与 `tbl_prefix` 同一规则：空 prefix → 裸文件名，
非空 → `prefix/name`。建议抽 `fn root_object_key(&self, name: &str) -> String`
与 `tbl_prefix` 并列，一处定义。

## 验收

- 单测：`prefix=""` 时 `read_term`/`upsert_term` 用的 key == `CURRENT_TERM_main_0`
  （无斜杠）；`prefix="pfx/"`、`"pfx"` 归一为 `pfx/CURRENT_TERM_main_0`。
- `cargo nextest run -p fkv_cloud`。

## 依赖 / 重叠

- 无。sto-61c（本地 term 文件）是本地路径，不相关。

## 核验记录

- 本人实读：`fkv_cloud/src/lib.rs:470-495/1750-1790`、`backend.rs:476-483`。
- 子代理摘录（未复核）：C++ `object_store.cpp` 行号。

## DONE（2026-11-04）

按「收敛到与 `tbl_prefix` 同一规则」落地（`crate/engine/fkv_cloud/src/lib.rs`）：

- 新增 `CloudStore::root_object_key(name)`：空 prefix（`trim_matches('/')` 后）→ 裸名字，
  非空 → `prefix/name`；与 `tbl_prefix` 同一判定，对齐 C++ `ComposeKeyFromRemote`
  「空组件直接跳过、不加分隔符」。
- `read_term` / `upsert_term` 都改走 `root_object_key(&current_term_file_name(...))`，
  删掉原来 `format!("{}/{}", self.prefix.trim_end_matches('/'), ...)`——空 prefix 下
  不再产生 `/CURRENT_TERM_main_0`。

验证（`crate/engine/fkv_cloud/tests/cloud.rs`）：

- 新增 `term_key_has_no_leading_slash_with_empty_prefix`：三种 prefix
  （`""` / `"pfx"` / `"pfx/"`）下 `upsert_term` 后远端 key 分别为
  `CURRENT_TERM_main_0` / `pfx/CURRENT_TERM_main_0` / `pfx/CURRENT_TERM_main_0`，
  且**不存在任何前导斜杠的 key**，`read_term` 读回一致。
  修复前 `prefix=""` 这条会失败（key 为 `/CURRENT_TERM_main_0`）。
- `cargo nextest run -p fkv_cloud` 33 passed；
- `cargo clippy -p fkv_cloud --all-targets --no-deps -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

**通过。** 核实：`fkv_cloud/src/lib.rs:541` `root_object_key(name)`（空 prefix → 裸名，与 `tbl_prefix` 同判定），`read_term`/`upsert_term` 均走该组装（全仓 grep 无 `trim_end_matches('/'), ...` 旧拼法残留）。测试 `tests/cloud.rs:863` `term_key_has_no_leading_slash_with_empty_prefix`：三种 prefix 断言期望 key 存在（:880）、任何前导斜杠 key 不存在（:884）、读回一致（:888）——断言实质覆盖修复。
