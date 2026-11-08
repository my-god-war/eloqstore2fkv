# sem-71 删除 index 逻辑页缓存后，多处注释仍把 `index cache` / `IndexNode` / 慢读探针当活跃组件描述

- 领域：语义 / 文档一致性（并行会话 `refactor(cache)` 遗留）
- 优先级：P3（纯注释，无运行时行为；但会误导后续读语义者以为 index 缓存仍在）
- 置信度：高（逐行 grep 实测；无生产代码引用，全部为注释 / doc）

## 现象

`f6f08c8`（`52650d6`）把 `IndexCache` / `IndexNode` / `CacheLedger` 与
`fkv_types::slow_read` 探针整套删除，收敛为单一 `PageCache`，但下列注释未同步：

1. `crate/foundation/fkv_config/src/lib.rs:18-19`
   —— `enable_data_page_cache` / `init_page_count` 条目仍写「Rust 读路径直接走
   **index cache** + page buffer，没有对应缓存层」；现读路径走 `PageCache`。
2. `crate/foundation/fkv_config/src/lib.rs:130`
   —— root meta 预算合计里仍列 `index cache`（已不存在）。
3. `crate/engine/fkv_store/src/state.rs:22`
   —— `evict_table` 文档仍列 `index cache`。
4. `crate/engine/fkv_store/src/state.rs:242`
   —— `consume_changed_pages` 文档「未变更子树的 **`IndexNode`** 保持有效」，
   该类型已删；同条目 `state.rs:350-351` 另有一行重复（同一句 doc 出现两遍）。
5. `crate/engine/fkv_store/src/lib.rs:313`
   —— 代际来源文档「快照 / **index cache** 均为 per-Store」。
6. `crate/engine/fkv_store/src/read.rs:96,232,313,357,539,652,704`
   —— 7 处「**慢读探针** / 独享**采样槽**」注释；`slow_read` 探针与该「采样槽」
   概念已随重构删除（`get_at` 只是普通入口）。
7. `crate/engine/fkv_task_write/src/batch.rs:668`
   —— 「与读路径 **`IndexNode::seek`** 的 `seek_floor` 口径一致」；`IndexNode`
   已删，读路径现为 `IndexPageIter::seek_floor`（`fkv_task_read/src/lib.rs:267`）。

## 影响

纯文档漂移，无正确性 / 盘面影响。风险在于后续审查者按旧注释去 `fkv_btree` /
`fkv_task_read` 找 `IndexCache` / `IndexNode` 的失效协同，落空且误判「缓存语义
缺失」（本轮语义域已核对：收敛后 index / data 同池、同一条物理页失效链，语义完整）。

## 建议

按现结构改写上述注释：`index cache` → `page cache`；删除 `IndexNode` 指代，
改指 `IndexPageIter`；「慢读探针 / 采样槽」整句删除；`state.rs:350-351` 去重。

## 验收方式

```bash
git grep -nE 'IndexNode|index cache|慢读探针|采样槽|IndexCache|CacheLedger|slow_read' -- '*.rs'
# 期望：仅剩本条目自身 / 历史注释外，生产与测试源码零命中
```

## 处置记录

- 2026-11-08 第 35 轮语义域发现（审查并行会话 `f6f08c8` 收敛提交的连带文档面）。
</content>

## 实现记录（2026-11-08）

- 修复 fkv commit **`4abc60c`**（22 文件、+47/-49，纯注释/文档）；范围按第 36
  轮结构域补全后的清单执行（含其发现的三处遗漏与 33 处中文等价术语）：
  - 英文 `index cache` → `page cache`（`fkv_config` ×2、`fkv_store/src/{lib,state}.rs`、
    2 个测试文件）；`IndexNode` → `IndexPageIter` / 索引页字节（`state.rs:242`、
    `batch.rs:668`）；
  - `read.rs` 7 处「慢读探针 / 独占采样槽」注释重写为「块内取一次快照」口径；
  - `page_cache.rs:79` 的 `fkv_btree::CacheStat`（该类型已删）改为「与 bench 指标
    汇总口径同形」；`state.rs:350-351` 重复 doc 行去重；
  - 中文等价术语 `index 缓存` ×18 / `索引缓存` ×15 → `页缓存`（16 文件；
    **保留** `fkv_shard/src/roots.rs:157` 的「路径索引缓存」= IO 层 fd 缓存）；
  - 机械替换遗留的 CJK 空格瑕疵 14 处一并清理（含义未变）。
- 验证：`fmt` 双通道 exit 0；clippy（含 absolute_paths）exit 0；受影响 4 crate
  **600/600 passed**。全仓残留 grep（排除「路径索引缓存」）**0 命中**。

## 复审记录（2026-11-08，第 37 轮结构域）

- 判定：**达标**，移入 reviewed/。抽检 8 处改写与代码行为逐条相符（read.rs 7 处
  函数均在 async 块顶取一次快照；`batch.rs:668` 指向实存的 `IndexPageIter::seek_floor`；
  `page_cache.rs:79` 的已删类型去除；`roots.rs:157` 路径缓存未动）；残留 grep
  （`IndexNode|index cache|慢读探针|采样槽|IndexCache|CacheLedger|slow_read` +
  中文术语，排除「路径索引缓存」）**0 命中**。唯一瑕疵为
  `fkv_mapping/src/generation.rs:18-19` 括注自指冗余（P4 文风，未立项）。
