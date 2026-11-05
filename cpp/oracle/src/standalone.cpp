// oracle 只链接 eloqstore 的格式层 TU，但格式层代码仍引用少量运行期符号：
//
//   - `Page(bool alloc)`：从线程局部 `shard->PagePool()` 取页缓冲；
//   - `ManifestBuilder::CalcChecksum`：shard 非空时用 `ThdTask()->YieldToLowPQ()`
//     做协作让渡；
//   - `compression.cpp`：读 `Options()->zstd_compression_level`。
//
// 这里提供最小实现，**不参与任何字节编码**：页 / index / manifest 字节仍由
// eloqstore 真实 builder / parser 产生。oracle 不启动分片，调度让渡是空操作。
#include "standalone.h"

#include <cstddef>

#include "kv_options.h"
#include "storage/page.h"
#include "storage/page_manager.h"
#include "storage/shard.h"
#include "tasks/task.h"

namespace eloqstore
{
namespace
{
// 零初始化的 Shard 存储：Shard::PagePool() 不访问 Shard 成员，只需要一个
// 非空指针让 `Page(bool)` 走正常的分配路径。
alignas(Shard) unsigned char kShardStorage[sizeof(Shard)]{};
}  // namespace

PagesPool *Shard::PagePool()
{
    // KvOptions 默认值（data_page_size=4KiB）与 fixture builder 一致。
    static const KvOptions options;
    static PagesPool pool(&options);
    static const bool ready = (pool.Init(), true);
    (void) ready;
    return &pool;
}

KvTask *ThdTask()
{
    // shard 为 nullptr 时格式层不会走到任务路径；返回 nullptr 只为满足链接。
    return nullptr;
}

void KvTask::YieldToLowPQ()
{
    // 独立 oracle 没有任务队列，无需让渡。
}

// MappingSnapshot 析构在 idx_mgr_ == nullptr 时不会调用这里；链接需要符号
// 定义（oracle 不链接 page_manager.cpp）。真实运行期由 PageManager 回收
// mapping 快照，oracle 构造的快照全部是栈/unique_ptr 生命周期，无需回收。
void PageManager::FreeMappingSnapshot(MappingSnapshot *)
{
}

const KvOptions *Options()
{
    static const KvOptions options;
    return &options;
}

void InitStandalone()
{
    shard = reinterpret_cast<Shard *>(kShardStorage);
}

void ResetStandalone()
{
    shard = nullptr;
}
}  // namespace eloqstore
