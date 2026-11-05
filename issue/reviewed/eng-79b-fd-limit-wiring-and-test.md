# eng-79b fd_limit 配置接线 + FD 缓存压力测试

## 位置

- `crate/foundation/fkv_config/src/lib.rs:30,101,302`：`fd_limit` 已有字段 / 默认 10_000 / INI 解析。
- `crate/engine/fkv_runtime/src/lib.rs:215-247`：`ShardConfig` 组装处（参考
  `max_write_batch_pages` 的接线方式）。
- `crate/engine/fkv_shard/src/lib.rs:35-61,136-140`：`ShardConfig` 与 `CompioStore` 构造。
- `crate/facade/fkv_sdk/src/lib.rs:30-34`：`KvStore::new` 单 store 接线。
- `crate/io/fkv_io_compio/src/lib.rs:723-726`：`with_fd_cache_limit`。

## 方案

1. `fkv_config` 新增 `DEFAULT_FD_LIMIT: u32 = 10_000`，`KvOption::default` 复用。
2. `ShardConfig` 增加 `fd_limit: usize`（按 shard 均分后的值），`Shard::start` 调
   `CompioStore::with_fd_cache_limit(config.fd_limit)`。
3. `Runtime::start_impl` 计算 `shard_fd_limit = store_fd_limit / num_threads`，至少 1；
   `store_fd_limit == 0` 视为不限制（直接传 0）。无 options 时用 `DEFAULT_FD_LIMIT`。
4. `fkv_sdk::KvStore::new` 直接 `.with_fd_cache_limit(options.fd_limit as usize)`
   （单 store，不按线程均分）。
5. 补压力测试：打开 N > limit 个不同文件，断言 `fd_cache_len() <= limit`；
   重复命中缓存内路径时 `fd_open_num()` 不增长。

## 验收

- `cargo nextest run -p fkv_io_compio -p fkv_config -p fkv_shard -p fkv_runtime --no-fail-fast` 全绿。
- `cargo clippy -p fkv_io_compio -p fkv_config -p fkv_shard --no-deps --all-features --all-targets -- -D warnings` 零警告。
- 配置链路：`KvOption::fd_limit` -> `ShardConfig::fd_limit` -> `CompioStore` 上限可观测。

## DONE（2026-11-04）

实现要点：

- `fkv_config`：新增 `DEFAULT_FD_LIMIT = 10_000`，`KvOption::default` 复用。
- `fkv_shard`：`ShardConfig.fd_limit: usize`（0 = 不限制），
  `Shard::start` 转 `CompioStore::with_fd_cache_limit(config.fd_limit)`；
  并发落地的 `backend::StoreBackend::local` / `cloud` 同样透传 `fd_limit`。
- `fkv_runtime`：`shard_fd_limit = store_fd_limit / num_threads`，至少 1；
  `fd_limit == 0` 保留为不限制；无 options 时用 `DEFAULT_FD_LIMIT`。
- `fkv_sdk::KvStore::new`：单 store 直接 `with_fd_cache_limit(options.fd_limit)`。
- 测试：`crate/io/fkv_io_compio/tests/hardening.rs::fd_cache_evicts_and_reuses_under_limit`
  —— limit=4 写 12 个文件，断言 `fd_cache_len() <= 4`；重复写缓存内 4 个文件时
  `fd_open_num()` 不增长。

验证：

- `cargo nextest run -p fkv_io_compio -p fkv_config -p fkv_shard -p fkv_runtime --no-fail-fast`
  ：84 passed, 1 skipped。
- `cargo clippy -p fkv_io_compio -p fkv_config -p fkv_shard --no-deps --all-features
  --all-targets -- -D warnings`：0 warning。

遗留：`fd_limit` 的 INI 解析沿用既有 `Section::Run`；未新增校验（0 表示不限制，
与 `with_fd_cache_limit` 既有语义一致）。

## 复审（2026-11-05 r6）

通过。

- `fkv_config/src/lib.rs:45 DEFAULT_FD_LIMIT = 10_000`、`:177 fd_limit` 默认引用
  常量；`fkv_shard/src/lib.rs:131 ShardConfig.fd_limit`、`backend.rs:28
  .with_fd_cache_limit(config.fd_limit)`；`fkv_runtime/src/lib.rs:1088-1091
  shard_fd_limit = store_fd_limit / num_threads`（0 保留不限制、至少 1）、
  `:1139` 写入 ShardConfig；`fkv_sdk/src/lib.rs:42
  .with_fd_cache_limit(options.fd_limit as usize)`。
- 测试 `hardening.rs:259 fd_cache_evicts_and_reuses_under_limit` 断言
  `fd_cache_len() <= 4` 且重复写缓存内文件 `fd_open_num()` 不增长（`:274-279`）。
