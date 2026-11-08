# sem-61-compact-file-order-and-record-split

> **存档桩**：本文件在 2026-11-07 仓库 re-init（issue 仓 init `4759859`）中丢失，
> 原始甄别 / 实现 / 复审记录不可恢复；本桩仅恢复账本引用完整性，不复原内容。
>
> 存活证据：结论与登记见 `COMPAT_BASELINE` 引用本文件的条目（含行为锁定测试名）；
> 轮次结论见 `issue/new/00-index.md` 对应轮记录（若有）。

## 00-index.md 中的轮次记录（摘）

- **第 21 轮迭代（2026-11-07，四域并行收敛判定轮：语义 / 异步 / 结构 / 性能）**：| [sem-61](sem-61-compact-file-order-and-record-split.md) | compact 文件搬迁顺序放大率降序 vs C++ 文件号升序（多文件超阈时新 `FilePageId` 发号互换）；data/segment 各写一条 record vs C++ 单 `UpdateMeta` 一条同携两侧 delta | 语义 / manifest 记录流 | P3 |

