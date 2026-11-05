# C++ → Rust 迁移方法与账本

## 1. 目标与硬约束

目标不是“把 C++ 语法翻译成 Rust”，而是得到一个**盘面格式兼容、行为可验证、性能可比较**的存储引擎。

硬约束：

1. **格式优先**：先锁 page / manifest / segment / term 的字节布局，再写引擎。
2. **双向可读**：C++ 写 → Rust 读；Rust 写 → C++ 读；不满足就不算兼容。
3. **证据驱动**：没有 fixture、差分或测试证据的 unit，不能标 `verified`。
4. **基线冻结**：`COMPAT_BASELINE` 固定 eloqstore commit；升级基线必须重扫、重验。
5. **master 常绿**：每个 crate 迁移完即可编译、测试，不做长期分支。

## 2. 当前状态（2026-11-05）

| 项 | 状态 |
|---|---|
| C++ 扫描单元 | 1848 个 unit，已写入 `.data/motedb.mote` |
| 账本状态 | `done = 1305`（`doc/adopt.tsv` 声明对齐）、`todo = 312`、`ignored = 231`（范围外裁定，见 `doc/ledger-classification.tsv` 与 `COMPAT_BASELINE`）、`<unmapped> = 0`、`verified = 0` |
| 审查看板 | `issue/{new,todo,done,reviewed,reject}` 实时统计，随 `report` 汇总进 `doc/progress.md`；本文不冻结计数 |
| Rust 测试 | 不冻结通过数；以 `./test.sh` / CI 输出为准。本轮文档对账复跑 `fkv_shard` 并发用例 12/12、27c 快照用例 5/5 + GC 保留 3/3 |
| 编译门禁 | fmt / clippy / workspace check 以 CI 为准 |
| 兼容基线口径 | （`COMPAT_BASELINE` 当前文本）`L1-partial`：Rust 仅 append 分配、验收默认无压缩；格式层 5 类 16 case 双向交叉读已落地，引擎级 V2/V4 未系统化。范围与排除项见 [`COMPAT_BASELINE`](../COMPAT_BASELINE) |
| C++ oracle | `cpp/oracle` 已实现格式层 5 类 16 case；`fixtures/cpp` = 16 个真实 oracle 字节 case + 6 个 `mapping`/`store` 占位（0 字节、`status=skipped`），`fixtures/rust` = 16 个反向 case。39 个 `_cpp` crate 仍是单行占位，仅差分验证用途 |
| 最新差距清单 | [`issue/new/00-index.md`](../issue/new/00-index.md) |

`done` 的含义：该 crate 对应的 C++ unit 已有 Rust 实现与通过的测试
（证据 = adopt 打的测试目录引用）。
`verified` 仍要求 C++ oracle 差分证据，按第 5 节逐 unit 流转；
里程碑叙事见 [`todo.md`](../todo.md) 的 D1–D70。

## 3. 迁移流程

```text
sync
  ├─ 更新 COMPAT_BASELINE
  ├─ tree-sitter scan C++ → unit
  └─ rule 映射 path → crate / layer
        ↓
crate 迁移完成 + 测试通过 → doc/adopt.tsv 加一行 → adopt（批量 done + 测试证据）
        ↓
next / list / claim（单 unit 精细操作）
        ↓
开发 + 测试 + fixture / 差分
        ↓
evidence add
        ↓
status done / verified
        ↓
report / export / changed（report 同时汇总 issue/ 看板计数）
```

### 3.1 日常命令

```bash
cd fkv

# 更新基线、重扫、重新映射
cargo run -p fkv_ledger -- sync

# 按可提交清单批量归属 / 裁定（map 312 + ignore 231，可从空账本重放）
cargo run -p fkv_ledger -- classify --file doc/ledger-classification.tsv --plan
cargo run -p fkv_ledger -- classify --file doc/ledger-classification.tsv
cargo run -p fkv_ledger -- report --out doc/progress.md
bash scripts/check_ledger_progress.sh

# 任务队列
cargo run -p fkv_ledger -- next --limit 20 --crate fkv_page
cargo run -p fkv_ledger -- list --status todo --crate fkv_page
cargo run -p fkv_ledger -- claim "<unit-id>" --owner z

# 状态
cargo run -p fkv_ledger -- status "<unit-id>" in_progress
cargo run -p fkv_ledger -- ignore "<unit-id>" --reason "inline helper"

# 证据（verified 必须有 pass evidence）
cargo run -p fkv_ledger -- evidence add "<unit-id>" test \
  --path crate/format/fkv_page/tests/page.rs --result pass
cargo run -p fkv_ledger -- evidence list "<unit-id>"

# 依赖 / 统计 / 报告
cargo run -p fkv_ledger -- edges "<unit-id>"
cargo run -p fkv_ledger -- stats
cargo run -p fkv_ledger -- changed --check
cargo run -p fkv_ledger -- report --out doc/progress.md
cargo run -p fkv_ledger -- export --out /tmp/fkv-ledger.json
```

### 3.2 状态规则

- `verified` 必须有至少一条 `result = pass` 的 evidence，否则 CLI 拒绝。
- `verified` 会记录当前 `source_hash` 到 `verification` 表。
- `scan` 更新 `source_hash` 后，`changed` 会列出 verified 但源码已变化的 unit。
- `changed --check` 可用于 CI / 提交前门禁。
- `ignored` 必须带 reason，reason 写入 `note`。
- `done` 表示代码已合入但证据未齐；`verified` 才表示通过验证。
- 当前 evidence 的 `kind` 只是自由字符串；建议统一为：
  `unit` / `round-trip` / `diff` / `cross-read` / `crash` / `bench`。

## 4. 账本数据模型

数据库：motedb，目录 `.data/motedb.mote`（gitignore）。

主要表（实际表名为单数）：

```sql
meta(k TEXT PRIMARY KEY, v TEXT)
unit(id TEXT PRIMARY KEY, path, symbol, kind, layer, crate, module,
     start_line, end_line, priority, risk, status, owner, compat,
     source_hash, created_at, updated_at)
edge(id TEXT PRIMARY KEY, from_id, to_id, kind)
include_edge(id TEXT PRIMARY KEY, path, inc_path)
evidence(id TEXT PRIMARY KEY, unit_id, kind, path, result, commit_id, created_at)
claim(id TEXT PRIMARY KEY, unit_id, owner, status, branch, pr,
      claimed_at, released_at)
run(id TEXT PRIMARY KEY, unit_id, suite, impl, result, metrics,
    commit_id, created_at)
note(id TEXT PRIMARY KEY, unit_id, author, body, created_at)
rule(id TEXT PRIMARY KEY, prefix, crate, layer)
verification(unit_id TEXT PRIMARY KEY, source_hash, commit_id, created_at)
```

`verification` 是新增表：记录 unit 被标为 `verified` 时的 C++ `source_hash`，用于 `changed`。

### 4.1 状态机

```text
todo → claimed → in_progress → review → done → verified
         ↑            ↓
         └── blocked ─┘
any → ignored（终态，需 reason）
```

### 4.2 单元粒度

- `class` / `struct` / `enum`：一个 unit。
- 类内成员函数：每个 `method` 一个 unit；class 本身保留一个 unit。
- 文件作用域函数：一个 `function` unit。
- 类外定义 `Foo::bar`：一个 `function` unit。
- 宏 / 全局常量 / 文件级静态数据：单独的 `macro` / `const` unit。
- 不需要迁移的 inline helper：`ignore` + reason。

### 4.3 crate 映射

`scan` 只填 path/symbol/kind；`map` 按最长 path prefix 填 crate/layer。

规则在：

[`tool/fkv_ledger/src/map.rs`](../tool/fkv_ledger/src/map.rs)

当前仍有 352 个 unit 未映射，说明规则需要继续补；它们会出现在
`doc/progress.md` 的 `<unmapped>` 行。

## 5. 验证阶梯

| 级别 | 内容 | 通过标准 |
|---|---|---|
| V0 | 单元测试 | `cargo nextest` 全绿 |
| V1 | 格式 round-trip | 编码 → 解码 → 再编码字节一致 |
| V2 | 格式差分 | 同输入 Rust / C++ **builder 输出**一致（checksum 见下） |
| V3 | 操作差分 | 同一 op log 分别跑 Rust / C++，结果一致 |
| V4 | 交叉读 | C++ 写 → Rust 读/续写 → C++ 读 |
| V5 | 崩溃恢复 | kill -9 后 replay 正确 |
| V6 | 性能 | 同 runner（4 KiB 页 NVMe）固定 workload；`perf-baseline` 记录绝对值 + Rust/C++ 同机比值；缺基线或不在 NVMe 上直接红 |

V2 checksum 约定（fmt-06）：C++ `DataPageBuilder::Finish` / `IndexPageBuilder::Finish` /
`OverflowPage` 构造**不写 checksum**（前 8 字节为 0），checksum 由写任务落盘前统一补。
Rust 对应提供 `finish_without_checksum()` / `build_overflow_without_checksum()`，
差分 harness 必须用它（或对两边统一 `set_checksum` 后）再逐字节比较；
直接比较 `finish()` 与 C++ `Finish()` 会只在前 8 字节不同。

当前实际覆盖：

- **V0**：workspace 单测 / nextest，CI 全量。
- **V1**：格式 round-trip 与篡改检测，`fkv_ledger/tests/corpus.rs` 覆盖全矩阵。
- **V2**：格式层 5 类 16 case 的 C++ builder 字节 → Rust 解码 / 重编码逐字节比对
  （`fkv_ledger verify` + `fkv_page/fkv_compress/fkv_manifest` 的 `cpp_oracle` 测试）。
- **V4**：格式层双向——C++ 写 → Rust 读（`fixtures/cpp`）、Rust 写 → C++ 读
  （`fixtures/rust`）；CI `fixture-diff` 重新生成并检查漂移。**引擎级 V4（含续写）未执行**。
- **V5**：确定性闭环已落地。`fkv_io_sim/tests/op_log_replay.rs` 对 6 条 op log
  穷举每个逻辑 op 边界，crash + restart + reopen 后与同前缀干净运行逐字节比对；
  `fkv_store/tests/crash_failpoint_invariants.rs` 在真实 compio 后端穷举 7 个
  命名 kill point × 命中序号（默认 6 次，`FKV_CRASH_HIT_MAX` 可调），失败信息含
  seed / kill_point / op_index 与复现命令，前缀检查器另有反向验证。未覆盖 24h
  持续 fuzz、指令级随机 kill point、sim 目录项语义与 `CloudManifest:Put`。
- **V3**：未系统化（无同一 op log 的 Rust/C++ 操作差分）。
- **V6**：`.github/workflows/perf.yml` + `scripts/perf_gate.sh` 已接成**可阻断**门槛：
  下载 fkv 仓的 `perf-latest`（Rust 二进制）与 `perf-baseline`（同 runner C++/Rust
  绝对值 + 比值），在 4 KiB 页 NVMe 上按固定 workload 重跑 Rust，再比
  「current rust / baseline rust」与「current rust / baseline cpp」。缺
  `perf-baseline` 默认 exit 78（skipped-with-error），仅显式
  `PERF_GATE_REQUIRE_BASELINE=0` 才降级；非 Linux / 非 4 KiB / 非 NVMe 同样 exit 78，
  不会给出绿色。workload 固定为 `write-read`、`rows=max_key=1000000`、
  `batch_size=100`、`partitions=1`、`read_per_part=4`、`kv_size=128`、
  `key_interval=4`、`del_ratio=4`、`test_secs=30`、`write_batchs=10000`、
  `scan_bytes=0`；C++ 侧用 `data_append_mode=true` 的 ini 与 `simple_bench`
  同参数。`scripts/perf_gate.sh selftest` 反向验证「Rust 吞吐人为降 30% 必红」。
  已知口径差异：C++ `simple_bench` 的 write-read 是写/读并发，fkv_bench 是
  先写后读的顺序阶段；主判据用 current-rust / baseline-rust（同形状），
  Rust/C++ 比值只作同 runner 趋势，不承诺绝对值等价。
  当前唯一外部前置：在指定 4 KiB NVMe runner 上跑一次
  `perf.yml` 的 `publish_baseline=true`，生成并上传 `perf-baseline` 资产；
  资产缺失期间 CI 会持续红，而不是静默通过。

因此 `COMPAT_BASELINE` 的 `L1-partial` 只表示「代码路径对齐到 append + 验收默认无压缩 +
格式层双向语料已验证」，**不构成引擎级双向字节兼容结论**；没有引擎级 V3/V4 evidence
前不得对外声称已验证。

## 6. Definition of Done

一个 unit 只有满足以下条件才能从 `done` 进入 `verified`：

1. 代码合入，fmt / clippy / workspace 测试通过。
2. 格式/算法单元：至少一条 V1 或 V2 evidence。
3. 引擎路径单元：至少一条 V3 或 V4 evidence。
4. 崩溃敏感单元：V5 evidence。
5. 性能关键单元：V6 evidence。
6. `fkv_ledger` 中存在 `result=pass` 的 evidence。
7. `compat` 等级已更新。
8. 相关文档/注释已更新。

`verified` 的证据检查已由 CLI 强制；证据与源码版本的新鲜度由
`verification.source_hash` + `changed` 检查。

## 7. 当前里程碑

| 阶段 | 状态 | 说明 |
|---|---|---|
| M0 脚手架 | 完成 | workspace / 工具链 / 账本 |
| M1 格式层 | 完成主体 | page / manifest / codec / TTL / large value / overflow 读写与双向语料；缺 `mapping`/`store` 类 oracle 与引擎级交叉读 |
| M2 读路径 | 完成主体 | read/scan/floor/page IO、index 页缓存与预取；读并发、in-flight / 读批指标（eng-32c）与 27c 快照验收已落地；`MappingSnapshot::open` 已改为 O(1) 指针表共享（rev-45），无未收敛代码缺口 |
| M3 写路径 | 完成主体 | COW write / reopen / TTL / compression / 自动字典训练 / overflow / group commit；跨分区写并发（eng-33a：`WriteFlight` 并发派发 + `WriteGate` 全局额度）与 merge 单趟排序折叠零 clone（eng-90）均已交付，写路径无未收敛缺口（见 [`eng-33a`](../issue/reviewed/eng-33a-write-gate-concurrent-dispatch.md)、[`eng-90`](../issue/reviewed/eng-90-merge-put-batches-per-entry-clone.md)） |
| M4 本地运行时 | 完成主体 | shard/runtime/GC/archive/branch、读 spawn、高/低优先 + yield 预算；单 Store 多 DS 托管为范围外有意偏离（每进程一个 attach 身份，见 [`sto-61b`](../issue/reviewed/sto-61b-multi-ds-shard-hosting-and-request-routing.md)）；per-table 控制面与 `FileGc` / `LocalGc` 请求已接线，GC 已按 `(branch, term)` 变体粒度落地 |
| M5 云端/standby | 完成主体 | Runtime 已接 Cloud/Standby；Cloud 增量 `put_range`、预热（含 reopen per-table 预热，rev-38）、auto-reopen 重试策略（rev-37）、standby 多根 master 选根已落地；GC 已按 branch/term 变体粒度落地 |
| M6 大 value/零拷贝 | 完成主体 | segment + fixed IO；Linux 完整，其他平台退化；缺 `GlobalRegisteredMemory` 段 slab |
| M7 硬化 | 未完成 | 24h / crash fuzz 闭环、V3/V6 系统化 |

详细差距见 [`issue/new/00-index.md`](../issue/new/00-index.md)。

## 8. CI 与性能

- fkv 仓 `ci.yml`：Ubuntu x86_64/aarch64，fmt / clippy / `tool/orphan.sh` /
  nextest；另有 S3（RustFS）与 web build 作业。
- fkv 仓 `release.yml`：构建 x86_64/aarch64 `fkv_bench`，发布滚动 `perf-latest`。
- 性能对比按 `web/doc/zh/doc/performance.md` 手动执行，结果写回 Markdown。
- `oracle.yml`：
  1. `fixture-diff` 作业用 `actions/checkout` 的 **LFS** 拉取 `*.bin` 语料；
  2. 按 `COMPAT_BASELINE` pin 构建 `cpp/oracle`；
  3. `cargo run -p fkv_ledger -- fixture --matrix --oracle /tmp/fkv_oracle
     --out fixtures/cpp` 重新生成 C++ 语料（每个 case 都写 `meta.txt`）；
  4. `git diff --exit-code -- fixtures/cpp` 检查 **payload 与 meta 漂移**；
  5. Rust 侧 `cpp_oracle` 解码测试 + `fkv_ledger verify`（Rust 断言 + C++ 交叉读）；
  6. 反向 `fixture --rust --matrix` 写 `fixtures/rust`，再跑 `verify` 让 C++ 读回；
  7. 上传双向 fixture 与报告。

  **口径**：`meta.txt` 的 `baseline=` 是 `COMPAT_BASELINE` 全文压行，因此改
  `COMPAT_BASELINE` 必须同时重跑 `scripts/gen_fixtures.sh` 刷新 38 份 meta，
  否则 drift 检查会失败。`mapping` / `store` 是占位 kind，保持 `status=skipped`，
  不伪造 fail。
- 性能目标：同机 Linux NVMe，Rust/C++ p99 ≤ 1.2x，吞吐 ≥ 0.85x。
- `gen_fixtures.sh` 是本地一键复现入口：构建 oracle → 双向生成 → 双向 verify，
  报告写到 `/tmp/fixture-{cpp,rust}-report.md`。

## 8.1 代码推断差异（待 V2/V4 验证，不保证逐字节等价）

| 差异 | 说明 |
|---|---|
| 压缩保留条件更严 | C++ 只比较压缩体长度；Rust 比较「varint 前缀 + 压缩体」。仅在 1~5 字节边界决策可能不同，解压侧不受影响。 |
| manifest 对齐 = OS 页大小 | 与 C++ `page_align = sysconf(_SC_PAGESIZE)` 同源：Linux 4 KiB、Apple Silicon 16 KiB。跨页大小主机之间 manifest framing 不兼容（C++ 同样如此），双向验收需在同一页大小主机上执行。 |
| index 迭代不产出 leftmost 虚拟 entry | Rust 把 leftmost 拆成 `IndexPage::leftmost_ptr()`；盘面字节与 C++ 相同。 |
| `IndexPageIter::seek` 是 ceiling | C++ 同名函数是 floor；树下降必须用 `seek_floor` + leftmost（已写入文档与测试）。 |
| 扫描遍历来源是叶子 next 链 | Rust 扫描下降到 data page 后按 `next_page_id` 单向链表推进；C++ `ScanTask::AdvanceToNextLeaf` 沿 `index_stack_` 推导顺序，`NextPageId()` 只在写路径维护。正确性因此依赖写路径（页重分配 / reopen / TTL 重写 / rebalance）维持链；链陈旧会串页、跳页。缓解：扫描做 O(1) 链环检测（超逻辑页槽位上界即 `Corrupted`）；预取只能 1 页的深度限制另见 eng-42 / eng-52。 |
| 收缩后稀疏写入的回填值（**有意偏离**） | 场景：`abort_batch` 回滚把 `MappingTbl` 收缩，之后再稀疏写到同一「未释放的 chunk」内（如 len 100→5 后 `set(39)`）。C++ `ResizeInternal`（`src/storage/page_mapper.cpp:266-300`）增长时只 `fill(InvalidValue)` **新分配的 chunk**，已存在 chunk 尾部的 stale 槽保持原值；Rust `MappingTbl::resize`（`crate/core/fkv_mapping/src/lib.rs:211-239`）把 `[old_len, len)` 显式填 `INVALID`。后果：同一逻辑状态下两侧 manifest 字节不同 —— C++ 会把回滚前残留的 `FilePageId` 序列化出去（解码方按活跃页计入 retained，多保留本可回收的文件），Rust 写 `Invalid`。**判定：保持 Rust 行为**，因为「stale 槽恰好没被覆盖」是 C++ 实现的副产物而非契约，且把已回滚页当活跃页是错误方向。跟踪：core-29。 |
| 页级 pin / `ref_cnt` 不做 1:1（**有意偏离**） | C++ `MemCachedPage.ref_cnt_` 让读任务下降树期间 pin 住页、`Evict` 跳过 pinned（`src/storage/page_manager.cpp:776`），解决的是多线程「使用中页被逐出」。fkv 单线程下由借用规则覆盖：`IndexCache::get` 返回的 `&IndexNode` 借用不跨 `.await`（`fkv_task_read::descend_step`），驱逐只发生在同任务 `insert` / `trim_to_budget` 的 `&mut self` 内；读请求不跨请求持页——data 页读入调用方自有缓冲、index `seek` 返回 owned `PageId`，27b 读跨 `await` 持的是 `MappingSnapshot`（映射层）而非页引用。重开条件：eng-33a 写并发落地后实测出现「下降途中被逐出重读」放大。已裁定：rev-41（`issue/done/`）。 |
| index 页并发首访不去重（WaitingZone 缺失，**有意偏离**） | C++ `FindPage` 首访以 `AddSwizzling` 占位 + `waiting_.Wait/WakeAll`，并发首访同一页只发一次 IO（`page_manager.cpp:663-729`）。fkv 读并发窗口（eng-32b：≤`MAX_READ_BATCH`=32 条独立 task 共享 `RefCell<IndexCache>`）下冷缓存确实各自 miss、各自发 IO，但放大有界：每冷窗口 ≤ 32 × 树深（实际 2~4，`MAX_TREE_DEPTH=64` 仅为上界）次 4 KiB index 页读；默认 buffered IO 下重复读命中内核 page cache，`with_direct_io(true)`（io-21）时才是真设备读。等效 single-flight 的集成点在 `fkv_task_read::descend_step`（miss → `read_page` → `insert` 回填），超出 index 缓存层、需跨 crate 改造。重开条件：IO 计数实测单冷窗口重复 index 读显著超理论下限，且写并发落地后仍存在。已记录：rev-41（`issue/done/`）。 |
| swizzle 直取快路径不做（**有意偏离**） | C++ `FindPage` 先查 mapping 槽内 `SwizzlingPointer` 直指常驻页省一次哈希（`page_manager.cpp:663-669`），代价是驱逐 / 快照销毁要全量 `Unswizzling`（`:800`）。fkv `IndexCache`（PageId 键 + 映射代际门控）已提供同等「热 index 页零 IO」，残差仅每树层一次哈希查找；把 `SwizzlingPointer` 变为可写值域将波及 `is_free_slot` 回收判定、恢复扫描、reopen 缓存复用与 GC retained 计算，复杂度不成比例。reopen 的 swizzle 复用语义已由 `IndexCache::retain_mapped` + `(branch, term, fpid)` 身份校验等价落地（eng-67）。重开条件：profile 证明缓存查找在点读路径占比可测（>5%）。已记录：rev-41（`issue/done/`）。 |
| 用户态 data page cache 不做（**有意偏离**） | C++ `enable_data_page_cache` 默认 `false`（`include/kv_options.h:58`；关闭时 `src/tasks/task.cpp:119` data 页每读一次 IO），默认配置两侧行为同构，无「C++ 有而 fkv 无」的默认行为差。fkv 默认 buffered IO 多一层内核 page cache 兜底冷读；`with_direct_io(true)` 时两侧同为直读（io-21）。Rust 侧 `fkv_config::enable_data_page_cache` 是死旋钮：INI 解析保留（`crate/foundation/fkv_config/src/lib.rs:175`）但全仓无消费者，为 INI 面兼容保留不删。重开条件：C++ 默认值改 `true`，或直读模式实测冷 data 读成为吞吐 / p99 瓶颈。已裁定：rev-41（`issue/done/`）。 |

## 8.2 L1-partial 范围与排除项

`COMPAT_BASELINE` 的等级是**声明范围**，不是已验证结论。当前范围与排除项如下，
升级为「已验证」前不得省略：

| 维度 | 当前口径 | 跟踪 |
|---|---|---|
| 分配模式 | Rust 恒用 `AllocMode::Append`（`STORE_ALLOC_MODE` 编译期常量，改动即编译失败）；C++ 默认 `data_append_mode=false` 走 `PooledFilePages`，不在对齐范围。互操作验收只接受 append store | sto-59 |
| 压缩 | 验收默认 `enable_compression=false`；压缩读写已实现，但保留决策非逐字节等价（varint 前缀 + 压缩体 vs 仅压缩体） | §8.1、sto-56 |
| 恢复语义 | main / branch 本地 manifest 的撕裂尾已按 `valid_len` 截断自愈（sto-51/51a）；`CloudStore` 的具名 manifest 截断仍是默认 no-op。`GetMapper` 分配器重建差异按 sto-52 的结论记录，不纳入互操作保证 | [`sto-51`](../issue/reviewed/sto-51-replayer-corrupted-tail-never-overwritten-permanent-corrupted.md)、[`sto-52`](../issue/reject/sto-52-replayer-getmapper-allocator-reconstruction-divergence.md) |
| branch + large value | C++ `CreateBranch`（82b774c）的 branch snapshot 不传 segment mapping，branch manifest 无 segment 段；Rust 为 large value 分支继承与 main/branch segment file_id 区间隔离仍写该段，**有意差异，不纳入逐字节等价**（plain store 未启用 large value 时两侧 branch snapshot 字节同构）。双向后果：Rust 若对齐 C++ 省略该段，分支重开后父分支 large value 全部不可读（继承被破坏）；C++ 侧自身该组合亦断链——reopen 无 segment 段时 `GetSegmentMapper` 返回空，底页 large value 指针无法解析（上游 `branch_operations` 测试无 large value 用例）。依据：`COMPAT_BASELINE:10-13`、`doc/architecture/06-gaps.md:45` 已有登记 | [02-cpp-create-branch-omits-segment-mapping](../issue/reviewed/02-cpp-create-branch-omits-segment-mapping.md) |
| 证据 | 格式层 5 类 16 case 双向语料 + CI 漂移检查；`mapping`/`store` 占位 skipped；引擎级 V2/V4、V3/V6 未系统化；V5 已有确定性崩溃闭环（Rust 侧不变量，非 C++ 差分） | [01-cpp-oracle-and-fixtures](../issue/reviewed/01-cpp-oracle-and-fixtures.md)、[`fixtures/README.md`](../fixtures/README.md) |
| 混部（C++ master ↔ Rust replica） | 允许 C++ master 的 `manifest_main_<term>` 被 replica 接收：standby 侧已按本地活跃名 `manifest_main_0` 安装 + 清旧 term manifest + fsync 目录（sto-70）。数据文件读侧已放宽：`fkv_io_compio::scan_dir(dir, branch, term)` 保留全部非当前 identity 变体，`data_path_li` 当前 identity 优先、其余按 term 降序回退（segment 同规则），因此 replica 的 attach term / branch 与 master 写数据时不同也能读到；CloudStore 读侧按 manifest 区间映射出的 `(branch, term)` 定位对象，manifest 指向哪个 term 就读哪个 term。跨 branch 的可见范围仍由 branch 归档 / GC 语义约束 | sto-70 |

升级前置：sto-51 的 Cloud 具名 manifest 截断残余处理完、引擎级 V2/V4（含续写）
产出证据、补齐 V3/V5/V6 中的适用项，并在 `fkv_ledger` 写入 `verified` 记录。
在此之前，`done` 只代表 Rust 单元测试通过，不代表引擎级盘面兼容。

## 8.3 写 IO 并发默认值口径（io-40）

`max_inflight_write` / `max_write_batch_pages` 的库默认值以 C++ `include/kv_options.h`
字段初始值为准：`32 << 10` / `32`。

`src/kv_options.cpp:152-160` 里的 `4096` / `64` 位于 `reader.HasValue(...)` 分支内，
是「INI 键存在但取不到值」时的解析兜底；`db_stress` gflag 的 `4096` / `64` 是压测
入口默认值。两者都不是 `KvOption` 的默认构造结果，不作为 Rust 默认值依据。

Rust 侧在飞写组上限只有一个来源：`KvOption::max_inflight_write`，经
`ShardConfig` / runtime / SDK 投影到 `CompioStore`；`max_write_batch_pages` 只控制
单组合并页数。注册写槽位缓存另有 `FIXED_WRITE_SLOT_LIMIT`（16）上限，它是缓冲
缓存上限、不是在飞上限，超出后回落普通写池。

## 9. 参考

- fish-shell C++ → Rust：<https://github.com/fish-shell/fish-shell/blob/master/doc_internal/fish-riir-plan.md>
- JetBrains C++ → Rust：<https://blog.jetbrains.com/rust/2026/07/27/cpp-to-rust-migration/>
- Strangler Fig：<https://martinfowler.com/bliki/StranglerFigApplication.html>
- Crubit：<https://github.com/google/crubit>
- motedb：<https://docs.rs/motedb/latest/motedb/>
