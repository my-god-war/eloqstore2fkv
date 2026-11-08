# Human-readable perf table renderer

`scripts/perf_table.py` renders fkv benchmark JSON as a compact, redb-style
perf table. It is pure Python 3 standard library and has no dependencies.

Supported inputs:

- `fkv_bench --history FILE` history JSON (`runs` array, or legacy `run_li`);
- `scripts/perf_gate.sh produce ... --out FILE` baseline JSON
  (`rust.metrics` / `cpp.metrics`).

The renderer never invents a metric. Every row comes from the input, and a
metric with no known suffix or kind is printed under **Other / unclassified**
rather than being dropped or guessed into throughput/latency.

## Usage

```bash
# Flat table with a Group column (text is the default format)
python3 scripts/perf_table.py --history /tmp/rust-bench.json

# One labelled section per metric group
python3 scripts/perf_table.py --history /tmp/rust-bench.json --group

# Markdown table, suitable for a report or PR comment
python3 scripts/perf_table.py --history /tmp/rust-bench.json --format markdown

# Baseline comparison: Rust vs C++ plus a recomputed Rust/C++ ratio
python3 scripts/perf_table.py --baseline /tmp/perf-baseline.json --group

# Read one input from stdin
cat /tmp/rust-bench.json | python3 scripts/perf_table.py --group
```

CLI summary:

```text
perf_table.py [--history FILE] [--baseline FILE]
              [--format text|markdown] [--group]
```

`--history` and `--baseline` may both be passed. In that case the history table
is printed first, followed by the baseline table. `--history -` or
`--baseline -` reads stdin. Omitting both flags reads stdin when it is piped.

## Grouping and direction

Metrics are grouped from their name. The `--group` flag prints one section per
group. Without `--group`, the same rows are printed in one table with a `Group`
column. In both modes the group order is fixed:

| Group | Matching rule | Direction shown |
|---|---|---|
| Throughput | suffix `_kv_s`, `_mib_s`, `_batch_s`, `_gib_s`, `_qps`, `_page_s`, `_file_s` | `higher` |
| Latency | suffix `_us` or `_ms` (except `ttl_ms`) | `lower` |
| Resources / size | `rss_peak_kb`, or a name containing `_bytes`, or suffix `_kb` | `info` |
| Amplification / counts | `write_amp`, `space_amp_nominal`, suffix `_ratio`, `_amp`, `_per_batch`, `_num`, `_cnt`, `_count`, `_deleted` | `info` |
| Other / unclassified | everything else, including `ttl_ms` | `info` |

`higher` and `lower` are the only performance directions. Resources, sizes,
amplification ratios and counters are shown as `info`: they depend on dataset,
file layout, GC timing and sampling window, so the table does not apply a hard
pass/fail direction to them. This matches the perf gate, which also treats these
as audit-only metrics.

Examples of the classification:

```text
put_kv_s          throughput    kv/s     higher
get_qps           throughput    QPS      higher
put_mib_s         throughput    MiB/s    higher
get_p99_us        latency       us       lower
reopen_us         latency       us       lower
store_bytes       resources     byte     info
rss_peak_kb       resources     kB       info
write_amp         amplification x        info
io_read_at_num    amplification count    info
ttl_ms            other         ms       info
```

## Repeated runs and ranges

`fkv_bench --repeat N` writes each metric as:

- `<metric>`: median over the repeated runs;
- `<metric>_min`: observed minimum;
- `<metric>_max`: observed maximum.

The renderer folds those three records into one row, so the table shows the
median plus the min-max range:

```text
Metric     Unit  Value   Range            Direction
put_kv_s   kv/s  21,377  20,000 - 22,000  higher
```

Median/mean fields are also understood:

- if `<metric>` is absent, `<metric>_median` supplies the center value;
- otherwise `<metric>_mean` supplies the center value;
- `<metric>_min` / `<metric>_max` supply the range when present.

If only a range is available and no center value exists, the value column shows
`-` and the range is still printed. The renderer does not synthesize a midpoint.
If the same metric appears more than once in a history (`impl` plus `metric`),
the renderer uses the median of those records and their min/max as the range.
A lone `_min` or `_max` with no base or sibling variant is left as its own row,
so unknown metrics are not silently renamed.

## Units

fkv history runs carry an explicit `unit` field, and that value is printed per
row (`kv/s`, `QPS`, `MiB/s`, `us`, `ms`, `byte`, `ratio`, `count`, and so on).

Baseline JSON produced by `perf_gate.sh produce` does not store per-metric units.
For baseline rows the unit is inferred from the metric name using the suffix
rules above. If a name has no unambiguous unit convention, the unit column shows
`-` instead of a fabricated unit.

## Baseline details

Baseline rows are the union of `rust.metrics` and `cpp.metrics`. Columns are:

```text
Metric | Unit | Rust | C++ | Rust/C++ | Direction
```

- A metric present on only one side shows `-` for the other side.
- The `Rust/C++` ratio is recomputed from the two displayed values when both are
  present and the C++ value is non-zero. The file's `ratios` object is not
  trusted separately because it can be stale.
- Repeated-run `_min` / `_max` / `_median` / `_mean` fields are folded the same
  way as history rows.
- The optional `resources` sidecar from `perf_local.sh` has no stable schema
  across versions, so it is not interpreted by this renderer.

## Example output

Text with grouping:

```text
$ python3 scripts/perf_table.py --history /tmp/rust-bench.json --group
fkv perf history
  impl: rust
  commit: 50c27aad1111
  workload: write-read
  runs: 19

Throughput (higher is better)

Metric     Unit     Value              Range  Direction
---------  -----  -------  -----------------  ---------
get_qps    QPS      7,293                  -  higher
put_kv_s   kv/s    21,377    20,000 - 22,000  higher
put_mib_s  MiB/s     2.61                  -  higher

Latency (lower is better)

Metric      Unit   Value  Range  Direction
----------  ----  ------  -----  ---------
get_p99_us  us    10,000      -  lower
put_p99_us  us    35,100      -  lower
reopen_us   us       696      -  lower
```

Markdown rows use the same data:

```markdown
| Group | Metric | Unit | Value | Range | Direction |
|---|---|---|---:|---:|---|
| throughput | put_kv_s | kv/s | 21,377 | 20,000 - 22,000 | higher |
| latency | get_p99_us | us | 10,000 | - | lower |
```

## Exit status

| Code | Meaning |
|---:|---|
| 0 | Table rendered |
| 2 | Unreadable file, invalid JSON, missing/unknown schema, or bad CLI usage |

Errors are written to stderr, for example:

```text
$ python3 scripts/perf_table.py --history /tmp/missing.json
perf_table.py: error: cannot read /tmp/missing.json: [Errno 2] No such file or directory: '/tmp/missing.json'
```
