#pragma once

namespace eloqstore
{
// 让格式层代码在无 shard 的独立进程里可运行：临时给线程局部 shard 一个非空
// 指针，使 `Page(true)` 能从独立 PagesPool 分配缓冲。
//
// 只在构造 Page 前后成对调用：shard 非空时 ManifestBuilder::CalcChecksum 会
// 调用 ThdTask()->YieldToLowPQ()，而 oracle 没有任务系统。恢复为 nullptr 后
// `can_yield=false`，格式代码不会进入任务路径。
void InitStandalone();
void ResetStandalone();
}  // namespace eloqstore
