# r8b-02 P3：TTL 读过滤默认开启是跨实现可见的有意偏离（C++ Read/Scan/Floor 不过滤），仅代码注释与测试名登记，06-gaps / COMPAT_BASELINE / doc 均无条目

## 现象（亲读）

Rust 侧读路径默认过滤已过期项：

- `crate/engine/fkv_store/src/lib.rs:342`：`ttl_read_filter: true`（结构体默认）。
- `:496-502`（`with_ttl_read_filter` 文档）自认偏离：「C++ eloqstore 的 Read /
  Scan / Floor 不过滤，过期项由 CleanExpired 物理删除后才消失；需要逐项对齐
  oracle 行为时传 false」。
- 生效点：点读 `:805`（`read_ctx_at` → `ttl_filter_now_ms`）、扫描 `:1084` /
  `:1186`（scan_entry_page / scan_entry_opt 下推）、floor `:3205-3219`
  （`read_ctx_at(tbl, &snapshot, now_ms())`）。
- **生产接线无一处覆盖默认值**：`grep with_ttl_read_filter` 在
  `fkv_shard/src`、`fkv_config`、`fkv_runtime`、`facade/`（sdk/cli）全部零命中
  ——shard 每分区 Store、SDK、Runtime 均按默认 `true` 运行。
- 登记缺失：`grep "ttl_read_filter|读路径过滤|过期过滤" doc/` 零命中；
  `doc/architecture/06-gaps.md` 与 `05-persistence-and-recovery.md` 无 TTL 读语义
  条目；`COMPAT_BASELINE` 无；`issue/` 无承接条目。唯一记录 = 上述代码注释 +
  测试名 `fkv_store/tests/ttl.rs:179`
  `ttl_read_filter_can_match_cpp_lazy_cleanup`（用 `false` 模拟 C++ 语义）。

后果：过期未清理窗口内，同一盘面两侧读结果不同——Rust `get`/`scan`/`floor`
返回 `NotFound` / 跳过该条，C++ 返回值并附 `expire_ts` 由应用层裁决。引擎级
V2/V4（含 TTL 语料）一旦系统化，该差异会直接进入验收结果，而基线的排除清单
对此沉默。

## C++ 对照

- `src/tasks/read_task.cpp:60-97`：`ReadTask::Read` / `LocateAndProcess` 找到
  entry 后直接返回 value + `timestamp` + `expire_ts`（:80/:97），**无任何
  `expire_ts <= now` 判断**。
- `src/sdk_runtime/worker.cpp` / `common.cpp`：grep `expire` 零命中——SDK 层同样
  不过滤，过期可见性交给应用。
- `src/tasks/batch_write_task.cpp:2016` `CleanExpiredKeys`：过期项只在 TTL 树
  扫描 + 主树删除后消失（写路径 `TriggerTTL` 异步驱动，write_task.cpp:943）。
- fkv 的写侧（`prepare_write_batch` 过期 upsert 折叠为 delete、
  `clean_expired` 主树复核）已对齐；偏离仅在读可见性默认值这一个面。

## 目标

在「有意差异」清单中登记（二选一）：

1. `doc/architecture/06-gaps.md` 增一行（建议挂「写路径与内存」或单列 TTL 段）：
   默认读路径过滤过期项（`ttl_read_filter=true`），C++ 依赖 CleanExpired 惰性
   清理、Read/Scan/Floor 不过滤；对齐开关与重开条件（oracle 逐项对齐时传
   false，见 `ttl.rs:179` 测试）。
2. 若裁定纳入 `COMPAT_BASELINE` 排除清单（引擎级 V2/V4 会踩到），按 r8b-01
   同款流程改基线 + 重跑 `gen_fixtures.sh` 刷 meta。

不要求改任何实现代码：偏离本身是合理的产品选择（过期 key 应及时不可见），
缺的只是登记。

## 验收

- `grep -rn "ttl_read_filter" doc/` 至少一处命中（06-gaps 或基线），
  且该处表述与 `fkv_store/src/lib.rs:342/:496-502` 一致；
- 若改 COMPAT_BASELINE：`fixtures/**/meta.txt` 的 `baseline=` 与基线 flatten
  逐字节一致；
- `fkv_store/tests/ttl.rs` 既有用例不回归。

## 核验记录

- 亲读 `fkv_store/src/lib.rs` 读路径四处生效点 + 默认值 + 注释；
  `grep -rn "with_ttl_read_filter|ttl_read_filter" crate/` 逐一核对消费面
  （仅 fkv_store 自身与测试，生产链路零覆盖 → 默认值即生产行为）。
- 亲读 C++ `read_task.cpp:60-97`、`sdk_runtime`（worker/common `expire` 零命中）、
  `batch_write_task.cpp CleanExpiredKeys`（:2016 起）、`write_task.cpp TriggerTTL`
  （:943 起）。
- 查重：eng-64（点读 TTL 下推）复审确认「语义不变：过滤开关仍由
  ttl_read_filter 决定」，登记欠账不在其范围；perf-84 / sto-66 同理；
  r8c-01/r8c-02/r8d-01、r8b-01 均不涉本条。
- mtime 备注：`fkv_store/src/lib.rs` mtime 05:19（r8d-01 记录的并行改动窗口），
  本条锚点（:342/:496/:805/:1084/:1186/:3205 与 ttl.rs:179）均按内容亲读命中，
  非 md5 级断言；`cargo nextest run -p fkv_store -p fkv_task_write
  -p fkv_task_compact` 193/193 通过（05:5x 本轮实测）。


## DONE（2026-11-05，第二轮复审修复）

DONE：`COMPAT_BASELINE` 登记 TTL 读过滤默认开启为有意差异，并给出 `with_ttl_read_filter(false)` 对齐入口。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`COMPAT_BASELINE:61-63` 已登记「TTL 读过滤：Rust 读路径默认过滤 `expire_ts <= now`（`ttl_read_filter=true`），C++ `Read`/`Scan`/`Floor` 返回过期值由应用层裁决。需要逐项对齐 oracle 时调用 `Store::with_ttl_read_filter(false)`」，与 `fkv_store/src/lib.rs:342` 默认值及 `:496-502` 注释一致。
- 验收：`grep -rn ttl_read_filter doc/` 命中 `doc/progress.md:6`（基线镜像）；`fixtures/**/meta.txt` baseline 同步含该条。
- 判定：完善。
