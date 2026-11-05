# gap-03 V3「同一 op log 双实现差分」未系统化（P2，可部分本机推进）

## 来源

dragon 的剩余工作盘点；`doc/migration.md` §5 自述「V3 / V6 未系统化」。

## 证据（2026-11-05 实测）

- `doc/migration.md` §5「当前实际覆盖」：V3 / V6 **未系统化**，注明
  「无同一 op log 的 Rust/C++ 操作差分」。
- 现有差分能力只有**格式层**：`fkv_page` / `fkv_manifest` / `fkv_compress` 的
  `cpp_oracle` 测试 + `fkv_ledger verify`（输入是固定语料，不是 op log）。
- 现有 **Rust 侧**可确定性重放的设施已具备：`fkv_io_sim` 提供
  `crash/restart`、`with_delay/with_fault/with_reorder_seed` 与「完整操作日志
  （含读 / 写 / flush / crash）」（见 `fkv_io_sim/src/lib.rs:388-409`）。

## 问题

格式层相等 ≠ 操作层相等。缺 V3 时，以下分歧**无法被发现**：TTL 过期判定、删除与
compaction 的可见性、rebalance 切分点、GC 保留集合、reopen 重放后的映射重建等
（这些正是本项目历史上反复出现真实 bug 的地方：`sto-65` / `sto-66` / `core-27` /
`core-29`）。

## 目标

1. 定义 **op log 格式**（一行一 op：`put/delete/scan/floor/truncate/reopen/gc/compact`，
   带时间戳与参数），作为 Rust / C++ 两侧共同的输入。
2. Rust 侧先跑通「op log → 引擎 → 结果快照」的**确定性驱动 + dump**（可用
   `fkv_io_sim` 的确定性后端），产物为规范化文本（键值遍历 + 文件/页布局摘要）。
3. C++ 侧补同格式的 dump（可复用 `db_stress`/`simple_bench` 的操作序列，或写一个
   最小 op-log 驱动器），两侧 dump 做 diff。
4. 至少覆盖 5 个 op log：纯写、写删混合（含旧 ts 删除）、TTL 到期、truncate 区间、
   归档 + 分支。

## 验收

- `cargo nextest` 里有一条「op log 回放 → dump」的 Rust 用例（确定性，无时序依赖）；
- 存在一份 op log 清单与两侧 dump 的 diff 脚本；本机无 C++ 时脚本报
  「缺 C++ 产物」而不是静默通过；
- 任意一条 op log 的 dump 在 Rust 侧连续两次运行逐字节一致（确定性前提）。

## 阻塞 / 依赖

- **Rust 半侧本机可做**（第 1、2、4 条的全部与第 3 条的脚本骨架）。
- **C++ 半侧**与 `gap-02` 同一硬前置：4 KiB 页 Linux + eloqstore 构建环境。

## 核验记录

- 实测：`doc/migration.md` §5；`fkv_io_sim` 的 crash/restart 与操作日志接口。

## 复审（2026-11-05）

原 index 把它归为“验证债务”不成立：`OpRecord` 不含 payload、无序列化、
无 replay driver，缺的是可重放 op-log 能力本身。另见 gap-07。
