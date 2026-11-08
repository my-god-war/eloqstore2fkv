# sem-60-cloud-branch-manifest-name-term

> **存档桩**：本文件在 2026-11-07 仓库 re-init（issue 仓 init `4759859`）中丢失，
> 原始甄别 / 实现 / 复审记录不可恢复；本桩仅恢复账本引用完整性，不复原内容。
>
> 存活证据：结论与登记见 `COMPAT_BASELINE` 引用本文件的条目（含行为锁定测试名）；
> 轮次结论见 `issue/new/00-index.md` 对应轮记录（若有）。

## 00-index.md 中的轮次记录（摘）

- **第 21 轮迭代（2026-11-07，四域并行收敛判定轮：语义 / 异步 / 结构 / 性能）**：| [sem-60](sem-60-cloud-branch-manifest-name-term.md) | Cloud 非 main branch 的 manifest 对象名恒 `_0`：C++ attach 后按 `ProcessTerm()` 晋升 `manifest_B_T` 并续写，fkv 恒 `manifest_B_0`（具名读无最高 term 回退 / 无栅栏；branch 模式远端剪枝永不发生；混合接手 C++→fkv 读不到对方写入） | 语义 / Cloud+branch | P2 |

