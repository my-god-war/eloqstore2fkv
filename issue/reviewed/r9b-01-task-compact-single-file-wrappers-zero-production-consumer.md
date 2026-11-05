# r9b-01 P3：fkv_task_compact 的 `compact_file` / `compact_segment_file` 单文件包装零生产消费（仅本 crate 自测），零死代码纪律残留

## 现象（亲读）

`crate/engine/fkv_task_compact/src/lib.rs:20-29`（`compact_file`）与 `:96-113`
（`compact_segment_file`）是两个单文件 pub 包装，内部都只转发 `*_li` 版本。

全仓引用计数（`grep -rn "fkv_task_compact" crate/ --include="*.rs"` 排除
`target/`、`*_cpp/`，逐条核对）：

- 生产链唯一消费点是 `fkv_store/src/lib.rs:2918`（`compact_file_li`）与
  `:3812`（`compact_segment_file_li`）——**只消费 `_li` 双件套**。
- `fkv_store/src/lib.rs:2888` 的 `Store::compact_file` 是同名 Store 方法（生产
  消费者在 `fkv_shard/src/lib.rs:3013`），它内部走 `compact_file_li`，不经
  `fkv_task_compact::compact_file`。
- `fkv_task_compact::compact_file` / `compact_segment_file` 的全部引用 =
  `fkv_task_compact/tests/compact.rs:7/:55/:98`（本 crate 自测导入）。

两个符号均无「测试 / 诊断用」接缝注释。按 r9a-02 的查重口径（同批扫描排除
了「注释明示测试接缝」的 `fkv_large::decode` 等），本对包装与 `put_fixed64`
完全同型：零生产消费 + 无接缝声明。

讽刺来源：`compact_segment_file` 当初是 eng-60b 的计划产物（其目标 1 写明
「`fkv_task_compact` 增加 `compact_segment_file`」），落地后生产链实际接的是
`compact_segment_file_li`（eng-60b 复审小节 `:42` 亲读确认），单件成了孤儿。

## C++ 对照

`src/tasks/background_write.cpp` 的 `Compact()` / `DoCompactDataFile` /
`DoCompactSegmentFile` 均为内部批式处理（`file_id` 区间循环 + `max_read_pages_batch`
分批），无「单文件公共 API」形态；调用方（`CreateBranch`、写后 `CompactIfNeeded`
经 `AddPendingCompact`）从不按单文件发起。单文件包装是 Rust 侧历史接口残留，
非兼容面。

## 目标

三选一：

1. 删除两个单文件包装，`tests/compact.rs` 改用 `_li` 版本（传单元素切片）；
2. 或在 doc 注释明示「测试 / 诊断用接缝」，与 r9a-02 认可的排除口径对齐；
3. 或给单件接上生产消费（如 `Store::compact_file` 改为直通
   `fkv_task_compact::compact_file`，消除绕行）。

## 验收

- `grep -rn "fkv_task_compact::compact_file\b\|fkv_task_compact::compact_segment_file\b" crate/`
  的命中要么消失（删除路径），要么定义处带测试接缝注释；
- `cargo nextest run -p fkv_task_compact -p fkv_store` 不回归；
- `./sh/clippy.sh` 零警告。

## 核验记录

- 亲读 `fkv_task_compact/src/lib.rs` 全文（214 行）与 `tests/compact.rs`；
- 全仓引用计数核对：`fkv_store/src/lib.rs:2888`（Store 单件，生产链有主）与
  `fkv_task_compact` 两个孤儿符号区分开，无误伤；
- 对照 r9a-02（put_fixed64）的立项 / 排除口径；r8a-01（死 pub 常量）同族先例；
- 查重：r9a-01/02/03、r9c-01、r9e-01/02/03 均不涉本条；eng-60b 复审小节
  记录的 `compact_segment_file_li` 在位事实与本条（单件孤儿）不重叠。

> 认领（2026-11-05 07:06，本会话子代理开发中）。lane：crate/engine/fkv_task_compact。

## DONE（2026-11-05）

走目标 1（删除路径）。甄别复核：全仓 grep（`crate/ tool/ web/`，含 tests，排除
`target/`）确认 `fkv_task_compact::compact_file` / `compact_segment_file` 仅剩定义本体与
`tests/compact.rs` 三处自测消费；生产链只接 `_li` 双件套（`fkv_store/src/lib.rs:2945/:3839`），
`Store::compact_file`（:2915）内部走 `compact_file_li`，与自由函数无关；doc/README 零引用。
两符号均无测试接缝注释，不构成「有意测试接缝」，按零死代码纪律（对齐 r8a-01 删
`DirectIoBuf` 先例）直接删除两个单件包装；`tests/compact.rs` 改导入并调用 `_li` 版本
（传单元素切片 `&[0]`），回滚用例 doc 同步改为 `compact_file_li`。

验收：`grep -rn "fkv_task_compact::compact_file\b|fkv_task_compact::compact_segment_file\b" crate/`
0 命中；`cargo nextest run -p fkv_task_compact` 3/3 绿；
`cargo clippy -p fkv_task_compact --all-features --all-targets -- -D warnings` EXIT=0 零警告
（注入坏代码阴性对照确认 lint 真实生效；仓库 `.cargo/config.toml` 设 `[term] quiet = true`，
干净跑无输出属正常）；`cargo check --workspace` EXIT=0 零警告，其它 crate 未破坏。
附带核实：依赖 crate `fkv_types` 在 `-W clippy::absolute_paths` 下有 2 处既有报错，与本次
改动无关（仓库 `./sh/clippy.sh` 先跑 `fixrs` 自动修复），未越 lane 处理。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`fkv_task_compact/src/lib.rs` 仅剩 `compact_file_li`(:25) / `compact_segment_file_li`(:88) 及各自 `_inner`，两个单件 pub 包装已删。
- 验收：`grep -rn "fkv_task_compact::compact_file\|fkv_task_compact::compact_segment_file" crate` 0 命中；`fkv_task_compact/tests/compact.rs:7` 改导入 `_li`，`:55/:98/:139` 传 `&[0]` 调用；生产链 `fkv_store/src/lib.rs:2976`/`:3891` 只接 `_li` 双件套。`grep pub use` 于 fkv_buf/fkv_task_compact 无；doc/ 无引用。删除连带清理到位（SKILL 12）。
- 判定：完善。
