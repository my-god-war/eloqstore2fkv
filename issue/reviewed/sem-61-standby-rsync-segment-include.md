# sem-61-standby-rsync-segment-include

> **存档桩**：本文件在 2026-11-07 仓库 re-init（issue 仓 init `4759859`）中丢失，
> 原始甄别 / 实现 / 复审记录不可恢复；本桩仅恢复账本引用完整性，不复原内容。
>
> 存活证据：结论与登记见 `COMPAT_BASELINE` 引用本文件的条目（含行为锁定测试名）；
> 轮次结论见 `issue/new/00-index.md` 对应轮记录（若有）。

## 00-index.md 中的轮次记录（摘）

- **第 21 轮流转（2026-11-07，run.md 流程：7 条全部闭环）**：| sem-61-standby-rsync-segment-include（并行立） | 登记 | `13de69e` | C++ 只同步 data_*（replica GetLarge 不可用）vs fkv 放行 segment_* + `--checksum`/`--compare-dest`：Rust 单侧加强，基线条目 + l2-03 跳表行（完整文件名引用）+ 锁定测试 `replica_catches_up_segment_files_and_serves_get_large`；全量 1506（并行波动） |

