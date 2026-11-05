# sto-57 KvOption：INI 解析语义与 C++ 分节模型/单位文法不符，且缺 partition_filter 等字段

## 位置
- Rust: `crate/foundation/fkv_config/src/lib.rs:244-338`（`load_from_ini`）、`361-377`（`parse_size`）
- C++: `src/kv_options.cpp:86-386`（`LoadFromIni`）、`20-84`（`ParseSizeWithUnit`）、`267-301`（`ParseStorePathListWithWeights` 带权后缀）、`include/kv_options.h:37、210-222、445`（缺失字段）

## 现状
Rust `load_from_ini`（todo.md 说「INI 手写 FSM 解析」，实际是逐行 `split_once('=')`）：
- **忽略 section**：`[run]`/`[permanent]` 与任意其他 section 一律跳过，任意 section 下的 key 都会覆盖默认值；
- **单位文法**：`parse_size` 接受 `b/k/m/g/t` 单字符与 `kb/mb/gb/tb`（361-377 行），`"128"` 按字节，无单位但带空格（`" 128 "` 截尾后为 128）也接受；
- **未知 key 静默忽略**、解析失败返回 `InvalidArgs`；
- **store_path 权重**只认独立 key `store_path_weights`，不解析 C++ 的 `store_path=path1:<2,1>` 后缀语法；`store_path` 直接逗号 split。

C++ 侧：
- `LoadFromIni` 用 inih，**要求 `[run]` 与 `[permanent]` 两个 section 都存在**（缺失返回 -2），key 只在所属 section 内生效（`kv_options.cpp:93-97、261-265`）；
- `ParseSizeWithUnit` 只认两位尾缀 `KB/MB/GB/TB`（大小写不敏感），空串/解析失败**静默返回 0**（20-84 行）；
- `store_path` 通过 `ParseStorePathListWithWeights` 解析 `path:<w1,w2>` 后缀，权重与路径绑定，权重语法错误返回 -3；
- `[permanent]` 内还有 `data_file_size`（341-374 行，非 2 幂向下取整、小于 page 退化 shift=0 并 WARNING）。

## 差异与风险
- **同一 INI 文件两边得到不同配置**：C++ 写在 `[permanent]` 的 `data_page_size=16KB`，Rust 仍能读到（忽略 section）——这里凑巧兼容；但 C++ 的 `store_path=/data:<2,1>` 在 Rust 里变成字面目录 `/data:<2,1>`（不存在的路径，运行时才炸），反之 Rust 的独立 `store_path_weights` key C++ 完全不识别；
- **默认值歧义**：C++ INI 的 fallback 参数（`fd_limit` 1024、`coroutine_stack_size` 16KB、`num_retained_archives` 0、`archive_interval_secs` 86400）只在 key 存在但值不可解析时生效，Rust 没有此区分；
- **data_file_size**：C++ 非 2 幂向下取整 + WARNING，Rust `InvalidArgs` 报错（`fkv_config/src/lib.rs:326-336`）；
- **缺失字段**：`comparator_`（kv_options.h:37）、`partition_filter`（445 行，基线 commit 82b774c 的多 shard 托管核心）、`global_registered_memories` / `pinned_memory_chunks`（210、222 行，fixed buffer 注册来源）。`partition_filter` 缺失使 Rust 无法表达「一个 Store 托管多个 data store shard 时分区归属」，与 sto-61 联动；
- `store_path_lut` 字段存在但 Rust 从不构建（C++ 在 `InitStoreSpace` 按磁盘容量生成 LUT）。

## 对齐建议
- `load_from_ini` 改分节 FSM：进入 `[run]`/`[permanent]` 才接受对应 key，未知 section 忽略、缺 section 按 C++ 返回错误码语义处理（或明确记录「宽松模式」文档）；
- `store_path` 解析支持 `path:<w1,w2>` 后缀（对齐 `ParseStorePathListWithWeights`），权重与路径绑定校验；
- `parse_size` 对齐 C++ 单位文法（KB/MB/GB/TB 尾缀）或明确文档化差异；`data_file_size` 按 C++ 取整策略（向下取 2 幂 + WARNING）对齐；
- 补 `partition_filter`（`std::function<bool(DsShardId, TableIdent)>` 等价 `Fn(DsShardId, &TableIdent) -> bool`）并接入 runtime 的 AttachShard 语义（见 sto-61）；`comparator`、`global_registered_memories`、`pinned_memory_chunks` 至少在 TODO/文档登记为「不迁移」决策；
- `validate()` 补 C++ `ValidateOption` 有的：gcs/google provider 的 endpoint 默认值（`eloq_store.cpp:132-137`）、standby 自动开启 append 的 WARNING（`eloq_store.cpp:244-248`）、`max_write_concurrency` cloud 下改写。

## 性能优化建议
无。

## 严重度
P1（行为不一致/功能缺失）

## 审查结论（2026-11-04）

**部分采纳。** 逐条核验结果：

| issue 条目 | 结论 | 去向 |
|---|---|---|
| 忽略 section / store_path 内联权重 / data_file_size 取整 | 已由并行会话 cfg-01 实现（`load_from_ini` 节状态机 + `parse_store_path` 内联权重 + `ilog2` 向下取整），见 `issue/done/cfg-01-ini-parsing-sections-weights-and-file-size-rounding.md` | 不重复处理 |
| `parse_size` 单位文法与 C++ 不符（接受 `b/k/m/g/t` 单字母） | 属实。C++ `ParseSizeWithUnit` 只认两位尾缀 `KB/MB/GB/TB`，单字母会解析失败 | sto-57a |
| `validate()` 缺 cloud/standby 派生（gcs endpoint 默认、`max_write_concurrency` 改写、追加写自动开启、`local_space_limit=0` 回退） | 属实 | sto-57b |
| 缺 `comparator_` / `partition_filter` / `global_registered_memories` / `pinned_memory_chunks`；`store_path_lut` 从不构建 | 属实，但前两者/后两者在当前 runtime 无消费点，直接加字段是死代码；`partition_filter` 依赖 sto-61 的 `DsShardId` 路由 | sto-57c（决策登记） |
| 默认值 fallback 语义（key 存在但值不可解析时 C++ 用 fallback） | 不采纳对齐。C++ 静默 fallback 会掩盖配置错误，Rust 保持 `InvalidArgs` 属于更安全的有意差异，在 sto-57c 记录 | sto-57c |
| `store_path_weights` 独立 key C++ 不识别 | 已由 cfg-01 删除 INI 键，字段仅由 `path:<w1,w2>` 后缀填充 | 无需处理 |

拆分：`sto-57a`（单位文法）、`sto-57b`（validate 派生）、`sto-57c`（未迁移字段决策登记）。

## DONE（2026-11-04）

- cfg-01（分节 / 内联权重 / data_file_size 取整）已由并行会话完成，本 issue 不重复。
- sto-57a：`parse_size` 单位文法对齐 C++（仅两位尾缀），保留 `InvalidArgs` 严格性。
- sto-57b：`validate` 补 GCS endpoint 默认、cloud `max_write_concurrency`/`local_space_limit` 回退、cloud/standby 追加写自动开启。
- sto-57c：`comparator_`/`global_registered_memories`/`pinned_memory_chunks` 登记不迁移，`partition_filter`→sto-61，`store_path_lut`→eng-72。
- 验证：`cargo check -p fkv_config` / `cargo nextest run -p fkv_config -p fkv_runtime -p fkv_sdk` 44 passed / clippy `-D warnings` clean / nightly fmt clean。

## 复审（2026-11-05 r6b）

通过（父条目收口）。三个拆分件 sto-57a / sto-57b / sto-57c 同批复核全部通过（证据见各文件），cfg-01 部分早已在 `issue/reviewed/`。本条自身无剩余未闭环声称。
