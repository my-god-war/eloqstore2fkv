#!/usr/bin/env python3
"""Render fkv perf history / perf-baseline JSON as a human-readable table.

Inputs
------
fkv_bench ``--history`` JSON::

    {"runs": [{"impl": "rust", "metric": "put_kv_s",
               "value": 21377.0, "unit": "kv/s", ...}, ...]}

``scripts/perf_gate.sh produce`` baseline JSON::

    {"rust": {"metrics": {"put_kv_s": 21377.0, ...}},
     "cpp":  {"metrics": {"put_kv_s": 16866.0, ...}}, ...}

The renderer only prints metrics that are present in the input.  Repeated-run
variants (``<metric>_min`` / ``<metric>_max`` / ``<metric>_mean`` /
``<metric>_median``) are folded into one row: the median (or the explicitly
provided mean/median) plus the min-max range.

Exit status is non-zero for unreadable or structurally invalid input.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Sequence, Tuple


# ---------------------------------------------------------------------------
# Metric classification
# ---------------------------------------------------------------------------

# The order here is also the render order of grouped tables.
GROUPS: List[Tuple[str, str]] = [
    ("throughput", "Throughput (higher is better)"),
    ("latency", "Latency (lower is better)"),
    ("resources", "Resources / size (informational)"),
    ("amplification", "Amplification / counts (informational)"),
    ("other", "Other / unclassified"),
]
GROUP_INDEX = {key: index for index, (key, _label) in enumerate(GROUPS)}

# Suffixes whose value is a rate where a bigger number is better.  This mirrors
# the perf gate's HIGHER_SUFFIXES, with ``_qps`` covering ``*_window_qps`` too.
HIGHER_SUFFIXES = (
    "_kv_s",
    "_mib_s",
    "_batch_s",
    "_gib_s",
    "_qps",
    "_page_s",
    "_file_s",
)

# fkv_bench appends these after ``--repeat N`` (or after median-only trials).
REPEAT_SUFFIXES = ("_min", "_max", "_mean", "_median")

# Names that are configuration echoes rather than performance results.
INFO_EXACT = frozenset({"ttl_ms"})

# Derived amplification metrics whose names do not end in ``_amp`` / ``_ratio``.
AMP_EXACT = frozenset({"write_amp", "space_amp_nominal"})

INTEGRAL_UNITS = frozenset({"byte", "kB", "count", "num"})


def classify_metric(name: str) -> str:
    """Return one of the GROUPS keys for a metric name.

    Unknown names deliberately fall through to ``other`` instead of being
    guessed into a performance direction.
    """
    if name in INFO_EXACT:
        return "other"

    if name.endswith(HIGHER_SUFFIXES):
        return "throughput"

    if name.endswith("_us") or name.endswith("_ms"):
        return "latency"

    if name == "rss_peak_kb" or "_bytes" in name or name.endswith("_kb"):
        return "resources"

    if (
        name in AMP_EXACT
        or name.endswith("_ratio")
        or name.endswith("_amp")
        or name.endswith("_per_batch")
        or name.endswith("_num")
        or name.endswith("_cnt")
        or name.endswith("_count")
        or name.endswith("_deleted")
    ):
        return "amplification"

    return "other"


def direction_for(group: str) -> str:
    if group == "throughput":
        return "higher"
    if group == "latency":
        return "lower"
    # Resources and amplification are path-dependent / trend metrics.  The perf
    # gate also treats them as informational, so the table does not invent a
    # hard direction for them.
    return "info"


def infer_unit(name: str) -> str:
    """Infer a display unit from a metric name (used for baseline JSON).

    fkv history runs carry an explicit ``unit``.  Baseline JSON does not, so
    only names with an unambiguous convention get a unit here; the rest stay
    blank rather than inventing one.
    """
    if name.endswith("_kv_s"):
        return "kv/s"
    if name.endswith("_mib_s"):
        return "MiB/s"
    if name.endswith("_gib_s"):
        return "GiB/s"
    if name.endswith("_batch_s"):
        return "batch/s"
    if name.endswith("_page_s"):
        return "page/s"
    if name.endswith("_file_s"):
        return "file/s"
    if name.endswith("_qps"):
        return "QPS"
    if name.endswith("_us"):
        return "us"
    if name.endswith("_ms"):
        return "ms"
    if name == "rss_peak_kb" or name.endswith("_kb"):
        return "kB"
    if "_bytes" in name:
        return "byte"
    if name.endswith("_per_batch"):
        return "num/batch"
    if name.endswith("_num") or name.endswith("_cnt") or name.endswith("_count"):
        return "count"
    if name in ("write_amp", "space_amp_nominal"):
        return "x"
    if name.endswith("_ratio") or name.endswith("_amp"):
        return "ratio"
    return ""


# ---------------------------------------------------------------------------
# Input model
# ---------------------------------------------------------------------------


class InputError(Exception):
    """Raised for missing files, bad JSON, or an unrecognised schema."""


@dataclass
class Sample:
    """A metric's center value plus an optional observed min-max range."""

    value: Optional[float]
    lo: Optional[float] = None
    hi: Optional[float] = None
    unit: str = ""


def _as_number(value: Any) -> Optional[float]:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        number = float(value)
    elif isinstance(value, str):
        try:
            number = float(value)
        except ValueError:
            return None
    else:
        return None
    return number if math.isfinite(number) else None


def read_json(path: str) -> Any:
    try:
        if path == "-":
            text = sys.stdin.read()
        else:
            with open(path, "r", encoding="utf-8") as handle:
                text = handle.read()
    except OSError as exc:
        raise InputError("cannot read %s: %s" % (path, exc)) from exc

    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise InputError("invalid JSON in %s: %s" % (path, exc)) from exc


def _sample_from_values(values: Sequence[Tuple[float, str]]) -> Sample:
    numbers = [number for number, _unit in values]
    units = []
    for _number, unit in values:
        if unit and unit not in units:
            units.append(unit)
    unit = units[0] if units else ""
    if len(numbers) == 1:
        return Sample(numbers[0], None, None, unit)
    return Sample(statistics.median(numbers), min(numbers), max(numbers), unit)


def _strip_repeat_suffix(name: str) -> Optional[str]:
    for suffix in REPEAT_SUFFIXES:
        if name.endswith(suffix) and len(name) > len(suffix):
            return name[: -len(suffix)]
    return None


def fold_repeat_metrics(metrics: Dict[str, Sample]) -> Dict[str, Sample]:
    """Fold ``<metric>_min/_max/_mean/_median`` variants into their base row.

    The base metric is normally already the median for fkv ``--repeat`` runs.
    If the base is absent, an explicit ``_median`` (then ``_mean``) supplies the
    center; that is why a lone median/mean field is folded too.  A lone
    ``_min``/``_max`` without a base or sibling variant is left alone, so a
    genuinely unknown metric is still visible under ``other``.
    """
    variant_groups: Dict[str, Dict[str, Sample]] = {}
    for name, sample in metrics.items():
        base = _strip_repeat_suffix(name)
        if base is None:
            continue
        suffix = name[len(base) :]
        variant_groups.setdefault(base, {})[suffix] = sample

    candidate_bases = set()
    for base, variants in variant_groups.items():
        if (
            base in metrics
            or len(variants) >= 2
            or "_median" in variants
            or "_mean" in variants
        ):
            candidate_bases.add(base)

    consumed = set()
    for base in candidate_bases:
        consumed.update(base + suffix for suffix in variant_groups.get(base, {}))

    result = {name: sample for name, sample in metrics.items() if name not in consumed}

    for base in sorted(candidate_bases):
        variants = variant_groups.get(base, {})
        base_sample = metrics.get(base)

        center: Optional[float] = None
        if base_sample is not None:
            center = base_sample.value
        if center is None and "_median" in variants:
            center = variants["_median"].value
        if center is None and "_mean" in variants:
            center = variants["_mean"].value

        lo = variants["_min"].value if "_min" in variants else None
        hi = variants["_max"].value if "_max" in variants else None
        if lo is None and base_sample is not None:
            lo = base_sample.lo
        if hi is None and base_sample is not None:
            hi = base_sample.hi

        unit = ""
        if base_sample is not None:
            unit = base_sample.unit
        if not unit:
            for suffix in REPEAT_SUFFIXES:
                if suffix in variants and variants[suffix].unit:
                    unit = variants[suffix].unit
                    break

        if center is None and lo is None and hi is None:
            continue
        result[base] = Sample(center, lo, hi, unit)

    return result


def load_history(data: Any) -> Tuple[Dict[str, Dict[str, Sample]], List[Tuple[str, str]]]:
    """Parse fkv_bench history JSON into ``{impl: {metric: Sample}}``."""
    if isinstance(data, dict):
        runs = data.get("runs")
        if runs is None:
            runs = data.get("run_li")  # pre-rename history files
    elif isinstance(data, list):
        runs = data
    else:
        runs = None

    if not isinstance(runs, list):
        raise InputError(
            "history JSON must contain a 'runs' (or legacy 'run_li') array"
        )
    if not runs:
        raise InputError("history JSON contains no runs")

    collected: Dict[Tuple[str, str], List[Tuple[float, str]]] = {}
    commits = set()
    platforms = set()
    impls = set()

    for index, run in enumerate(runs):
        if not isinstance(run, dict):
            raise InputError("history runs[%d] is not an object" % index)
        name = run.get("metric")
        if not isinstance(name, str) or not name.strip():
            raise InputError("history runs[%d] has no metric name" % index)
        number = _as_number(run.get("value"))
        if number is None:
            raise InputError(
                "history runs[%d] (%s) has a non-numeric value" % (index, name)
            )
        impl = run.get("impl")
        impl = impl if isinstance(impl, str) else ""
        unit = run.get("unit")
        unit = unit if isinstance(unit, str) else ""

        collected.setdefault((impl, name), []).append((number, unit))
        if isinstance(run.get("commit"), str) and run["commit"]:
            commits.add(run["commit"])
        if isinstance(run.get("platform"), str) and run["platform"]:
            platforms.add(run["platform"])
        if impl:
            impls.add(impl)

    by_impl: Dict[str, Dict[str, Sample]] = {}
    for (impl, name), values in collected.items():
        by_impl.setdefault(impl, {})[name] = _sample_from_values(values)

    folded = {
        impl: fold_repeat_metrics(metrics) for impl, metrics in by_impl.items()
    }

    meta: List[Tuple[str, str]] = []
    if impls:
        meta.append(("impl", ", ".join(sorted(impls))))
    if commits:
        shown = ", ".join(_short_commit(commit) for commit in sorted(commits))
        meta.append(("commit", shown))
    if platforms:
        meta.append(("platform", ", ".join(sorted(platforms))))
    if isinstance(data, dict):
        workload = data.get("workload")
        if isinstance(workload, str) and workload:
            meta.append(("workload", workload))
        generated = _format_generated_at(data.get("generated_at"))
        if generated:
            meta.append(("generated_at", generated))
    meta.append(("runs", str(len(runs))))

    return folded, meta


def load_baseline(data: Any) -> Tuple[Dict[str, Dict[str, Sample]], List[Tuple[str, str]]]:
    """Parse perf_gate.sh ``produce`` baseline JSON.

    Only ``rust.metrics`` / ``cpp.metrics`` are rendered.  The optional
    ``resources`` sidecar has no single schema across perf_local versions, so
    it is intentionally not interpreted here.
    """
    if not isinstance(data, dict):
        raise InputError("baseline JSON must be an object")

    sides: Dict[str, Dict[str, Sample]] = {}
    for impl in ("rust", "cpp"):
        side = data.get(impl)
        metrics = side.get("metrics") if isinstance(side, dict) else None
        if metrics is None:
            continue
        if not isinstance(metrics, dict):
            raise InputError("baseline %s.metrics must be an object" % impl)

        parsed: Dict[str, Sample] = {}
        for name, raw in metrics.items():
            if not isinstance(name, str) or not name:
                raise InputError("baseline %s.metrics has an invalid metric name" % impl)
            number = _as_number(raw)
            if number is None:
                raise InputError(
                    "baseline %s.metrics.%s is not numeric" % (impl, name)
                )
            parsed[name] = Sample(number, None, None, infer_unit(name))
        sides[impl] = fold_repeat_metrics(parsed)

    if not sides:
        raise InputError("baseline JSON has neither rust.metrics nor cpp.metrics")
    if not any(sides.values()):
        raise InputError("baseline JSON contains no rust/cpp metrics")

    meta: List[Tuple[str, str]] = []
    for key in (
        "schema",
        "source",
        "runner",
        "bench_commit",
        "data_append_mode",
        "direct_io",
        "generated_at",
    ):
        if key not in data:
            continue
        value = data[key]
        if isinstance(value, bool):
            value = "true" if value else "false"
        if value is None or value == "":
            continue
        meta.append((key, str(value)))

    return sides, meta


def _short_commit(commit: str) -> str:
    return commit[:12] if len(commit) > 12 else commit


def _format_generated_at(value: Any) -> str:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        try:
            from datetime import datetime, timezone

            return datetime.fromtimestamp(float(value), tz=timezone.utc).strftime(
                "%Y-%m-%dT%H:%M:%SZ"
            )
        except (OverflowError, OSError, ValueError):
            return str(value)
    return str(value) if isinstance(value, str) else ""


# ---------------------------------------------------------------------------
# Number / table formatting
# ---------------------------------------------------------------------------


def _trim_zeros(text: str) -> str:
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"


def format_number(value: Optional[float], unit: str) -> str:
    if value is None:
        return "-"

    magnitude = abs(value)
    if unit in INTEGRAL_UNITS:
        if abs(value - round(value)) < 1e-9:
            return "{:,.0f}".format(value)
        return _trim_zeros("{:,.2f}".format(value))

    if unit in ("us", "ms"):
        if magnitude >= 1000:
            return "{:,.0f}".format(value)
        return _trim_zeros("{:,.1f}".format(value))

    if unit in ("ratio", "x"):
        return _trim_zeros("{:,.3f}".format(value))

    if unit in ("MiB/s", "GiB/s"):
        return _trim_zeros("{:,.2f}".format(value))

    if unit in (
        "kv/s",
        "QPS",
        "req/s",
        "batch/s",
        "page/s",
        "file/s",
        "num/batch",
    ):
        if magnitude >= 1000:
            return "{:,.0f}".format(value)
        if magnitude >= 10:
            return _trim_zeros("{:,.1f}".format(value))
        return _trim_zeros("{:,.2f}".format(value))

    # Unknown unit: keep enough precision to be useful without scientific
    # notation for ordinary benchmark values.
    if magnitude >= 1000:
        return "{:,.0f}".format(value)
    return _trim_zeros("{:,.3f}".format(value))


def format_range(sample: Sample) -> str:
    if sample.lo is None and sample.hi is None:
        return "-"
    if sample.lo is not None and sample.hi is not None:
        return "%s - %s" % (
            format_number(sample.lo, sample.unit),
            format_number(sample.hi, sample.unit),
        )
    if sample.lo is not None:
        return "min %s" % format_number(sample.lo, sample.unit)
    return "max %s" % format_number(sample.hi, sample.unit)


def format_sample_cell(sample: Optional[Sample]) -> str:
    if sample is None:
        return "-"
    value = format_number(sample.value, sample.unit)
    span = format_range(sample)
    if span == "-":
        return value
    return "%s (%s)" % (value, span)


def format_ratio(numerator: Optional[Sample], denominator: Optional[Sample]) -> str:
    if (
        numerator is None
        or denominator is None
        or numerator.value is None
        or denominator.value is None
        or denominator.value == 0
    ):
        return "-"
    return "{:.3g}x".format(numerator.value / denominator.value)


def _sorted_metrics(metrics: Dict[str, Sample]) -> List[str]:
    return sorted(metrics, key=lambda name: (GROUP_INDEX[classify_metric(name)], name))


def _render_text_table(
    headers: Sequence[str], rows: Sequence[Sequence[str]], aligns: Sequence[str]
) -> List[str]:
    widths = [len(header) for header in headers]
    for row in rows:
        for index, cell in enumerate(row):
            widths[index] = max(widths[index], len(cell))

    def render(row: Sequence[str]) -> str:
        cells = []
        for index, cell in enumerate(row):
            if aligns[index] == "r":
                cells.append(cell.rjust(widths[index]))
            else:
                cells.append(cell.ljust(widths[index]))
        return "  ".join(cells).rstrip()

    lines = [render(headers), "  ".join("-" * width for width in widths)]
    lines.extend(render(row) for row in rows)
    return lines


def _render_markdown_table(
    headers: Sequence[str], rows: Sequence[Sequence[str]], aligns: Sequence[str]
) -> List[str]:
    def escape(cell: str) -> str:
        return cell.replace("|", "\\|")

    lines = [
        "| " + " | ".join(escape(header) for header in headers) + " |",
        "|"
        + "|".join("---:" if align == "r" else "---" for align in aligns)
        + "|",
    ]
    for row in rows:
        lines.append("| " + " | ".join(escape(cell) for cell in row) + " |")
    return lines


def _emit_table(
    lines: List[str],
    fmt: str,
    headers: Sequence[str],
    rows: Sequence[Sequence[str]],
    aligns: Sequence[str],
) -> None:
    if not rows:
        return
    if fmt == "markdown":
        lines.extend(_render_markdown_table(headers, rows, aligns))
    else:
        lines.extend(_render_text_table(headers, rows, aligns))


def _emit_sections(
    lines: List[str],
    fmt: str,
    grouped: bool,
    rows: Sequence[Dict[str, Any]],
    base_headers: Sequence[str],
    base_aligns: Sequence[str],
    include_group_column: bool,
) -> None:
    """Emit either one flat table or one table per metric group."""
    headers = list(base_headers)
    aligns = list(base_aligns)
    if include_group_column and not grouped:
        headers = ["Group"] + headers
        aligns = ["l"] + aligns

    def row_values(row: Dict[str, Any]) -> List[str]:
        values = list(row["cells"])
        if include_group_column and not grouped:
            values = [row["group"]] + values
        return values

    if not grouped:
        ordered = sorted(
            rows,
            key=lambda row: (
                GROUP_INDEX[row["group"]],
                row["sort_key"],
            ),
        )
        _emit_table(lines, fmt, headers, [row_values(row) for row in ordered], aligns)
        return

    for group, label in GROUPS:
        group_rows = [row for row in rows if row["group"] == group]
        if not group_rows:
            continue
        group_rows.sort(key=lambda row: row["sort_key"])
        if lines and lines[-1] != "":
            lines.append("")
        lines.append(("### " if fmt == "markdown" else "") + label)
        lines.append("")
        _emit_table(
            lines,
            fmt,
            headers,
            [row_values(row) for row in group_rows],
            aligns,
        )


# ---------------------------------------------------------------------------
# History / baseline rendering
# ---------------------------------------------------------------------------


def render_history(
    history: Dict[str, Dict[str, Sample]],
    meta: List[Tuple[str, str]],
    fmt: str,
    grouped: bool,
) -> List[str]:
    lines = ["## fkv perf history" if fmt == "markdown" else "fkv perf history"]
    if fmt == "markdown":
        lines.append("")
        for label, value in meta:
            lines.append("- %s: `%s`" % (label, value))
    else:
        for label, value in meta:
            lines.append("  %s: %s" % (label, value))

    impls = sorted(history)
    include_impl = len(impls) > 1

    headers = ["Metric", "Unit", "Value", "Range", "Direction"]
    aligns = ["l", "l", "r", "r", "l"]
    if include_impl:
        headers = ["Impl"] + headers
        aligns = ["l"] + aligns

    rows: List[Dict[str, Any]] = []
    for impl in impls:
        for name in _sorted_metrics(history[impl]):
            sample = history[impl][name]
            group = classify_metric(name)
            cells = [
                name,
                sample.unit or "-",
                format_number(sample.value, sample.unit),
                format_range(sample),
                direction_for(group),
            ]
            if include_impl:
                cells = [impl or "-"] + cells
            rows.append(
                {
                    "group": group,
                    "sort_key": (impl, name),
                    "cells": cells,
                }
            )

    if not rows:
        lines.append("")
        lines.append("(no metrics)")
        return lines

    if fmt == "markdown":
        lines.append("")
    _emit_sections(
        lines,
        fmt,
        grouped,
        rows,
        headers,
        aligns,
        include_group_column=True,
    )
    return lines


def render_baseline(
    sides: Dict[str, Dict[str, Sample]],
    meta: List[Tuple[str, str]],
    fmt: str,
    grouped: bool,
) -> List[str]:
    lines = [
        "## fkv perf baseline" if fmt == "markdown" else "fkv perf baseline"
    ]
    if fmt == "markdown":
        lines.append("")
        for label, value in meta:
            lines.append("- %s: `%s`" % (label, value))
    else:
        for label, value in meta:
            lines.append("  %s: %s" % (label, value))

    rust = sides.get("rust", {})
    cpp = sides.get("cpp", {})
    names = sorted(
        set(rust) | set(cpp),
        key=lambda name: (GROUP_INDEX[classify_metric(name)], name),
    )

    headers = ["Metric", "Unit", "Rust", "C++", "Rust/C++", "Direction"]
    aligns = ["l", "l", "r", "r", "r", "l"]

    rows: List[Dict[str, Any]] = []
    for name in names:
        rust_sample = rust.get(name)
        cpp_sample = cpp.get(name)
        group = classify_metric(name)
        unit = ""
        if rust_sample is not None and rust_sample.unit:
            unit = rust_sample.unit
        elif cpp_sample is not None and cpp_sample.unit:
            unit = cpp_sample.unit
        else:
            unit = infer_unit(name)

        cells = [
            name,
            unit or "-",
            format_sample_cell(rust_sample),
            format_sample_cell(cpp_sample),
            format_ratio(rust_sample, cpp_sample),
            direction_for(group),
        ]
        rows.append(
            {
                "group": group,
                "sort_key": (name,),
                "cells": cells,
            }
        )

    if not rows:
        lines.append("")
        lines.append("(no metrics)")
        return lines

    if fmt == "markdown":
        lines.append("")
    _emit_sections(
        lines,
        fmt,
        grouped,
        rows,
        headers,
        aligns,
        include_group_column=True,
    )
    return lines


def render(
    fmt: str,
    grouped: bool,
    history: Optional[Tuple[Dict[str, Dict[str, Sample]], List[Tuple[str, str]]]],
    baseline: Optional[Tuple[Dict[str, Dict[str, Sample]], List[Tuple[str, str]]]],
) -> str:
    lines: List[str] = []
    if history is not None:
        lines.extend(render_history(history[0], history[1], fmt, grouped))
    if baseline is not None:
        if lines:
            lines.append("")
        lines.extend(render_baseline(baseline[0], baseline[1], fmt, grouped))
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Render fkv_bench --history or perf_gate.sh produce baseline JSON "
            "as a grouped human-readable perf table."
        )
    )
    parser.add_argument(
        "--history",
        metavar="FILE",
        help="fkv_bench --history JSON ('-' reads stdin)",
    )
    parser.add_argument(
        "--baseline",
        metavar="FILE",
        help="perf_gate.sh produce baseline JSON ('-' reads stdin)",
    )
    parser.add_argument(
        "--format",
        choices=("text", "markdown"),
        default="text",
        help="output format (default: text)",
    )
    parser.add_argument(
        "--group",
        action="store_true",
        help="print one labelled section per metric group",
    )
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    stdin_uses = [path for path in (args.history, args.baseline) if path == "-"]
    if len(stdin_uses) > 1:
        parser.error("stdin can only be used by one of --history / --baseline")

    history_path = args.history
    baseline_path = args.baseline
    if history_path is None and baseline_path is None:
        if sys.stdin.isatty():
            parser.error("pass --history FILE or --baseline FILE")
        history_path = "-"

    try:
        history = None
        baseline = None
        if history_path is not None:
            history = load_history(read_json(history_path))
        if baseline_path is not None:
            baseline = load_baseline(read_json(baseline_path))
    except InputError as exc:
        print("perf_table.py: error: %s" % exc, file=sys.stderr)
        return 2

    sys.stdout.write(render(args.format, args.group, history, baseline))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
