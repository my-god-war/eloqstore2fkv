# eng-70 页打开常数成本：validate_order 每次全量校验 restart 升序 + IndexCache miss 全页物化 N 次堆分配（C++ 原始字节常驻 + 惰性二分）

## 位置
- Rust:
  - `crate/format/fkv_page/src/restart.rs:43-57`（`validate_order` 遍历全部 restart 点解码比较）+ `data.rs:316`、`index.rs:234`（`DataPageIter::new`/`IndexPageIter::new` 无条件调用）。热路径：点读/扫描（`fkv_task_read/src/lib.rs:127,257`）、index 下降（`fkv_btree/src/lib.rs:76,97`）、写路径读 index（`fkv_task_write/src/lib.rs:1027`）
  - `crate/core/fkv_btree/src/lib.rs:94-102`（`IndexNode::new` miss 时整页全量解码 `(key.to_vec(), page_id)` 每条一次堆分配）；`:116-122`（`byte_len` 低估）
- C++: `src/storage/data_page.cpp:146-157`（`DataPageIter` 构造仅算 `restart_num_/restart_offset_`）；`src/storage/mem_cached_page.cpp:108-140`（`IndexPageIter` 持 `std::string_view page_`，Seek = restart 二分 + 区间线性惰性解码）；`page_manager.cpp:663-729`（FindPage 命中返回 `MemCachedPage`，`mem_cached_page.h:222` 存原始 `Page page_`）

## 现状
1. 每次页打开多 O(restart_num) 次 varint 头解析 + 比较（4KB 页、interval=16 约 15~30 次）；一次点读下降 3~5 层 index + 1 data 页全付；
2. IndexCache miss 全页解码 + N 次小分配；命中常驻的是物化节点（16B key 下每 key 约 32B 结构 + 分配开销，内存可达原始页字节 2-3×）。

## C++ 实现
页打开零校验直接二分；缓存常驻原始页字节，seek 惰性解码仅 log + 区间个 entry。

## 差异与风险
Rust 的校验是有意加固（畸形页显式 Corrupted，且比 C++ 严格 —— C++ 成环会挂死、错型静默误解析），语义面 fmt 系列已认可；本条只记**成本面**。CPU 常数项 + 缓存内存放大。

## 对齐建议
- 校验降级方案任选：`debug_assert!` / 仅 checksum 失败或首次加载时执行 / 推迟到二分实际触达该 restart 时校验；
- IndexCache 中期方向：IndexNode 持原始页 + 惰性 seek（复用 `IndexPageIter`），`byte_len` 计入分配开销。

## 严重度
P2（读路径 CPU 常数 + 缓存内存模型）

## 核验记录（2026-11-04）
`validate_order` 全量遍历与无条件调用点、`IndexNode::new` 全页物化实读确认；C++ 惰性解码引用自子代理摘录。注意保留 Rust 防御优于 C++ 的部分（见 00-index 附录「有意偏离」）。

## 审查结论（2026-11-04）

逐条核验：

- **行号**：`data.rs:316`（`DataPageIter::new` 调 `validate_order`）、`index.rs:234`（`IndexPageIter::new` 同）实读一致；`fkv_btree/src/lib.rs:94-102` 全页物化一致；`byte_len` 实际在 `:111-118`（原写 116-122，轻微漂移）。`fkv_btree/src/lib.rs:76` 是 `seek_index` 内的 restart 二分，不是独立「下降」点。C++ 行号本仓库无法复核。
- **子问题 1（每次页打开全量 `validate_order`）：拒绝降级。**
  - 成本被高估：`restart_key` 返回页内切片（零分配），单次是 2 个 varint 头解析；index/data 的 `restart_interval` 默认 16，4KB 页 restart 数约 3~13，不是 15~30；页读默认已过 `validate_checksum`（`fkv_io_compio` 读路径）。
  - `debug_assert` / 仅 checksum 失败时校验 / 二分触达时才校验，都会让**线性扫描与 merge 路径**失去显式 `Corrupted`（`advance` 不依赖 restart 顺序，故不会自然发现畸形），与 fmt 系列已认可的加固语义冲突。
  - 收益在页 IO / 解压面前可忽略。**不做**，保留无条件校验。
- **子问题 2（`IndexNode` 全页物化）：成立**，按 C++ 常驻原始页 + 惰性 seek 重构 → `eng-70a`。
- **附带发现**：`fkv_task_read::descend` cache miss 时构造两次 `IndexNode`（`:100` seek、`:102` 插缓存），全页解码/校验双付 → `eng-70b`。
- 与 eng-69 关系：eng-69 负责「byte_len 计分配粒度 + 预算模型」，本条只改 `IndexNode` 表示的必然配套（计页字节），不重复其预算语义。

## 复审（2026-11-05 r6）

通过（父条目）。子问题 2（IndexNode 物化）由 eng-70a 落地、eng-70b 由 eng-86 的
`descend_step` 重构吸收，均已按当前源码复核。子问题 1（validate_order 降级）按审查结论
拒绝、保留无条件校验——当前 `fkv_page` 的 `IndexPageIter::new`/`DataPageIter::new` 仍
无条件调用（见 eng-70a r6 复核的 `new_prevalidated` 拆分），与结论一致。
