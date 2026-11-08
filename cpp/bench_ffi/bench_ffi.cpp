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
    uint32_t batch_size{0};
    uint32_t kv_size{0};
    uint32_t partitions{0};
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
constexpr size_t kKeySize = sizeof(uint64_t);
constexpr size_t kRangeScanEntries = 10;
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

void EncodeKey(char *dst, uint64_t value)
{
    for (int i = static_cast<int>(kKeySize) - 1; i >= 0; --i)
    {
        dst[i] = static_cast<char>(value & 0xff);
        value >>= 8;
    }
}

std::string KeyOf(uint64_t value)
{
    std::string key(kKeySize, '\0');
    EncodeKey(key.data(), value);
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
            KeyOf(first_key + i), h->value, timestamp, op);
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

bool BulkLoad(FkvBenchFfi *h, double *secs)
{
    const auto start = Clock::now();
    for (uint64_t offset = 0; offset < h->rows; offset += h->batch_size)
    {
        const uint64_t count =
            std::min<uint64_t>(h->batch_size, h->rows - offset);
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
    return BulkLoad(h, nullptr);
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
    if (!BulkLoad(h, &secs))
    {
        return -1;
    }
    out->ops = h->rows;
    out->secs = secs;
    out->p99_us = 0.0;
    return 0;
}

int RunIndividualWrites(FkvBenchFfi *h, FkvBenchResult *out)
{
    const auto start = Clock::now();
    for (uint64_t key = 0; key < h->rows; ++key)
    {
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
    out->ops = h->rows;
    out->secs = ElapsedSecs(start);
    out->p99_us = 0.0;
    return 0;
}

int RunBatchWrites(FkvBenchFfi *h, FkvBenchResult *out)
{
    const uint64_t batches = h->rows / h->batch_size;
    const auto start = Clock::now();
    for (uint64_t batch = 0; batch < batches; ++batch)
    {
        if (!ExecBatch(h,
                       MakeEntries(h,
                                   batch * h->batch_size,
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
    if (batches * h->batch_size == h->rows)
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

    // More reader threads than keys would only create idle threads.
    const size_t thread_count =
        std::min<size_t>(readers, static_cast<size_t>(h->rows));
    const uint64_t base = h->rows / thread_count;
    const uint64_t remainder = h->rows % thread_count;

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
                                KeyOf(rng() % static_cast<uint64_t>(h->rows)));
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
    latencies.reserve(static_cast<size_t>(h->rows));
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

    const uint64_t scans = std::max<uint64_t>(1000, h->rows / 2);
    std::vector<double> latencies;
    latencies.reserve(static_cast<size_t>(scans));
    std::mt19937_64 rng(0xd1b54a32d192ed03ULL);

    const auto start = Clock::now();
    for (uint64_t i = 0; i < scans; ++i)
    {
        const uint64_t begin = rng() % static_cast<uint64_t>(h->rows);
        const uint64_t end = begin + kRangeScanEntries;
        eloqstore::ScanRequest request;
        request.SetPagination(kScanPageEntries, 0);
        request.SetArgs(
            h->table, KeyOf(begin), KeyOf(end), /*begin_inclusive=*/true,
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

    const auto start = Clock::now();
    for (uint64_t offset = 0; offset < h->rows; offset += h->batch_size)
    {
        const uint64_t count =
            std::min<uint64_t>(h->batch_size, h->rows - offset);
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

    out->ops = h->rows;
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

extern "C" FKV_BENCH_FFI_API FkvBenchFfi *fkv_bench_open(const char *root,
                                                         uint32_t rows,
                                                         uint32_t batch_size,
                                                         uint32_t kv_size,
                                                         uint32_t partitions)
{
    tls_open_error.clear();
    if (root == nullptr || root[0] == '\0')
    {
        tls_open_error = "fkv_bench_open: root must be a non-empty path";
        return nullptr;
    }
    if (rows == 0)
    {
        tls_open_error = "fkv_bench_open: rows must be greater than zero";
        return nullptr;
    }
    if (batch_size == 0)
    {
        tls_open_error =
            "fkv_bench_open: batch_size must be greater than zero";
        return nullptr;
    }
    if (kv_size <= kKeySize)
    {
        tls_open_error =
            "fkv_bench_open: kv_size must be greater than the 8-byte key";
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
        handle->batch_size = batch_size;
        handle->kv_size = kv_size;
        handle->partitions = partitions;
        handle->value.assign(kv_size - kKeySize, 'v');

        eloqstore::KvOptions options;
        options.store_path = {handle->root};
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
