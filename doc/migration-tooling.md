# 迁移工具链（现状与路线）

## 1. 目标

让“哪个 C++ unit 已经迁完、证据是什么、下一步做什么”不依赖聊天记录或个人记忆。

单一事实源：

1. `motedb` 账本：unit / evidence / verification
2. `COMPAT_BASELINE`
3. fixtures / 测试 / benchmark 产物

当前阶段最重要的不是继续加功能，而是**让账本反映真实实现状态**。

## 2. 当前快照（2026-11-04，adopt 对齐后）

| 项 | 值 |
|---|---|
| 数据库 | `.data/motedb.mote` |
| C++ unit | 1848 |
| `done` | 1305（由 `doc/adopt.tsv` 声明对齐） |
| `todo` | 543（含未映射 352） |
| `verified` | 0（格式层已具备交叉读证据，引擎级 V2/V4 未系统化；见 issue 01） |
| 审查看板 | 由 `report` 实时统计 `issue/{new,todo,done,reviewed,reject}`，写入 `doc/progress.md` |
| 兼容基线 | `eloqstore main 82b774c` |
| C++ oracle | `cpp/oracle` 5 类 16 case；`fixtures/cpp` 16 真实 payload + 6 占位，`fixtures/rust` 16 反向 |
| 最新差距清单 | [`issue/new/00-index.md`](../issue/new/00-index.md) |

结论：账本已能回答「哪些 crate 已迁移且有测试」（done）、
「哪些还没动」（todo / unmapped）。「已迁移」的下一步证据是引擎级差分 / 交叉读
（verified）；格式层的双向语料已先行落地。

## 3. 工具状态

### 3.1 已实现

[`tool/fkv_ledger`](../tool/fkv_ledger)

| 命令 | 作用 |
|---|---|
| `sync` | 更新 baseline + scan + map |
| `baseline` | 读取/更新 `COMPAT_BASELINE` |
| `scan` | tree-sitter 扫描 C++ → `unit` / `include_edge` |
| `map` | path prefix → crate / layer |
| `adopt` | 按 `doc/adopt.tsv` 批量对齐 unit 状态 + 补测试证据（幂等；`--plan` 只看计划） |
| `issues` | 统计 `issue/` 看板各目录条目数（随 `report` 一并输出） |
| `next` | 下一批待办 |
| `list` | 按 status / crate 过滤 |
| `claim` | 认领 |
| `status` | 状态流转；`verified` 必须有 pass evidence |
| `ignore` | 忽略并记录 reason |
| `evidence add/list` | 测试/差分/交叉读/crash/bench 证据 |
| `edges` | C++ include 依赖 |
| `stats` | 状态统计 |
| `changed` | verified 后 `source_hash` 变化的 unit；`--check` 可做门禁 |
| `report` | 生成 Markdown 进度报告 |
| `export` | 导出 JSON 给 dashboard / 脚本 |

新增的 `verification` 表记录 verified 时的 `source_hash`，让 `changed` 可以发现
C++ 源码变化后需要重验的 unit。

### 3.2 待实现

| 命令 | 作用 | 优先级 |
|---|---|---|
| `scaffold` | 按 unit 生成 Rust 模块骨架 + TODO + 测试文件 | P1 |
| `report --dashboard` | 直接生成 web 看板数据 | P2 |
| `compare` | 合并 Rust/C++ benchmark，输出比值 | P1 |

### 3.3 fixture / verify（脚手架已实现）

| 命令 | 作用 |
|---|---|
| `fixture` | 生成 fixture 目录与清单（`meta.txt` + `payload.bin`）；无 oracle 时写 `status=skipped` |
| `verify` | 遍历 fixture 目录逐 case 校验，输出 Markdown 报告；有 fail 时退出码非 0 |

约定目录与 oracle 接口见 [`../fixtures/README.md`](../fixtures/README.md)。
`cpp/oracle` 已实现 page / index / overflow / manifest / compress 5 类 16 case 的
真实字节；`mapping` / `store` 仍是占位 kind（0 字节、`status=skipped`）。Rust 侧
`verify` 先跑解码 + 重建字节断言，再调 oracle 交叉读；CI `fixture-diff` 用
`git diff --exit-code fixtures/cpp` 检查 payload 与 `meta.txt` 漂移。

## 4. 开发流程

### 4.1 怎么同步当前开发（两条进度线合一）

真实进度有两个来源，用同一份 `doc/progress.md` 汇总：

1. **代码线（账本）**：一个 crate 迁移完成且测试通过 → 在
   [`adopt.tsv`](./adopt.tsv) 加一行 `crate done <测试目录>` →
   `cargo run -p fkv_ledger -- adopt`（幂等，只推进 todo/claimed 等，
   不动 done/verified/ignored，并为每个 unit 补一条指向测试目录的 pass 证据）。
2. **审查线（issue/）**：按 [`issue/run.md`](../issue/run.md) 流程
   甄别 → todo → 开发 → done → reviewed 移动文件即可，
   `report` 会自动统计各目录条目数。

单个 unit 的精细操作仍走 `claim` / `status` / `evidence add`。

**DB 重建**（`.data/motedb.mote` 不入 git）：

```bash
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- adopt
cargo run -p fkv_ledger -- report --out doc/progress.md
```

`adopt.tsv` 是可提交的声明文件，上面三条命令可完整重放账本状态。

### 4.2 单 unit 精细流程

```text
sync
  → next --crate X
  → claim
  → 开发 + 测试
  → evidence add ... --result pass
  → status done
  → status verified（CLI 校验 evidence）
  → report --out doc/progress.md
```

提交前：

```bash
cargo +nightly fmt --check
cargo clippy --workspace --all-targets -- -D warnings
./test.sh
cargo test -p fkv_ledger
cargo run -p fkv_ledger -- changed --check
```

每周（或每次 adopt.tsv / issue 目录变动后）：

```bash
cargo run -p fkv_ledger -- adopt
cargo run -p fkv_ledger -- report --out doc/progress.md
cargo run -p fkv_ledger -- export --out /tmp/fkv-ledger.json
```

## 5. 证据等级

| 等级 | 证据 | 适用 |
|---|---|---|
| V0 | `cargo test` / `nextest` | 所有 unit |
| V1 | round-trip / proptest | codec、page、manifest、mapping |
| V2 | C++ vs Rust byte/structure diff | 格式层 |
| V3 | 同一 op log 操作差分 | read/scan/write/reopen |
| V4 | C++ 写 → Rust 读/续写 → C++ 读 | 端到端 |
| V5 | kill -9 / crash replay | 写路径、manifest、GC |
| V6 | Rust/C++ benchmark 比值 | 热路径 |

`verified` 至少需要 V0；格式/引擎单元按上表要求更高等级。
当前 CLI 已强制“至少一条 pass evidence”，但还没有按 unit 类型强制 V2/V4。

## 6. 指标

| 指标 | 定义 | 目标 |
|---|---|---|
| verified 覆盖率 | (verified + ignored) / total | 1.0 前 100% |
| 兼容通过率 | V2/V4 pass / 执行数 | 100% |
| changed 数 | verified 后源码变化的 unit | 每周清零 |
| 性能比值 | Rust/C++ p99、吞吐 | p99 ≤ 1.2x，吞吐 ≥ 0.85x |
| 未映射 unit | crate 为空 | 持续下降 |

`doc/progress.md` 由 `report` 生成，包含状态、crate 分布、changed、下一批任务。

## 7. 路线图

| 阶段 | 交付 | 状态 |
|---|---|---|
| P0 | sync / scan / map / next / list / claim / status / ignore / evidence / edges / stats | 已完成 |
| P0 | verified 证据校验 / changed / report / export | 已完成 |
| P0 | C++ oracle 可执行文件 + 格式层双向交叉读 | 已完成：5 类 16 case，CI `fixture-diff` |
| P0 | 引擎级 V2/V4（含续写） | 未完成（`mapping`/`store` 也无 oracle 字节实现） |
| P1 | `scaffold` / `compare` | 未完成 |
| P1 | 真实 24h / crash / fuzz | 未完成（`fkv_failpoint` / `fkv_io_sim` 已具备积木） |
| P2 | dashboard / PR 自动评论 | 未完成（issue 计数同步已由 `report` 覆盖） |

## 8. C++ oracle 现状与维护

`oracle.yml` 的 `fixture-diff` 作业已经不只是构建，而是完整双向流水线：

1. `actions/checkout` 开 `lfs: true`，否则 `*.bin` 只是 LFS pointer；
2. 按 `COMPAT_BASELINE` pin 拉取 eloqstore，构建 `cpp/oracle`；
3. `fkv_ledger fixture --matrix --oracle /tmp/fkv_oracle --out fixtures/cpp`
   重新生成每个 case（含 `meta.txt`）；
4. `git diff --exit-code -- fixtures/cpp` 检查 payload / meta 漂移；
5. Rust 侧 `cpp_oracle` 解码测试 + `fkv_ledger verify`（Rust 断言 + C++ 交叉读）；
6. `fkv_ledger fixture --rust --matrix` 写 `fixtures/rust`，再 `verify` 让 C++ 读回；
7. 上传双向 fixture 与报告。

维护注意：

- `meta.txt` 的 `baseline=` 是 `COMPAT_BASELINE` 全文压行；改基线必须重跑
  `scripts/gen_fixtures.sh` 刷新全部 38 份 meta，否则 drift 检查失败。
- `mapping` / `store` 没有 C++ 字节实现，保持 `skipped`，不伪造 `fail`。
- branch + large value 的 segment 段是有意差异，见
  [`issue/reviewed/02`](../issue/reviewed/02-cpp-create-branch-omits-segment-mapping.md)。
- 本地一键复现：`ELOQSTORE_SRC=/path/to/eloqstore bash scripts/gen_fixtures.sh`。

## 9. 性能工具

- `crate/perf/fkv_bench`：端到端 workload + gxhash 指标；`--test-secs`
  （默认 60，0 = 固定次数）提供 C++ `simple_bench` 同口径稳态时间窗。
- `release.yml`：x86_64/aarch64 release binary + `perf-latest`。
- 性能对比在固定机器上手动跑 `fkv_bench` 与 `simple_bench`，结果写回
  `web/doc/<lang>/doc/performance.md`；站点直接渲染 Markdown，不维护 JSON
  数据层。
- 待补：同机 C++ `simple_bench` 的常态化产出（目前依赖人工上传 release asset）。

## 10. 相关文档

- [`doc/migration.md`](./migration.md)：迁移原则、状态机、DoD
- [`doc/progress.md`](./progress.md)：当前账本报告
- [`issue/new/00-index.md`](../issue/new/00-index.md)：代码审查差距看板
- [`todo.md`](../todo.md)：实际开发里程碑
- [`tool/fkv_ledger`](../tool/fkv_ledger)：账本 CLI
