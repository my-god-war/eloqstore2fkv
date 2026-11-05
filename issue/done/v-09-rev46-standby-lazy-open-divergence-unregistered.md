# v-09 P3：rev-46 的「standby 懒开先追 master」刻意差异未落 COMPAT_BASELINE / persistence 分册，DONE 自认残留且无归口 issue

## 现象（亲读，2026-11-05 v 验证轮）

rev-46（`issue/done/rev-46-standby-lazy-open-skips-sync-standby-partition.md`）
代码 + 测试闭环属实（见本文件末尾证据），但其甄别段「开发计划」第 3 条的
**刻意差异登记**未做，DONE「残留」段已自认：

> `COMPAT_BASELINE` / `doc/architecture/05-persistence-and-recovery.md` 的
> 「fkv standby 懒开先追 master vs C++ 懒 replay」刻意差异登记未做
> （只允许改 fkv_shard / fkv_runtime / fkv_task_reopen / fkv_standby 及测试）。

复核现状（当前盘面）：

- `grep -n "standby\|懒开\|lazy\|FindRoot" COMPAT_BASELINE` **零命中**；82 行
  全文无 rev-46 条目（sto-61b / rev-44 / rev-41 等同类差异均已登记）。
- `grep -rn "懒开" doc/` **零命中**：`05-persistence-and-recovery.md` 无该段，
  `doc/migration.md` 只在 M5 行（:208）提到 rev-38/rev-37 能力，无 rev-46。
- 唯一提及在 `doc/architecture/06-gaps.md:83`「Standby 懒打开先追 master
  （rev-46）」——是能力陈述，未标注「有意超越 C++ 基线」，不构成刻意差异登记。
- 代码注释已就地写明（`fkv_shard/src/lib.rs:1065-1067`「这是**有意超越 C++
  基线**：C++ 懒开路径 `FindRoot` 也是纯本地 replay」），但注释不在兼容基线里。

为何需要登记：rev-46 甄别段「修正二」明确结论——Rust 现状修法**忠实于 C++**
之外是**有意超越基线**，不是对齐修复；`COMPAT_BASELINE` 若缺条目，后续 compat
对齐轮可能以「C++ 也这样」为由回退该修复（甄别段原话）。这与 r8c-02 承接
rev-37 文档残留同型。

## 目标

1. `COMPAT_BASELINE`「范围与排除项」增条目：fkv StandbyReplica 懒开路径
   （`ensure_table_ready` main branch）先 `sync_standby_partition` 追 master，
   C++ 懒开 `FindRoot`（`read_task.cpp` → `PageManager::FindRoot` →
   `IouringMgr::GetManifest`）纯本地 replay；理由=副本重启 / 换主后进程内无
   其它收敛机制，静默读旧 root 窗口持久。附重开条件（若控制面提供周期 catch-up
   则可回退到 C++ 懒 replay 语义）。
2. `doc/architecture/05-persistence-and-recovery.md` 恢复分册补同款说明。
3. （可选）`doc/migration.md` M5 行补 rev-46 引用。

## 验收

- `grep -n "FindRoot\|懒开\|lazy" COMPAT_BASELINE` ≥1 命中，条目含 C++ 对照
  与理由；
- 05-persistence 有对应段落；
- 与 `fkv_shard/src/lib.rs:1062-1070` 现行为、06-gaps:83 表述一致无分叉。

## 证据（v 验证轮亲读）

- 代码：`crate/engine/fkv_shard/src/lib.rs:1068-1070`
  `if branch == MAIN_BRANCH_NAME && state.standby.is_some() {
  sync_standby_partition(state, tbl).await?; }`；
  `sync_standby_partition` :1872。
- 测试：`crate/engine/fkv_runtime/tests/standby.rs:284`
  `replica_lazy_open_syncs_master_before_replaying_local_manifest`、:322
  `replica_lazy_open_after_restart_reads_current_master`、:388
  `replica_lazy_open_missing_master_does_not_create_empty_table`。
- 登记面：`COMPAT_BASELINE` 全文 82 行无条目；`doc/` 无「懒开」；
  `doc/architecture/06-gaps.md:83` 仅能力陈述。
- 查重：`grep -rln "rev-46\|懒开\|FindRoot" issue/` 仅本条出处 rev-46 与
  00-r7 组报告（立项记录），无承接 issue；`issue/new/` 现有 v-01（lane A，
  不同主题）。

## 来源

v 验证轮 lane C（`issue/reviewed/00-v-lane-c.md`），承接 rev-46 DONE 残留。

## 甄别（2026-11-05 v 验证轮，亲读盘面）

**结论：部分采纳（登记为纯文档任务，P3；无未落地代码面）。移入 `issue/todo/`。**

事实复核（全部在盘）：

- 代码属实：`crate/engine/fkv_shard/src/lib.rs:1075-1076`
  `if branch == MAIN_BRANCH_NAME && state.standby.is_some() { sync_standby_partition(state, tbl).await?; }`
  前置于 `store.reopen`；:1069-1072 注释明文「有意超越 C++ 基线」（C++ 懒开
  `FindRoot` 纯本地 replay）；`sync_standby_partition` :1879（显式 Reopen 路径
  :3055/:3068/:3087 亦调用）。代码 + 测试已闭环，**无任何未落地代码面**。
- 登记面确未落：`grep -n "standby\|Standby\|懒开\|lazy\|FindRoot" COMPAT_BASELINE`
  零命中（rc=1，全文 82 行无 rev-46 条目）；`grep -rn "懒开\|FindRoot\|lazy" doc/`
  零命中；`doc/architecture/06-gaps.md:83` 仅列「Standby 懒打开先追 master
  （rev-46）」能力陈述，未标注刻意超越；`doc/migration.md` M5 行（:208）只提
  rev-37/rev-38，无 rev-46。
- 先例写法（`COMPAT_BASELINE:10-33`）：每条 = 「C++ 行为 vs fkv 行为」+ 理由 +
  「判定与重开条件见 issue/...」链接（sto-61b:20-26、rev-44:27-33、rev-41:14-19）。
  本条目应对齐该模板。
- 去重：`grep -rln "rev-46\|懒开\|FindRoot" issue/` 只命中 rev-46 自身、r7 组报告、
  lane-c 报告与 `issue/new/00-index.md:68`（能力行）；`issue/todo/` 现有 eng-91/
  eng-92 无关。无承接 issue，非重复。
- 范围判定：登记只改 `COMPAT_BASELINE` + `doc/architecture/05-persistence-and-recovery.md`
  （可选 `doc/migration.md` M5 行），**不触及代码/测试**，低风险纯文档；rev-46
  的 lane 文件范围限制正是转出本条的原因（同 r8c-02 ↔ rev-37 模式）。

### 开发计划（登记文案与位置）

1. `COMPAT_BASELINE`「范围与排除项」块内新增条目（建议紧随 :27-33 rev-44 之后），
   接入 build 前保持 `fkv_ledger sync` 首两行不动。文案要点：
   - fkv：`StandbyReplica` 懒开路径（`ensure_table_ready` main branch，
     `state.standby.is_some()`）先 `sync_standby_partition` 追 master 再
     `store.reopen`（`fkv_shard/src/lib.rs:1075`）；sync 失败
     `NotFound/ResourceMissing` 沿 park→driver 重试，不装空表。
   - C++：懒开路径同样不 sync（`read_task.cpp` → `PageManager::FindRoot` →
     `IouringMgr::GetManifest`，纯本地 FD+statx replay）；C++ 把懒开新鲜度交给
     控制面 `Reopen`（`eloq_store.h` AttachShard 明文 lazy first open）。
   - 理由：副本重启 / 换主后进程内无其它收敛机制，静默读旧 root 窗口持久
     （非「首次打开时机」窗口）。
   - 性质：**有意超越 C++ 基线**的正确性改进，非对齐修复；后续 compat 对齐轮
     不得以「C++ 也这样」为由回退。
   - 重开条件：若控制面提供周期性 catch-up（进程内 sync 循环），方可回退到
     C++ 懒 replay 语义。
   - 链接：`issue/reviewed/rev-46-standby-lazy-open-skips-sync-standby-partition.md`。
2. `doc/architecture/05-persistence-and-recovery.md` 恢复分册补同款段落，措辞与
   06-gaps:83 能力行、`lib.rs:1069-1072` 注释一致，不产生分叉。
3. （可选）`doc/migration.md` M5 行补 rev-46 引用。

### 验收

- `grep -n "FindRoot\|懒开\|lazy" COMPAT_BASELINE` ≥1 命中，条目含 C++ 对照与理由；
- 05-persistence-and-recovery.md 有对应段落；
- 与 `fkv_shard/src/lib.rs:1069-1076` 现行为、06-gaps:83 表述一致无分叉。


## DONE（2026-11-05）

DONE：rev-46 有意加强已登记 `COMPAT_BASELINE`（StandbyReplica 懒开先追 master）。
