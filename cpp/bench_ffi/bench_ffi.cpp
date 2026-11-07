// C ABI shim for the C++ EloqStore redb-style benchmark matrix.
//
// The Rust matrix driver in crate/perf/fkv_bench_matrix dlopens this library
// and calls one phase at a time on a handle that owns a live EloqStore.  The
// store stays open until fkv_bench_close so the caller can measure on-disk
// bytes between phases.
//
// This file deliberately keeps all EloqStore types out of the public header:
// the ABI is plain C (uint64_t / double / char *) so the Rust side never has
// to link the C++ runtime or EloqStore headers.

#include "bench_ffi.h"

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <limits>
#include <memory>
#include <random>
#include <string>
#include <string_view>
#include <thread>
#include <utility>
#include <vector>

#include "eloq_store.h"
#include "kv_options.h"

// The opaque handle.  Kept in the global namespace so it matches the
// `typedef struct FkvBenchFfi FkvBenchFfi` in the public header.
struct FkvBenchFfi
{
    std::string root;
    uint32_t rows{0};
    uint32_t bulk_rows{0};
    uint32_t batch_size{0};
    uint32_t kv_size{0};
    uint32_t partitions{0};
    uint32_t key_size{0};
    uint32_t num_threads{1};
    uint32_t individual_writes{0};
    uint32_t batch_writes{0};
    uint32_t removals{0};
    uint32_t reads{0};
    uint32_t range_scans{0};
    uint32_t range_len{0};
    uint64_t last_timestamp{0};
    std::string value;
    eloqstore::TableIdent table{"bm", 0};
    std::unique_ptr<eloqstore::EloqStore> store;
    bool loaded{false};
    std::string last_error;

    // Writes need a strictly increasing timestamp per batch.  Wall-clock
    // microseconds keep values comparable with other EloqStore clients; the
    // local counter guards against clock resolution / clock rollback.
    uint64_t NextTimestamp();
};

namespace
{

using Clock = std::chrono::steady_clock;

constexpr char kTableName[] = "bm";
/// Key 编码尾部固定 8 字节大端 uint64；`key_size` 之前的字节补零。
constexpr size_t kKeyIndexBytes = sizeof(uint64_t);
constexpr size_t kScanPageEntries = 256;

// open()/run(NULL, ...) failures have no handle to attach the message to.
// They are reported through fkv_bench_last_error(NULL) on the same thread.
thread_local std::string tls_open_error;

double ElapsedSecs(Clock::time_point start)
{
    const double secs =
        std::chrono::duration<double>(Clock::now() - start).count();
    // The Rust adapter rejects non-positive durations; steady_clock resolution
    // is normally fine, but clamp empty phases defensively.
    return secs > 0.0 ? secs : std::numeric_limits<double>::min();
}

uint64_t WallMicros()
{
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::system_clock::now().time_since_epoch())
            .count());
}

void EncodeKey(char *dst, size_t key_size, uint64_t value)
{
    const size_t offset = key_size - kKeyIndexBytes;
    for (int i = static_cast<int>(kKeyIndexBytes) - 1; i >= 0; --i)
    {
        dst[offset + static_cast<size_t>(i)] =
            static_cast<char>(value & 0xff);
        value >>= 8;
    }
}

std::string KeyOf(const FkvBenchFfi *h, uint64_t value)
{
    std::string key(h->key_size, '\0');
    EncodeKey(key.data(), key.size(), value);
    return key;
}

std::string KvErrorText(const char *what, eloqstore::KvError err)
{
    return std::string(what) + ": " + eloqstore::ErrorString(err) +
           " (KvError=" + std::to_string(static_cast<int>(err)) + ")";
}

int Fail(FkvBenchFfi *h, std::string message)
{
    if (h != nullptr)
    {
        h->last_error = std::move(message);
    }
    else
    {
        tls_open_error = std::move(message);
    }
    return -1;
}

std::vector<eloqstore::WriteDataEntry> MakeEntries(
    const FkvBenchFfi *h,
    uint64_t first_key,
    uint64_t count,
    eloqstore::WriteOp op,
    uint64_t timestamp)
{
    std::vector<eloqstore::WriteDataEntry> entries;
    entries.reserve(static_cast<size_t>(count));
    for (uint64_t i = 0; i < count; ++i)
    {
        entries.emplace_back(
            KeyOf(h, first_key + i), h->value, timestamp, op);
    }
    return entries;
}

bool ExecBatch(FkvBenchFfi *h,
               std::vector<eloqstore::WriteDataEntry> &&entries,
               const char *what)
{
    eloqstore::BatchWriteRequest request;
    request.SetArgs(h->table, std::move(entries));
    h->store->ExecSync(&request);
    if (request.Error() != eloqstore::KvError::NoError)
    {
        h->last_error = KvErrorText(what, request.Error());
        return false;
    }
    return true;
}

/// Upsert `keys` entries in batches.  `secs` (optional) receives the wall time.
bool BulkLoad(FkvBenchFfi *h, uint64_t keys, double *secs)
{
    const auto start = Clock::now();
    for (uint64_t offset = 0; offset < keys; offset += h->batch_size)
    {
        const uint64_t count =
            std::min<uint64_t>(h->batch_size, keys - offset);
        if (!ExecBatch(h,
                       MakeEntries(h,
                                   offset,
                                   count,
                                   eloqstore::WriteOp::Upsert,
                                   h->NextTimestamp()),
                       "bulk_load batch write"))
        {
            return false;
        }
    }
    if (secs != nullptr)
    {
        *secs = ElapsedSecs(start);
    }
    h->loaded = true;
    return true;
}

// Read/scan/delete/len/compact phases prepare their data with an untimed load.
bool EnsureLoaded(FkvBenchFfi *h)
{
    if (h->loaded)
    {
        return true;
    }
    return BulkLoad(h, h->rows, nullptr);
}

// Nearest-rank percentile: rank = ceil(p * n), 1-based; 0 for an empty sample.
double Percentile(std::vector<double> &latencies, double p)
{
    if (latencies.empty())
    {
        return 0.0;
    }
    size_t rank = static_cast<size_t>(std::ceil(p * latencies.size()));
    rank = std::clamp<size_t>(rank, 1, latencies.size());
    const size_t index = rank - 1;
    std::nth_element(latencies.begin(),
                     latencies.begin() + static_cast<ptrdiff_t>(index),
                     latencies.end());
    return latencies[index];
}

int RunBulkLoad(FkvBenchFfi *h, FkvBenchResult *out)
{
    double secs = 0.0;
    const uint64_t keys = h->bulk_rows > 0 ? h->bulk_rows : h->rows;
    if (!BulkLoad(h, keys, &secs))
    {
        return -1;
    }
    out->ops = keys;
    out->secs = secs;
    out->p99_us = 0.0;
    return 0;
}

int RunIndividualWrites(FkvBenchFfi *h, FkvBenchResult *out)
{
    const uint64_t writes =
        h->individual_writes > 0 ? h->individual_writes : h->rows;
    const auto start = Clock::now();
    // Keys wrap over the dataset universe, matching `fkv_bench`'s
    // `--write-batchs` overwrite semantics.
    for (uint64_t i = 0; i < writes; ++i)
    {
        const uint64_t key = i % h->rows;
        if (!ExecBatch(h,
                       MakeEntries(h,
                                   key,
                                   1,
                                   eloqstore::WriteOp::Upsert,
                                   h->NextTimestamp()),
                       "individual_writes batch write"))
        {
            return -1;
        }
    }
    h->loaded = true;
    out->ops = writes;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

int RunBatchWrites(FkvBenchFfi *h, FkvBenchResult *out)
{
    const uint64_t batches =
        h->batch_writes > 0 ? h->batch_writes : h->rows / h->batch_size;
    const auto start = Clock::now();
    for (uint64_t batch = 0; batch < batches; ++batch)
    {
        const uint64_t first =
            (batch * h->batch_size) % static_cast<uint64_t>(h->rows);
        if (!ExecBatch(h,
                       MakeEntries(h,
                                   first,
                                   h->batch_size,
                                   eloqstore::WriteOp::Upsert,
                                   h->NextTimestamp()),
                       "batch_writes batch write"))
        {
            return -1;
        }
    }
    // A floor batch count leaves a remainder unloaded.  Only claim the handle
    // is fully loaded when the phase actually covered every key.
    if (h->batch_writes == 0 && batches * h->batch_size == h->rows)
    {
        h->loaded = true;
    }
    out->ops = batches * h->batch_size;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

struct ReadThreadState
{
    uint64_t reads{0};
    uint64_t completed{0};
    std::string error;
    std::vector<double> latencies;
};

int RunRandomReads(FkvBenchFfi *h, uint32_t readers, FkvBenchResult *out)
{
    if (readers == 0)
    {
        return Fail(h, "random_reads: readers must be greater than zero");
    }
    if (!EnsureLoaded(h))
    {
        return -1;
    }

    // redb shape: a fixed total read count shared across the reader threads.
    const uint64_t total_reads = h->reads > 0 ? h->reads : h->rows;
    // More reader threads than reads would only create idle threads.
    const size_t thread_count =
        std::min<size_t>(readers, static_cast<size_t>(total_reads));
    const uint64_t base = total_reads / thread_count;
    const uint64_t remainder = total_reads % thread_count;

    std::vector<ReadThreadState> states(thread_count);
    const auto start = Clock::now();
    std::vector<std::thread> threads;
    threads.reserve(thread_count);
    try
    {
        for (size_t thread_id = 0; thread_id < thread_count; ++thread_id)
        {
            states[thread_id].reads = base + (thread_id < remainder ? 1 : 0);
            threads.emplace_back(
                [h, &state = states[thread_id], thread_id]() {
                    try
                    {
                        state.latencies.reserve(
                            static_cast<size_t>(state.reads));
                        std::mt19937_64 rng(0x9e3779b97f4a7c15ULL ^
                                            (thread_id * 0x100000001b3ULL));
                        for (uint64_t i = 0; i < state.reads; ++i)
                        {
                            eloqstore::ReadRequest request;
                            request.SetArgs(
                                h->table,
                                KeyOf(h, rng() % static_cast<uint64_t>(h->rows)));
                            const auto read_start = Clock::now();
                            h->store->ExecSync(&request);
                            const double usecs =
                                std::chrono::duration<double, std::micro>(
                                    Clock::now() - read_start)
                                    .count();
                            if (request.Error() !=
                                eloqstore::KvError::NoError)
                            {
                                state.error = KvErrorText(
                                    "random_reads read", request.Error());
                                return;
                            }
                            state.latencies.push_back(usecs);
                            ++state.completed;
                        }
                    }
                    catch (const std::exception &err)
                    {
                        state.error =
                            std::string("random_reads thread: ") + err.what();
                    }
                    catch (...)
                    {
                        state.error =
                            "random_reads thread: unknown exception";
                    }
                });
        }
    }
    catch (const std::exception &err)
    {
        for (auto &thread : threads)
        {
            thread.join();
        }
        return Fail(h,
                    std::string("random_reads: failed to start reader "
                                "thread: ") +
                        err.what());
    }

    for (auto &thread : threads)
    {
        thread.join();
    }
    const double secs = ElapsedSecs(start);

    std::vector<double> latencies;
    latencies.reserve(static_cast<size_t>(total_reads));
    uint64_t completed = 0;
    for (size_t i = 0; i < states.size(); ++i)
    {
        if (!states[i].error.empty())
        {
            return Fail(h, "random_reads: reader " + std::to_string(i) + ": " +
                               states[i].error);
        }
        completed += states[i].completed;
        latencies.insert(latencies.end(),
                         states[i].latencies.begin(),
                         states[i].latencies.end());
    }

    out->ops = completed;
    out->secs = secs;
    out->p99_us = Percentile(latencies, 0.99);
    return 0;
}

int RunRangeReads(FkvBenchFfi *h, FkvBenchResult *out)
{
    if (!EnsureLoaded(h))
    {
        return -1;
    }

    const uint64_t scans = h->range_scans > 0
                               ? h->range_scans
                               : std::max<uint64_t>(1000, h->rows / 2);
    const uint64_t scan_len = h->range_len > 0 ? h->range_len : 10;
    std::vector<double> latencies;
    latencies.reserve(static_cast<size_t>(scans));
    std::mt19937_64 rng(0xd1b54a32d192ed03ULL);

    const auto start = Clock::now();
    // fkv 的 scan_range 把起点限制在 [0, rows - scan_len)，保证每次窗口
    // 都是完整 scan_len 条；这里同口径，避免尾部短窗口拉低平均条数。
    const uint64_t max_start =
        static_cast<uint64_t>(h->rows) > scan_len
            ? static_cast<uint64_t>(h->rows) - scan_len
            : 1;
    for (uint64_t i = 0; i < scans; ++i)
    {
        const uint64_t begin = rng() % max_start;
        const uint64_t end = begin + scan_len;
        eloqstore::ScanRequest request;
        request.SetPagination(kScanPageEntries, 0);
        request.SetArgs(
            h->table, KeyOf(h, begin), KeyOf(h, end),
            /*begin_inclusive=*/true,
            /*end_inclusive=*/false);
        const auto scan_start = Clock::now();
        h->store->ExecSync(&request);
        const double usecs =
            std::chrono::duration<double, std::micro>(Clock::now() - scan_start)
                .count();
        if (request.Error() != eloqstore::KvError::NoError)
        {
            return Fail(h, KvErrorText("range_reads scan", request.Error()));
        }
        latencies.push_back(usecs);
    }

    // redb reports random range reads in scans/s, not entries/s.
    out->ops = scans;
    out->secs = ElapsedSecs(start);
    out->p99_us = Percentile(latencies, 0.99);
    return 0;
}

int RunRemovals(FkvBenchFfi *h, FkvBenchResult *out)
{
    if (!EnsureLoaded(h))
    {
        return -1;
    }

    const uint64_t removals = h->removals > 0 ? h->removals : h->rows;
    const auto start = Clock::now();
    for (uint64_t offset = 0; offset < removals; offset += h->batch_size)
    {
        const uint64_t count =
            std::min<uint64_t>(h->batch_size, removals - offset);
        if (!ExecBatch(h,
                       MakeEntries(h,
                                   offset,
                                   count,
                                   eloqstore::WriteOp::Delete,
                                   h->NextTimestamp()),
                       "removals batch delete"))
        {
            return -1;
        }
    }
    h->loaded = false;

    out->ops = removals;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

// EloqStore has no O(1) key-count API, so len is a full paginated scan.  The
// measured time includes every page fetch, not just the final count.
int RunLen(FkvBenchFfi *h, FkvBenchResult *out)
{
    if (!EnsureLoaded(h))
    {
        return -1;
    }

    uint64_t count = 0;
    std::string begin;
    bool begin_inclusive = true;

    const auto start = Clock::now();
    while (true)
    {
        eloqstore::ScanRequest request;
        request.SetPagination(kScanPageEntries, 0);
        request.SetArgs(
            h->table, begin, std::string{}, begin_inclusive, false);
        h->store->ExecSync(&request);
        if (request.Error() != eloqstore::KvError::NoError)
        {
            return Fail(h, KvErrorText("len scan", request.Error()));
        }

        const auto entries = request.Entries();
        count += entries.size();
        // HasRemaining means the page limit was hit; continue after the last
        // returned key without re-counting it.  The empty check is a
        // defensive guard against a degenerate empty page.
        if (!request.HasRemaining() || entries.empty())
        {
            break;
        }
        begin = entries.back().key_;
        begin_inclusive = false;
    }

    out->ops = count;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

int RunCompact(FkvBenchFfi *h, FkvBenchResult *out)
{
    if (!EnsureLoaded(h))
    {
        return -1;
    }

    const auto start = Clock::now();
    // The harness always passes partitions=1 and writes only partition 0, but
    // honor the argument so a future multi-partition caller compacts each
    // partition exactly once.
    for (uint32_t partition = 0; partition < h->partitions; ++partition)
    {
        eloqstore::CompactRequest request;
        request.SetTableId(eloqstore::TableIdent(kTableName, partition));
        h->store->ExecSync(&request);
        if (request.Error() != eloqstore::KvError::NoError)
        {
            return Fail(h,
                        KvErrorText(
                            ("compact partition " + std::to_string(partition))
                                .c_str(),
                            request.Error()));
        }
    }

    out->ops = 0;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

}  // namespace

uint64_t FkvBenchFfi::NextTimestamp()
{
    uint64_t ts = WallMicros();
    if (ts <= last_timestamp)
    {
        ts = last_timestamp + 1;
    }
    last_timestamp = ts;
    return ts;
}

extern "C" FKV_BENCH_FFI_API uint32_t fkv_bench_abi_version(void)
{
    return FKV_BENCH_ABI_VERSION;
}

extern "C" FKV_BENCH_FFI_API FkvBenchFfi *fkv_bench_open(
    const char *root, const FkvBenchConfig *config)
{
    tls_open_error.clear();
    if (root == nullptr || root[0] == '\0')
    {
        tls_open_error = "fkv_bench_open: root must be a non-empty path";
        return nullptr;
    }
    if (config == nullptr)
    {
        tls_open_error = "fkv_bench_open: config must not be null";
        return nullptr;
    }
    const uint32_t rows = config->rows;
    const uint32_t bulk_rows =
        config->bulk_rows > 0 ? config->bulk_rows : config->rows;
    const uint32_t batch_size = config->batch_size;
    const uint32_t kv_size = config->kv_size;
    const uint32_t partitions = config->partitions;
    const uint32_t key_size = config->key_size;
    if (rows == 0)
    {
        tls_open_error = "fkv_bench_open: rows must be greater than zero";
        return nullptr;
    }
    if (bulk_rows == 0)
    {
        tls_open_error =
            "fkv_bench_open: bulk_rows must be greater than zero";
        return nullptr;
    }
    if (batch_size == 0)
    {
        tls_open_error =
            "fkv_bench_open: batch_size must be greater than zero";
        return nullptr;
    }
    if (key_size < kKeyIndexBytes)
    {
        tls_open_error =
            "fkv_bench_open: key_size must be at least 8 bytes";
        return nullptr;
    }
    if (kv_size <= key_size)
    {
        tls_open_error =
            "fkv_bench_open: kv_size must be greater than key_size";
        return nullptr;
    }
    if (partitions == 0)
    {
        tls_open_error = "fkv_bench_open: partitions must be greater than zero";
        return nullptr;
    }

    try
    {
        auto handle = std::make_unique<FkvBenchFfi>();
        handle->root = root;
        handle->rows = rows;
        handle->bulk_rows = bulk_rows;
        handle->batch_size = batch_size;
        handle->kv_size = kv_size;
        handle->partitions = partitions;
        handle->key_size = key_size;
        handle->num_threads =
            config->num_threads > 0 ? config->num_threads : 1;
        handle->individual_writes = config->individual_writes;
        handle->batch_writes = config->batch_writes;
        handle->removals = config->removals;
        handle->reads = config->reads;
        handle->range_scans = config->range_scans;
        handle->range_len = config->range_len;
        handle->value.assign(kv_size - key_size, 'v');

        eloqstore::KvOptions options;
        options.store_path = {handle->root};
        // shard / worker 数：写阶段矩阵固定传 1，读 / scan 阶段传读者数。
        options.num_threads = static_cast<uint16_t>(
            std::min<uint32_t>(handle->num_threads, 65535));
        if (config->buffer_pool_size > 0)
        {
            options.buffer_pool_size = config->buffer_pool_size;
        }
        // 与 fkv 的 data page cache 对齐：C++ 默认只缓存 index 页，
        // 开启后 data / index 共享 buffer_pool_size。
        options.enable_data_page_cache = true;
        handle->store = std::make_unique<eloqstore::EloqStore>(options);
        const eloqstore::KvError err = handle->store->Start("main", 0);
        if (err != eloqstore::KvError::NoError)
        {
            tls_open_error = KvErrorText("fkv_bench_open: Start", err);
            return nullptr;
        }
        return handle.release();
    }
    catch (const std::exception &err)
    {
        tls_open_error =
            std::string("fkv_bench_open: unhandled exception: ") + err.what();
        return nullptr;
    }
    catch (...)
    {
        tls_open_error = "fkv_bench_open: unknown exception";
        return nullptr;
    }
}

extern "C" FKV_BENCH_FFI_API void fkv_bench_close(FkvBenchFfi *h)
{
    if (h == nullptr)
    {
        return;
    }
    try
    {
        if (h->store)
        {
            h->store->Stop();
            h->store.reset();
        }
    }
    catch (...)
    {
        // close() must not throw across the C ABI; a failed Stop still drops
        // the store object below.
    }
    delete h;
}

extern "C" FKV_BENCH_FFI_API int fkv_bench_run(FkvBenchFfi *h,
                                               const char *phase,
                                               uint32_t readers,
                                               double read_secs,
                                               FkvBenchResult *out)
{
    // The fixed-op phases below do not use a duration window.  Keep the
    // parameter for ABI and matrix parity.
    (void) read_secs;

    if (out == nullptr)
    {
        return Fail(h, "fkv_bench_run: out must not be null");
    }
    std::memset(out, 0, sizeof(*out));
    if (h == nullptr)
    {
        return Fail(h, "fkv_bench_run: handle must not be null");
    }
    if (phase == nullptr)
    {
        return Fail(h, "fkv_bench_run: phase must not be null");
    }
    h->last_error.clear();

    try
    {
        const std::string_view name(phase);
        if (name == "bulk_load")
        {
            return RunBulkLoad(h, out);
        }
        if (name == "individual_writes")
        {
            return RunIndividualWrites(h, out);
        }
        if (name == "batch_writes")
        {
            return RunBatchWrites(h, out);
        }
        if (name == "random_reads")
        {
            return RunRandomReads(h, readers, out);
        }
        if (name == "range_reads")
        {
            return RunRangeReads(h, out);
        }
        if (name == "removals")
        {
            return RunRemovals(h, out);
        }
        if (name == "len")
        {
            return RunLen(h, out);
        }
        if (name == "compact")
        {
            return RunCompact(h, out);
        }
        return Fail(h, "fkv_bench_run: unknown phase: " + std::string(phase));
    }
    catch (const std::exception &err)
    {
        return Fail(h,
                    std::string("fkv_bench_run(") + phase +
                        "): unhandled exception: " + err.what());
    }
    catch (...)
    {
        return Fail(h,
                    std::string("fkv_bench_run(") + phase +
                        "): unknown exception");
    }
}

extern "C" FKV_BENCH_FFI_API const char *fkv_bench_last_error(FkvBenchFfi *h)
{
    if (h == nullptr)
    {
        return tls_open_error.c_str();
    }
    return h->last_error.c_str();
}
