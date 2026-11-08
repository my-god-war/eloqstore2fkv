# sem-62-put-large-subthreshold-inline

> **存档桩**：本文件在 2026-11-07 仓库 re-init（issue 仓 init `4759859`）中丢失，
> 原始甄别 / 实现 / 复审记录不可恢复；本桩仅恢复账本引用完整性，不复原内容。
>
> 存活证据：结论与登记见 `COMPAT_BASELINE` 引用本文件的条目（含行为锁定测试名）；
> 轮次结论见 `issue/new/00-index.md` 对应轮记录（若有）。

## 00-index.md 中的轮次记录（摘）

- **第 21 轮流转（2026-11-07，run.md 流程：7 条全部闭环）**：| sem-62-put-large-subthreshold-inline（并行立） | 登记 | `981cc55` | C++ 由宿主构造决定（引擎无门槛）vs fkv put_large 阈值 `page_size/2`：输入契约差异（sem-01 同型），基线条目 + 跳表行 + 锁定测试（含 `>=` 边界 + segment 槽计数）；变异（改恒真）红于 `LargeValueUnsupported`；全量 1507 |

