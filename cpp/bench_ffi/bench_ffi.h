#ifndef FKV_BENCH_FFI_H
#define FKV_BENCH_FFI_H

/*
 * C ABI for the C++ EloqStore side of the redb-style benchmark matrix.
 *
 * The Rust matrix crate (`crate/perf/fkv_bench_matrix`) loads this shared
 * library with dlopen and drives one phase at a time.  The handle keeps the
 * EloqStore instance (and its store directory) open between `fkv_bench_run`
 * calls, so `store_bytes` can be measured before the handle is closed.
 *
 * Key / value shape:
 *   key   = 8-byte big-endian uint64
 *   value = (kv_size - 8) bytes, all 'v'
 * Table is always `bm`, partition 0.  The `partitions` argument is accepted
 * for ABI compatibility but the harness uses one partition.
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C"
{
#endif

#if defined(_WIN32)
#define FKV_BENCH_FFI_API __declspec(dllexport)
#elif defined(__GNUC__) || defined(__clang__)
#define FKV_BENCH_FFI_API __attribute__((visibility("default")))
#else
#define FKV_BENCH_FFI_API
#endif

    /** Opaque benchmark handle (one open EloqStore + store path). */
    typedef struct FkvBenchFfi FkvBenchFfi;

    /**
     * Result of one phase.
     *
     * `ops` is the number of logical key/entry operations performed:
     *   bulk_load         keys upserted
     *   individual_writes keys upserted
     *   batch_writes      keys upserted (batches * batch_size)
     *   random_reads      ReadRequest calls completed
     *   range_reads       ScanRequest calls completed (scans/s)
     *   removals          keys deleted
     *   len               entries counted by the full scan
     *   compact           0 (compaction has no logical KV operation)
     *
     * `secs` is the wall-clock duration of the measured phase, excluding any
     * untimed bulk load performed to prepare a read/delete/len phase.
     *
     * `p99_us` is the nearest-rank 99th percentile latency in microseconds for
     * phases that collect per-request latency.  It is 0 when not applicable.
     *
     * Layout is fixed: uint64_t, double, double.  Keep it in sync with the
     * Rust `#[repr(C)] struct FfiBenchResult`.
     */
    typedef struct
    {
        uint64_t ops;
        double secs;
        double p99_us;
    } FkvBenchResult;

    /**
     * Create a store at `root` and start it on branch "main", term 0.
     *
     * `root` must be a fresh store path (the caller owns directory creation /
     * cleanup).  `rows` and `batch_size` must be non-zero and `kv_size` must
     * be greater than 8.
     *
     * On failure returns NULL and makes the error text available through
     * `fkv_bench_last_error(NULL)` on the same thread.
     */
    FKV_BENCH_FFI_API FkvBenchFfi *fkv_bench_open(const char *root,
                                                  uint32_t rows,
                                                  uint32_t batch_size,
                                                  uint32_t kv_size,
                                                  uint32_t partitions);

    /** Stop the store and release the handle.  NULL is a no-op. */
    FKV_BENCH_FFI_API void fkv_bench_close(FkvBenchFfi *h);

    /**
     * Run one phase synchronously on the calling thread.
     *
     * `phase` is one of:
     *   "bulk_load", "individual_writes", "batch_writes", "random_reads",
     *   "range_reads", "removals", "len", "compact".
     *
     * `readers` is only used by random_reads; `read_secs` is accepted for
     * matrix/ABI parity but the FFI executes the fixed operation counts
     * described below instead of a duration window.
     *
     * Returns 0 on success and -1 on failure, with details in
     * `fkv_bench_last_error(h)`.
     */
    FKV_BENCH_FFI_API int fkv_bench_run(FkvBenchFfi *h,
                                        const char *phase,
                                        uint32_t readers,
                                        double read_secs,
                                        FkvBenchResult *out);

    /**
     * Last error message for `h`, or the open() failure for a NULL handle.
     *
     * The returned pointer is owned by the handle (or a thread-local buffer for
     * NULL) and stays valid until the next `fkv_bench_run` / `fkv_bench_open`
     * call or `fkv_bench_close`.  Never returns NULL; an empty string means no
     * error has been recorded.
     */
    FKV_BENCH_FFI_API const char *fkv_bench_last_error(FkvBenchFfi *h);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* FKV_BENCH_FFI_H */
