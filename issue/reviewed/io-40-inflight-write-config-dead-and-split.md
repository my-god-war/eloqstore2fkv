# io-40 P2：`max_inflight_write` 配置死键 + 常量三处分裂（16 / 8 / 配置 32768 vs C++ 4096）

来源：第 5 轮 io/compio lane 侦查，协调者亲验。

## 证据（2026-11-05 核验）

- `crate/io/fkv_io_compio/src/lib.rs:55-62`：

```rust
const WRITE_INFLIGHT_LIMIT: usize = 8;
/// IO 层在飞写组上限（对齐 C++ `max_inflight_write`）：…
const MAX_INFLIGHT_WRITE: usize = 16;
/// 单组合并写页数默认上限（对齐 C++ `KvOption::max_write_batch_pages`）。
const DEFAULT_MAX_WRITE_BATCH_PAGES: usize = 32;
```

- 消费点：`MAX_INFLIGHT_WRITE` → `:789 write_gate`、`:1303/:1316` 池上界；
  `WRITE_INFLIGHT_LIMIT` → `:1971 buffer_unordered`。两个「在飞上限」并存。
- `fkv_config` 的 `max_inflight_write: u32 = 32 << 10`（`fkv_config/src/lib.rs:119`）
  解析、校验后**全仓零消费**（grep 仅 compio 注释提及）。
- C++ 默认值（`eloqstore/src/kv_options.cpp:152-160`）：`max_inflight_write=4096`、
  `max_write_batch_pages=64`，均可 ini 覆盖。

## 问题

注释声称「对齐 C++」但数值/可配置性都不对齐：写吞吐被 16 组 × 32 页 × 4 KiB
= 2 MiB 在飞封顶（C++ 默认 4096 组）；配置键存在却接不到实现，用户调了没用。

## 目标

1. `with_max_inflight_write(usize)` 贯通 config → CompioStore（对齐
   `with_fd_cache_limit` 先例），默认值对齐 C++ 4096。
2. `WRITE_INFLIGHT_LIMIT`（8）与 `MAX_INFLIGHT_WRITE`（16）的关系写清或合并。
3. `DEFAULT_MAX_WRITE_BATCH_PAGES` 默认 32 vs C++ 64：查 io-36 done 时的决策，
   无决策记录则对齐 64 并在 COMPAT/迁移文档注明。

## 验收

- config 键改小后写池上界随之变化（单测观察 `write_gate` 行为）；
  默认值与 C++ 对齐或有书面偏离记录。
- `cargo nextest run -p fkv_config -p fkv_io_compio`。

## 依赖 / 重叠

- io-27（done）补了 bound 本身；eng-72（done）是定义去重；本条是贯通与数值。

## 核验记录

- 本人实读：`fkv_io_compio/src/lib.rs:55-66/789/1303/1971`、
  `kv_options.cpp:152-160`、全仓 grep `max_inflight_write`。

## 审查结论（2026-11-04，协调者）

**采纳**（实读核对，断言成立，留 todo）：

- `KvOption.max_inflight_write`（`fkv_config/src/lib.rs:124`，默认 `32 << 10`）全仓
  只有 `validate()` 里的非零检查（`:252`）**零消费** → 配置静默失效；
- 真正的上界在 `fkv_io_compio/src/lib.rs` 里硬编码（issue 引用的
  `WRITE_INFLIGHT_LIMIT = 8` / `MAX_INFLIGHT_WRITE = 16`），与 C++ 4096 也不同量级；
- 修法照 `with_fd_cache_limit` 先例加 `with_max_inflight_write(usize)` 贯通
  config → CompioStore，并写清两个常量的关系（或合并）。
- 依赖 `fkv_io_compio`（另一会话热点文件），本轮不动代码，交给其 owner。

## DONE（2026-11-05）

### 结论

核心断言成立：`KvOption::max_inflight_write` 解析后零消费，IO 层另有
`WRITE_INFLIGHT_LIMIT = 8` / `MAX_INFLIGHT_WRITE = 16` 两个硬编码在飞上限。

默认值断言需纠正：C++ 库默认值是 `eloqstore/include/kv_options.h:79,83` 的
`max_inflight_write = 32 << 10`、`max_write_batch_pages = 32`。
`src/kv_options.cpp:152-160` 的 `4096` / `64` 位于 `reader.HasValue(...)` 分支内，
是「INI 键存在但取不到值」时的解析兜底；`db_stress` gflag 的 `4096` / `64`
是压测入口默认值。二者都不是 `KvOption` 默认构造结果，因此 Rust 默认值保持
`32768` / `32`，与 C++ 头文件对齐，未按 issue 建议改成 4096/64。
书面口径见 `doc/migration.md` §8.3。

### 改动

- `crate/foundation/fkv_config/src/lib.rs`
  - 新增 `DEFAULT_MAX_INFLIGHT_WRITE = 32 << 10`，`max_inflight_write` 字段默认引用该常量；
  - `DEFAULT_MAX_WRITE_BATCH_PAGES` 注释登记 32/64/4096 三个数的来源与口径。
- `crate/io/fkv_io_compio/src/lib.rs`
  - 删除 `WRITE_INFLIGHT_LIMIT = 8` 与 `MAX_INFLIGHT_WRITE = 16`；
  - 新增 `CompioStore::max_inflight_write`（默认 32768）、
    `with_max_inflight_write(usize)`、`max_inflight_write()`；
  - `WriteGate` 容量与 `write_page_li` 的 `buffer_unordered` 窗口共用该值，
    全局只剩一个在飞写上限；0 归一为 1；
  - 注册写槽位缓存另设 `FIXED_WRITE_SLOT_LIMIT = 16`（仅 Linux），实际容量
    `min(max_inflight_write, 16)`；它不是第二处在飞上限，超限回落普通写池。
- `crate/engine/fkv_shard/src/lib.rs` / `backend.rs`
  - `ShardConfig.max_inflight_write`，Local / Cloud 两条装配路径均透传到
    `CompioStore::with_max_inflight_write`。
- `crate/engine/fkv_runtime/src/lib.rs`
  - 新增 `write_limits(options)` 作为写 IO 两个上限的唯一投影；
  - `ShardConfig`、`CloudStoreConfig`、runtime 侧 CloudStore 全部透传。
- `crate/facade/fkv_sdk/src/lib.rs`
  - `KvStore::new` 直连 CompioStore 的路径也透传 `max_inflight_write`，
    避免 facade 侧继续存在死配置。
- `doc/migration.md`
  - 新增 §8.3，记录默认值口径与槽位缓存上限的语义。

### 测试

- `cargo test -p fkv_config -p fkv_io_compio`：全部通过。
  - 新增 `write_limits_default_and_ini_override`：默认 32768/32、INI 可覆盖、validate 拒绝 0；
  - 新增 `inflight_write_limit_is_configurable`：默认 32768，调到 1 后 96 页全部落盘，
    注册槽位复用不超过 1；
  - 新增 `write_gate_blocks_at_limit_and_wakes_on_release` /
    `write_gate_limit_is_raised_before_acquire`：闸门满额挂起、归还唤醒、装配期容量可改。
- `cargo test -p fkv_shard --test backend --test write_gate --test lifecycle`：
  3/3、3/3、3/3 通过；新增 `local_backend_carries_inflight_write_limit`
  （7 透传、0 归一为 1）。
- `cargo test -p fkv_runtime --lib write_limits_project_defaults_and_overrides`：通过；
  `cargo test -p fkv_runtime --test options`：3/3 通过。
- `cargo test -p fkv_sdk`：7 passed。
- `cargo +nightly clippy -p fkv_config -p fkv_io_compio -p fkv_shard -p fkv_runtime -p fkv_sdk --tests --all-targets -- -D warnings -A deprecated -A clippy::incompatible_msrv -W clippy::absolute_paths`：通过（两个 `-A` 仅用于屏蔽并发工作区 `fkv_metrics` 正在改 `fetch_update → try_update` 的告警）。
- `cargo +nightly fmt -p fkv_config -p fkv_io_compio -p fkv_shard -p fkv_runtime -p fkv_sdk -- --check`：通过。

### 默认值对齐表

| 配置 | Rust | C++ `include/kv_options.h` | C++ 非默认值 |
|---|---|---|---|
| `max_inflight_write` | 32768 | 32768 | `kv_options.cpp` 解析兜底 4096 / `db_stress` 4096 |
| `max_write_batch_pages` | 32 | 32 | `kv_options.cpp` 解析兜底 64 / `db_stress` 64 |

无偏离，未修改默认值。

### 风险 / 未做

- `fkv_runtime --lib` 全量跑时既有 `prewarm_tests::prewarm_uses_attach_identity_term`
  失败（PrewarmReport 计数 1 vs 0），与本条无关；本次只跑写上限相关单测与
  `--test options`。
- 注册写槽位缓存上限固定 16（Linux），配置调大时不会把 128 KiB 槽位池放大到
  `max_inflight_write` 量级；在飞名额仍完整跟随配置。

## 复审（2026-11-05 r6）

通过。

- `fkv_config/src/lib.rs:37 DEFAULT_MAX_INFLIGHT_WRITE = 32 << 10`、`:184`
  字段默认引用常量；`fkv_io_compio/src/lib.rs` 原 `WRITE_INFLIGHT_LIMIT=8` /
  `MAX_INFLIGHT_WRITE=16` 已删（全文无此二常量），`DEFAULT_MAX_INFLIGHT_WRITE`
  （`:60`）+ `with_max_inflight_write`（`:864-868`）+ `max_inflight_write()`
  （`:871-873`）；WriteGate 容量与 `buffer_unordered` 窗口共用该值
  （`:2099-2103`）；`FIXED_WRITE_SLOT_LIMIT=16` 为槽位缓存上限、容量取
  `min(max_inflight_write, 16)`（`:61-67`、`:1350`）。
- `fkv_runtime/src/lib.rs:114 write_limits` 唯一投影（`:1099/1136`）、
  ShardConfig 透传（`:743/756/790`）。
- 测试：`write_pool.rs:64 inflight_write_limit_is_configurable`（默认 32768、
  调 1 后 96 页全落盘且槽位 ≤1）；`lib.rs:2669/2687` WriteGate 满额挂起/归还
  唤醒/装配期改容量两个单测。
- 默认值对齐表（32768/32 对齐 C++ 头文件）与 doc/migration.md §8.3 口径一致。
