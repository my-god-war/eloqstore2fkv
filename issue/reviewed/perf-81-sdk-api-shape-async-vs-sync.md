
## 复审（2026-11-05 r6）

通过（本条落点在 fkv_sdk，非本 lane crate，抽验关键符号）。

- `fkv_sdk/src/lib.rs:84 put_at`、`:103 put_kv_at`、`:124 exists`（KvApi
  `exists` trait `:263/:309`）、`:141 delete_at`。
- 测试 `tests/sdk.rs:144 sdk_exists_matches_get_and_covers_large_value`、
  `:175 sdk_explicit_timestamp_put_and_delete`。
- 拒绝 sync 兼容层/Option 旋钮/独立 put_batch 的三条理由（block_on 禁忌、
  配置重复、能力子集）与仓库现状一致。
