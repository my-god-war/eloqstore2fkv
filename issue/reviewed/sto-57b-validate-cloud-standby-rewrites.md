# sto-57b `validate()` 补齐 cloud/standby 派生与 WARNING

## 位置
- `crate/foundation/fkv_config/src/lib.rs::KvOption::validate`
- C++：`src/eloq_store.cpp:121-360`（`ValidateOption`）

## 现状
只有基础校验与 cloud 清 standby。缺：
- gcs/google/google-cloud provider 且 `cloud_endpoint` 空 → `https://storage.googleapis.com`；
- cloud 模式 `max_cloud_concurrency==0` / `cloud_request_threads==0` 未拒绝；
- cloud 模式 `max_write_concurrency==0` → `max_cloud_concurrency`（WARNING）；
- cloud 模式 `local_space_limit==0` → `TB`（WARNING）；
- cloud / `enable_local_standby` 自动开启 `data_append_mode`（WARNING）。

## 方案
按 C++ `ValidateOption` 顺序：endpoint 默认 → 基础校验 → cloud 清 standby → standby 校验（含 append 自动开启）
→ `standby_master_addr` 校验（本轮不动，属 12b/eng-39c）→ cloud 块。
provider 比较用 `eq_ignore_ascii_case`，不分配小写串。

## 验收
- `tests/config.rs::validate_cloud_rewrites`、`validate_standby_enables_append`。
- 不改 `standby_master_addr` 文法/路径校验（避免与 runtime 测试、12b 冲突）。

## 不做
- `fd_limit` 按 `local_space_limit/data_file_size` 夹取：依赖 cloud cache 预算语义（runtime 里 `cache_budget: 0`），归 cloud cache 议题。

## DONE（2026-11-04）

- `validate`：GCS 系 endpoint 默认（`GCS_ENDPOINT` 常量，`eq_ignore_ascii_case` 零分配）；cloud 块补
  `max_cloud_concurrency`/`cloud_request_threads` 非零校验、`max_write_concurrency` 回退、
  `local_space_limit=0` → `TB`、`data_append_mode` 自动开启；`enable_local_standby` 分支自动开启追加写。
- 测试：`validate_cloud_rewrites`、`validate_standby_enables_append`。
- 三 crate 回归：`cargo nextest run -p fkv_config -p fkv_runtime -p fkv_sdk --no-fail-fast` → 44 passed / 1 skipped。
- 遗留：`fd_limit` 夹取依赖 cloud cache 预算（`cache_budget: 0`），归 cloud cache 议题；`standby_master_addr` 文法校验归 12b/eng-39c。

## 复审（2026-11-05 r6b）

通过。

- GCS endpoint 默认：`crate/foundation/fkv_config/src/lib.rs:60`（`GCS_ENDPOINT` 常量）、`:316`（validate 回填）。
- cloud 块：`:415`（`max_cloud_concurrency`/`cloud_request_threads` 非零拒绝）、`:419`（`max_write_concurrency` 回退）、`:425`（`local_space_limit=0` → TB）；standby/cloud 自动开启追加写：`:404-411`、`:429-430`。
- 测试：`crate/foundation/fkv_config/tests/config.rs:306 validate_cloud_rewrites`、`:333 validate_standby_enables_append` 在盘。
