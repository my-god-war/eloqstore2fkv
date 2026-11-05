# gap-11 两个后端（compio / sim）的 failpoint 名字集合已发散，arm 未命中的名字会静默不生效（P2）

## 来源

对 in-flight gap-03/04/07 实现的复审（2026-11-05）。gap-07 把 `fkv_io_sim` 接进引擎并
在其中埋了注入点，这使 SimStore 与 `fkv_io_compio` 成为两套**并列的**注入面；
本条记录两者的名字集合已经不同步，以及由此引出的静默失效风险。

## HEAD 复核（2026-11-05，fkv HEAD `b5073e1`）

集合差异**未变**，且仓内**没有**任何名字一致性测试：

```
$ rg -o 'fail_point!\(\s*"([^"]+)"|kill_point\(\s*"([^"]+)"|fail_point\(\s*"([^"]+)"' -r '$1$2$3' \
    crate/io/fkv_io_compio/src/lib.rs | sort -u | tr '\n' ' '
AppendManifest:Sync AppendManifest:SyncFail AppendManifest:Write AtomicWriteFile:Rename \
AtomicWriteFile:Sync AtomicWriteFile:SyncDir SyncData SyncDir WriteFile:SyncFail WritePage WriteSegments

$ ... crate/io/fkv_io_sim/src/lib.rs | sort -u | tr '\n' ' '
AppendManifest:Sync AppendManifest:Write AtomicWriteFile:Rename AtomicWriteFile:Sync SyncData \
WriteFile:Rename WriteFile:SyncFail WritePage WriteSegments
```

## 现象

两边的名字集合不同步，见下方「HEAD 复核」的实测；差异与风险如下。

- compio 有、sim 无：`SyncDir`、`AtomicWriteFile:SyncDir`、`AppendManifest:SyncFail`
- sim 有、compio 无：`WriteFile:Rename`

## 问题

`fkv_failpoint::fail_point(name)` / `kill_point(name)` 对**未登记或未命中的名字
返回 `None` / 不动作**（`crate/foundation/fkv_failpoint/src/lib.rs:135-152`），
不报错。于是：

1. 用一个后端的名字集合去驱动另一个后端时，那一侧**静默 no-op**，测试照绿。
   这与 gap-08 是同一族问题（「没跑」被当成「通过」），只不过发生在 failpoint 侧。
2. gap-04 想要的「随机枚举 kill point」若按名字并集枚举，会有一半在某一侧
   永远不触发，且没有任何信号。
3. `WriteFile:Rename` 是 sim 侧新造的名字：compio 的整文件写用的是
   `WriteFile:SyncFail`，没有对应的 rename 注入点。名字一旦成为对外契约
   （fuzz 驱动 / 文档），要么两侧补齐，要么显式记为「仅 sim」。

## 建议

1. 把 failpoint 名字收敛到一个**共享常量表**（放在 `fkv_failpoint` 或
   `fkv_io` 的测试接缝里），两个后端都从常量引用，编译期消除拼写/集合漂移。
2. 增加一条测试：**断言两个后端实现的名字集合相等**（或相等 + 显式白名单记录
   「仅某后端」的项及原因）。把白名单长度固定在测试里，新增/删除会立刻可见。
3. 给 `fail_point` / `kill_point` 增加「严格模式」：当 `FKV_FAILPOINT` 里 arm 的
   名字在整个进程里从未被任何 `fail_point` / `kill_point` 调用点登记时，
   在测试结束时报警（可作为 opt-in，避免污染生产）。
4. 明确 `WriteFile:Rename` 的去留：补齐 compio 侧，或改名为
   `WriteFile:<动作>` 的统一命名并在文档里标注仅 sim。

## 验收

- 存在一条测试，能因「某后端新增了一个别处没有的注入点名」而失败；
- fuzz 驱动枚举名字时，任一后端未命中任何 arm 的名字会**报错**而非静默通过；
- 两边名字集合的差异（若有）在代码里以显式白名单 + 理由出现，而不是隐含在
  两处 `rg` 的差集里。

## 处置（2026-11-05）—— 已实现，且改成编译期消除而非测试比对

原「建议 1/2」设想用**源码扫描 + 集合一致性测试**。落地前按 `rust_review` 规范复核后
否决了该方案：规范第 10 行「严禁盲目使用正则」、第 130 行「严禁编写常量 == 常量本身
之类的废话测试」。改为**让漂移写不出来**：

1. **`fkv_failpoint::points` 是名字的唯一来源**（13 个常量，按前缀分组注释），并提供
   `COMPIO` / `SIM` / `CLOUD` 三个子集清单供枚举式驱动使用。
2. **`fail_point!` / `kill_point!` 宏只接受 `points` 的常量标识符**
   （`$name:ident` → `$crate::points::$name`）。写字符串字面量或未定义标识符直接编译失败，
   因此「某后端新增一个别处没有的点名」不可能悄悄发生。
3. **38 处生产埋点全部改为常量引用**（compio 19 / sim 18 / cloud 1），
   `rg 'fkv_failpoint::(fail_point|kill_point)[!(]\s*"'` 残留 **0**。

顺带消除了发散的一个方向：gap-12 把 `WRITE_FILE_RENAME` 补进 compio 后，
**sim 的名字集合现在是 compio 的真子集**。剩余 3 个 compio-only 名字
（`SYNC_DIR`、`ATOMIC_SYNC_DIR`、`APPEND_SYNC_FAIL`）依赖 sim 掉电模型里不存在的概念
（目录项持久化、manifest 写可失败），属**有意缺失**，理由写在 `points::SIM` 的文档注释里 ——
正是本条目验收第 3 条要求的「显式白名单 + 理由」。

验收对照：

| 验收 | 落实方式 |
|---|---|
| 因「某后端新增别处没有的点名」而失败 | **升级为编译失败**（比测试更强）。反向验证实测：插入 `fkv_failpoint::fail_point!("DoorBell:New")` → `error: no rules expected "DoorBell:New"` |
| 枚举时未命中的名字不许静默通过 | 运行时那一半仍由 `fkv_store/tests/crash_failpoint_invariants.rs` 的 `aborted > 0` 承担（每个 kill 名字必须在 6 条 op 内真的 abort 过） |
| 差异以显式白名单 + 理由出现 | `points::COMPIO` / `points::SIM` 常量 + `SIM` 文档注释 |

验证：`sh/clippy.sh` 零警告且 `--fix` 无改动；`./test.sh` → 1002 passed / 1 skipped；
`-p fkv_failpoint -p fkv_io_sim -p fkv_io_compio -p fkv_store` → 251 passed。
