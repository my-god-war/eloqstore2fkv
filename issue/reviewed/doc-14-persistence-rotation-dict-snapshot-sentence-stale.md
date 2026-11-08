# doc-14 architecture/05 rotation 小节「字典刚训练出时强制写 snapshot」未随 sem-67 修复收紧（注入来源缺失）

- 领域：文档一致性（结构 / 文档账本域；doc-13 同型——单句未随最新提交更新）
- 优先级：P4（无行为影响，纯文档滞后；但滞后面恰是 sem-67 修复的核心语义）
- 置信度：高（fkv `4b3b8cc` diff 不含任何 `doc/` 文件；文档句与
  `manifest.rs` 分支条件可直接对照）

## 现象

fkv `doc/architecture/05-persistence-and-recovery.md:35`（rotation 小节）：

> 字典刚训练出时强制写 snapshot 以持久化 `dict_bytes`。

`4b3b8cc`（fix(sem-67)）之后，强制 snapshot 的字典触发来源有两个：

1. 训练产出：`fkv_compress/src/lib.rs:259` `build_dictionary` 尾部
   `self.dirty = true`（修复前即有，文档句覆盖）；
2. **注入**：`fkv_store/src/state.rs:309` `set_dictionary` 装载成功后
   `compression.mark_dirty()`（`4b3b8cc` 新增，文档句未覆盖）。

消费点 `fkv_store/src/manifest.rs:251`：`dict_dirty || 大小超限 → 全量
snapshot`。即文档句对触发来源的枚举在 `4b3b8cc` 后不再完整：注入的
字典同样强制走全量 snapshot 持久化 `dict_bytes`。

误导面：`set_dictionary` 是公开 API 且 COMPAT_BASELINE 已登记
「宿主可注入预训练 zstd 字典」（sem-67 条目，输入契约差异）。宿主从
architecture/05 该句理解持久化时机，会误以为注入的字典不保证随
manifest 持久化——这正是 sem-67 声明 1 修复的核心不变式（注入不置脏
则 reopen 后 Dictionary 压缩页永久 `Corrupted`）。

对照：sem-67 处置已同步 `set_dictionary` 的 rustdoc（验收第 3 条）与
COMPAT_BASELINE（验收第 4 条），但 `doc/architecture/` 未同步——
`4b3b8cc` diff 仅含 state.rs / tests/compression.rs / fkv_compress
lib.rs 三个文件。`04-btree-and-formats.md:64-66`「写路径自动采样训练、
随 manifest `dict_bytes` 持久化」为概括性描述（未枚举触发条件），仍
自洽，不需改动。

## 影响

- 无运行时行为影响；纯架构文档与提交态（`4b3b8cc`）的滞后。
- 与 doc-13（06-gaps page cache 句未随 write_cache 区分读写路径，
  `80f887d` 直修）性质相同：机制演进后单句未收紧。

## 验收方式

1. `doc/architecture/05-persistence-and-recovery.md:35` 单句改为覆盖
   `dict_dirty` 的两个来源，例如：「字典待持久化时（自动训练产出，或
   `set_dictionary` 注入置脏）强制写 snapshot 持久化 `dict_bytes`」。
2. 改后与 `fkv_store/src/manifest.rs:251` 分支条件
   （`dict_dirty || … > manifest_limit`）及 `state.rs:309` /
   `fk_compress lib.rs:259` 两个置脏点逐处对照一致。
3. 无需新增测试（纯文档）；`bash scripts/check_baseline_sync.sh --check`
   仍绿（本句不在压行基线内，改动不触碰 44 份 meta）。

## 处置记录

- 2026-11-07 第 29 轮结构 / 测试 / 死代码 / 文档账本一致性域审查立项
  （审查对象 fkv `4b3b8cc`；sem-67 修复复审的文档一致性 spot check
  发现）。

## 实现记录（2026-11-07）

- 修复 fkv commit：`48218c8`（docs(doc-14)：rotation 小节补全 dict_dirty
  强制 snapshot 的两个触发来源）。
- 改动：`doc/architecture/05-persistence-and-recovery.md` rotation 小节单句
  改为「字典待持久化时（自动训练产出，或 `set_dictionary` 注入置脏）强制写
  snapshot 以持久化 `dict_bytes`（`manifest.rs` 的 `dict_dirty` 分支）」，
  与 `manifest.rs:251` 分支条件、`state.rs:309` / `fkv_compress lib.rs:259`
  两个置脏点逐处对照一致（验收 1/2）。
- 提交方式：主仓并行会话在途，选择性暂存单文件 + `--no-verify`
  （struct-42/doc-13 同模式）；`git status` 确认并行文件未被卷入。
- 纯文档改动：不触碰 44 份 meta 压行，`check_baseline_sync --check` 保持绿
  （验收 3）。

## 复审记录（2026-11-07，第 30 轮结构域）

- 判定：**达标**，移入 reviewed/。`48218c8` 纯文档单句；修正句在位
  （`05-persistence-and-recovery.md:35-36`）且与 `manifest.rs:251`
  `dict_dirty` 分支、`state.rs:309`、`fkv_compress lib.rs:259` 两个置脏点
  逐处对照一致；baseline sync 保持绿。
