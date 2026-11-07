# Read-concurrency scaling: fkv vs EloqStore

`scripts/perf_scale_read.sh` measures random point-read throughput while the
number of concurrent readers grows (`1,2,4,8` by default).  It runs the Rust
`fkv_bench` and the C++ `simple_bench` on the same host, with the same dataset
parameters, and emits one row per implementation and reader count.

The harness reuses the binaries and conventions of `scripts/perf_local.sh`:

- build helpers: `scripts/ci/build_rust_bench.sh`,
  `scripts/ci/build_cpp_bench.sh`
- Rust bench: `fkv_bench` from the sibling `fkv` tree
  (`FKV_REPO_PATH`, default `<repo>/../fkv`)
- C++ bench: `simple_bench` from `cpp/eloqstore`

## Quick start

```bash
# Build both sides, write each dataset once, sweep 1,2,4,8, print TSV.
scripts/perf_scale_read.sh

# Prebuilt binaries and JSON to a file.
scripts/perf_scale_read.sh \
  --rust-bin /tmp/fkv/target/release/fkv_bench \
  --cpp-bin  /tmp/build/simple_bench \
  --readers 1,2,4,8,16 --rows 2000000 --test-secs 20 \
  --format json --out /tmp/scale.json

# Rust only (hosts without a C++ toolchain).
scripts/perf_scale_read.sh --rust-only --readers 1,2,4,8

# Stabilize the Rust side: repeat each read phase 3 times on the same store.
scripts/perf_scale_read.sh --trials 3 --readers 1,2,4,8
```

`--help` prints the full option list.  The script requires Linux: it is a
performance harness for the CI / self-hosted hosts.  On macOS only `bash -n`
and `--help` are meaningful.

`--trials N` is Rust-only: `fkv_bench --read-only-existing --trials N` repeats
the read phase N times on the same store and keeps the median. C++ `simple_bench`
has no phase-trial flag, so with `--trials > 1` and a C++ build the script prints
a warning and the C++ rows remain single-pass; do not compare those rows to Rust
medians without saying so.

## Metrics and output

Each read run is timed for `--test-secs` seconds after reopening the store.
`readers` is the number of concurrent in-flight point reads:

| implementation | reader knob |
| --- | --- |
| Rust `fkv_bench` | `--read-per-part N` (lane count = `partitions * read_per_part`) |
| C++ `simple_bench` | `--read_per_part N --read_thds 1` (readers = `read_per_part * partitions`) |

Columns (TSV header and JSON object keys):

| column | meaning |
| --- | --- |
| `impl` | `rust` or `cpp` |
| `readers` | configured concurrent reader count |
| `qps` | completed point reads / second over the timed window |
| `p99_us` | p99 read latency in microseconds |
| `speedup` | `qps` divided by the same implementation's `qps` at `readers=1` |

`speedup` is `-` in TSV (and `null` in JSON) when `readers=1` was not part of the
sweep.  TSV goes to stdout unless `--out` is given; progress, flow labels, and
per-run log paths go to stderr.

Metric sources:

- Rust: `get_kv_s` and `get_p99_us` from the `--history` JSON
  (whole read window, single merged histogram).
- C++: `read average <n> QPS` (summed across threads) and the maximum
  `read speed ... p99 <n> microseconds` from the periodic log lines,
  matching `scripts/perf_gate.sh`.

## Flow: load once, read many times

Both implementations can open an existing store in a new process without
rewriting it, so the default flow shares one dataset across all reader counts:

1. Rust `fkv_bench --load-only --store-dir <root>/rust-store`
2. C++ `simple_bench --workload write --kvoptions <root>/cpp-bench.ini`
3. For every `N`:
   - Rust: `fkv_bench --read-only-existing --store-dir ... --read-per-part N`
   - C++: `simple_bench --workload read --read_per_part N --read_thds 1`

Both read runs reopen the store, which drops the Rust index cache
(`--read-only-existing` calls `reopen`), so the comparison is cold-index for
both sides.  The dataset is written with the same generator:

- `--rows` (Rust `--rows` / `--max-key`; C++ `--max_key`)
- `--batch-size`, `--kv-size`, `--partitions`
- `--write-batchs = rows / batch_size` (derived exactly like `perf_local.sh`)
- Rust `--rng cpp`, which reproduces the C++ `mt19937(0)` random walk with
  `key_interval=4` and `del_ratio=4`

Dataset parameters must stay identical on both sides; do not add
`--key-interval` / `--del-ratio` to the Rust load, because `--rng cpp` ignores
them.

### C++ has no single-process load+read

`simple_bench` cannot write and then reopen the store inside one process.  It
does, however, open the `store_path` from the INI on every invocation, so the
two-process sequence above (the same one used by the `read-only` mode of
`scripts/perf_local.sh`) is a true load-once flow: the C++ dataset is written
once and only reopened by the read runs.

For a build that cannot reopen an existing store, `--cpp-flow fresh` rebuilds
the C++ store for every reader count:

```bash
scripts/perf_scale_read.sh --cpp-flow fresh --readers 1,4,8
```

Fresh mode is logged as `C++ flow=fresh` and must not be compared numerically
with load-once results: each reader count starts from a different freshly
written store.  The harness never switches flows silently; the selected flow is
printed on stderr, and fresh mode writes one
`logs/cpp-load-readers-<N>.log` per reader count.

## Caveats

- Use `--partitions 1` for strict parity.  C++ keeps one full copy of
  `[0, max_key)` per partition; Rust shards the global key space by partition.
- C++ p99 is the maximum p99 over the periodic (about one-second) log windows,
  while Rust p99 covers the whole timed window.  This follows the existing
  `perf_gate.sh` convention and is a summary difference, not a flow difference.
- The C++ `--test-secs` window must be at least 2 seconds so at least one
  periodic `read speed` line exists; the harness enforces this minimum.
- Store directories are kept after the run so logs and data can be inspected.
  Pass `--clean` to remove `<root>/rust-store` and `<root>/cpp-store`.
- The harness never runs Docker and never commits or pushes anything.
