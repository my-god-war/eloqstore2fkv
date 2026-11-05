# sto-59 `data_append_mode` 选项从未接线：Rust 引擎恒为 append，与 C++ 默认（非 append）不对称

## 位置
- Rust: `crate/foundation/fkv_config/src/lib.rs:76`（字段）、`147`（默认 false）；全仓无消费点（grep `data_append_mode` 仅命中 config 自身）；`crate/engine/fkv_store/src/lib.rs:267、825、1259`（所有 `PageMapper::new/with_tbl` 恒 `AllocMode::Append`）
- C++: `include/kv_options.h:394-397`（默认 false）、`src/replayer.cpp:319-391`（GetMapper 按 append/pooled 双路径）、`include/kv_options.h:90-99、112-124、173-175`（多组选项「Only take effect when data_append_mode is enabled」）

## 现状
`KvOption.data_append_mode` 字段存在、默认 false、可 INI 解析、可 validate，但：
- `Store`/`ShardConfig`/runtime 从不读取它；
- 恢复路径恒 `AllocMode::Append`（sto-52）；
- 选项语义联动缺失：C++ 中 `num_retained_archives`、`archive_interval_secs`、`file_amplify_factor`、`segment_file_amplify_factor`、`write_buffer_size/ratio` 等**全部以 data_append_mode 为前置条件**，Rust 无条件生效。

## C++ 实现
C++ 的 append/pooled 是分配器选择开关：默认 `data_append_mode=false` → `PooledFilePages`（原地复用空闲页、小文件放大低）；append → `AppendAllocator`（只追加、配合 archive/compaction 回收，云上传友好）。文档明确各 append-only 选项的生效前提（kv_options.h 各字段注释）。

## 差异与风险
- **互操作声明失真**：`COMPAT_BASELINE` 声称 L1 =「Local + append + 无压缩」。实际上 Rust 只有 append 一种分配模式，C++ 默认模式（pooled）的 store 由 Rust 打开可读，但 Rust 续写会切换为 append 布局（合法但行为与 C++ 同参数不同），「Local」等级的互操作声明未区分这两种子模式；
- **功能不等价**：任何以 `data_append_mode=false` 运行 C++ 的行为（空闲页复用、无 archive/compaction 语义的纯本地小文件模型）Rust 无法复现；反之 append 的 archive/compaction 联动（C++ 门控）在 Rust 无门控地总开；
- **文档/账本口径**：todo.md §1.1「模式对标」未把 append/pooled 作为模式维度写清楚，ledger 的 unit 映射里 `data_append_mode` 消费点缺失会导致「done」误标。

## 对齐建议
- 短期（文档化）：在 `COMPAT_BASELINE` 与 todo.md 明确「Rust 仅实现 append 分配模式；`data_append_mode=false` 的 C++ 行为不在对齐范围，互操作验收仅限 append 模式 store」；ledger 把该字段标 `ignored + reason`；
- 中期（实现）：`ShardConfig` 增加 `data_append_mode`，`install_replay_state` 按 C++ GetMapper 双路径重建（与 sto-52 合并实施）；archive/compaction/write-buffer 选项按 C++ 门控条件生效；
- 长期：若确认 1.0 只做 append，则从 `KvOption` 移除字段或加 `#[deprecated]`，避免「字段存在但不生效」的静默陷阱。

## 性能优化建议
无（先定语义）。

## 严重度
P1（功能缺失/行为不对称）

## 审查结论（2026-11-04）

判定：**降级为文档登记 + 显式校验**，pooled 分配器超出当前 L1 兼容范围。

- 兼容基线为 `L1（Local + append + 无压缩）`。`data_append_mode=false` 的
  `PooledFilePages`（原地复用空闲页）不在对齐范围；硬接半个 pooled 路径会牵出
  C++ `Replayer::GetMapper` 的 branch 区间过滤、free page 重建、`empty_file_cnt`
  等整套语义，已由 `sto-52` 跟踪，本次不实现。
- 已落地：
  - `fkv_config`：字段与模块级「未迁移字段」文档登记「语义登记、false 不切换
    分配器」；`validate` 在 local 模式显式 `warn`，消除「字段存在但不生效」的
    静默陷阱；新增测试 `validate_local_append_mode_inert`。
  - `fkv_store`：新增编译期常量 `STORE_ALLOC_MODE` 替代 8 处
    `AllocMode::Append` 字面量，并加编译期断言（改成 `Pooled` 直接编译失败）；
    各建 mapper 点行为不变。
  - 文档：`COMPAT_BASELINE`、`todo.md` §1.1、`doc/migration.md` §8.1 均已登记
    「Rust 仅 append，互操作验收限 append store」。
- 账本：`include/kv_options.h#struct_specifier:eloqstore::KvOption` 是 struct
  粒度 unit 且已 `done`，账本无字段级状态；把整 struct 标 `ignored` 会误伤已
  实现的 `KvOption` 映射，故不改账本，理由记录于此。
- 未做（留给 `sto-52`，需 L1 扩容才启动）：pooled 分配器接线、
  `empty_file_cnt`、archive/compaction/write-buffer 选项的 `data_append_mode` 门控。

## DONE（2026-11-04）

- 代码：`crate/foundation/fkv_config/src/lib.rs`（字段/模块文档 + validate 告警）、
  `crate/engine/fkv_store/src/lib.rs`（`STORE_ALLOC_MODE` 常量 + 编译期断言）、
  `crate/foundation/fkv_config/tests/config.rs`（新测试）。
- 文档：`COMPAT_BASELINE`、`todo.md`、`doc/migration.md`。
- 验证：
  - `cargo check -p fkv_store -p fkv_config --all-features`：exit 0。
  - `cargo nextest run -p fkv_store -p fkv_config --no-fail-fast`：94 passed, 0 skipped。
  - `cargo clippy -p fkv_store -p fkv_config --no-deps --all-features --all-targets -- -D warnings`：零警告。

## 复审（2026-11-05 r6b）

通过。

- `STORE_ALLOC_MODE` 编译期常量 + 断言：`crate/engine/fkv_store/src/lib.rs:73-77`（改成 `Pooled` 直接编译失败）；建 mapper 点统一引用（`:510`、`:1331`、`:1395`、`:1793`、`:1847` 等），`AllocMode::Append` 字面量无散落。
- validate 显式 warn：`crate/foundation/fkv_config/src/lib.rs:433-438`（local 下 `data_append_mode=false` 提示空操作，指向 COMPAT_BASELINE）。
- 文档登记：`COMPAT_BASELINE:2,5-6`（L1-partial、仅 append、互操作限 append store）、`doc/migration.md:24,228`。
- 测试：`crate/foundation/fkv_config/tests/config.rs:346 validate_local_append_mode_inert` 在盘。
