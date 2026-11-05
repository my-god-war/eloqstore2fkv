# gap-06 请求级 deadline / 队列背压「有 API 无配置面」（P2，本机可做）

## 来源

`sto-62b` 残余（我方复核记录于 `issue/reviewed/sto-62b-request-deadline-and-backpressure.md`）；
dragon 的剩余工作盘点。

## 证据（2026-11-05 实测）

已实现的部分（**不要重复做**）：

- `ShardSender::request_deadline / request_at / with_request_timeout / with_queue_timeout`
  （`fkv_shard/src/lib.rs:295-320` 一带），`Envelope.deadline` + `expired()` 在
  `process_envelope` / `run_read_batch` 两处执行路径生效；满队列 → `Busy`、过期 → `Timeout`
  由 `queue_wait` 取「入队 timeout 与请求 deadline 较早者」决定。
- `Runtime::with_request_timeout` / `with_queue_timeout`（`fkv_runtime/src/lib.rs:1978-1990`），
  并在 `Runtime::request` 路径套用（`:2092`），另有默认值说明（`:2435`）。
- 测试：`fkv_runtime/tests/deadline.rs`、`fkv_shard` 内 `enqueue_full_*` 单测。

**缺的部分**：

- `rg request_queue_timeout_us|request_timeout_us crate/foundation/fkv_config → 0 命中`：
  `KvOption` 里**没有**任何请求超时 / 队列背压字段。
- 因此 `crate/facade/fkv` 的 `start_runtime(root, &KvOption)`（`:35-37`，
  只是 `Runtime::start_with_option(RuntimeConfig::new(root), option)`）**不带默认 deadline**：
  宿主不显式链式调用 `with_request_timeout/with_queue_timeout` 就仍是「无限等待」。

## 问题

云模式下「一次 S3 重试窗口」量级的默认 deadline 没有落点：不配就没有保护，
而配置文件（INI 面，C++ 兼容）无法表达它 —— 这属于「有实现无入口」。

## 目标

1. `KvOption`（或 `KvOptions` 扩展面，与既有 `max_processing_time_us` 同一条链路）
   增加 `request_timeout_us` / `request_queue_timeout_us`（0 = 不启用 = 保持旧行为）。
2. `start_runtime` 用这两个值初始化 `Runtime`；默认值策略要写清：Local / Mem 默认不启用，
   **Cloud 默认覆盖一次 S3 重试窗口**（具体秒数需给出依据，可引用
   `fkv_object_s3` 的重试参数）。
3. `attach_shard` / `detach_shard` / `Shutdown` 不套用（现有注释已声明控制面例外，
   测试需固定这一点）。

## 验收

- 配置文件设 `request_queue_timeout_us=N` 后，满队列请求在 N 内返回 `Busy`
  （集成测试：小 `queue_size` + 饱和发送方，断言 `Busy` 与耗时上界）；
- 不设时行为与今天一致（现有用例全绿）；
- 云模式默认值有测试或明确文档依据（不能只写注释）。

## 阻塞 / 依赖

**无外部阻塞**：本机可做。
注意 `fkv_shard` / `fkv_runtime` / `fkv_config` 是并发热点，落地前按
`gap-08` 的规则确认 owner 与 mtime 静默。

## 核验记录

- 实测 grep（本次）：`KvOptions` 无超时字段；`Runtime::with_*` 无配置来源；
  `fkv_runtime:2092` 确实套用了 builder 值。

## 复审（2026-11-05）

原 index 把它归为“验证债务”不成立：`KvOption` 无超时字段，`start_runtime`
不链式配置，是典型功能缺口。直接开发。

## 复审归档（2026-11-05）

工作已落地，本轮复审逐项复核成立后归档到 `issue/reviewed/`。跳过 `done/`：这些条目的
实现在本轮复审之前就已提交，`done/` 只用于本轮亲自开发并附测试证据的条目。
`new/` 只留仍未落地的条目。

证据（复核命令与结果）：

- `crate/foundation/fkv_config/src/lib.rs`：`request_timeout_us` /
  `request_queue_timeout_us` 两个 `Ext` 键（默认 `0` = 未配置）+ `request_timeout()` /
  `queue_timeout()` / `resolve_timeout()` 解析器，Cloud 未配置时回退
  `DEFAULT_CLOUD_REQUEST_TIMEOUT_US`（30s，依据 `fkv_object_s3` 的重试窗口）。
- `crate/facade/fkv/src/lib.rs` 的 `start_runtime` 链式套用
  `Runtime::with_request_timeout` / `with_queue_timeout`。
- 测试：`fkv_config/tests/config.rs::request_timeout_keys_parse_and_resolve_by_mode`；
  `fkv/tests/runtime_entry.rs` 三条（INI→`Timeout`、饱和→`Busy` 带上界、不配置保持旧行为）。

无残余项。
