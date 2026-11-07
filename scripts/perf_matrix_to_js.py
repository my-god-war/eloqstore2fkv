#!/usr/bin/env python3
"""把 fkv_bench_matrix 的 JSON 报告回填到 web/src/conf/perf.js。

首页矩阵的每个数字都必须能从 CI 产物追溯，手工抄写既容易抄错也难以复核。
本脚本是唯一回填入口：

    python3 scripts/perf_matrix_to_js.py \
      --json /path/fkv-matrix.json \
      --perf-js web/src/conf/perf.js \
      --run-id 42329292969 \
      --date 2026-11-07

只改写 `PERF_META` 与 `PERF_MATRIX_ROWS` 两个块，其余内容（PERF_NOTE、
VERIFY_ROWS、DOC_LINKS 等）原样保留，便于人工继续补充口径说明。

规则（与既有页面一致）：
- 吞吐格式：>=1e6 Mops/s、>=1e3 kops/s（2 位小数）、否则 ops/s（1 位小数）。
- 有 p99 的一侧写成 "<throughput>, p99 <n> µs"（JSON 里 p99_us 为 null 就留空）。
- ratio = fkv / cpp，3 位小数；吞吐行 ratio < 0.5 记 tone "bad"（体积 / compact
  行不判 tone）。
- 体积用 MiB（>=1024 MiB 才切 GiB），compact 时间用秒（3 位小数）。
"""

from __future__ import annotations

import argparse
import json
import re
from datetime import date as date_cls
from pathlib import Path

# phase 标签 -> (是否吞吐行, tone 判据阈值)
THROUGHPUT_PHASES = {
    "bulk load",
    "individual writes",
    "batch writes",
    "random range reads",
    "removals",
    "len()",
}

BAD_RATIO_BELOW = 0.5


def fmt_throughput(value: float) -> str:
    if value >= 1e9:
        return f"{value / 1e9:.2f} Gops/s"
    if value >= 1e6:
        return f"{value / 1e6:.2f} Mops/s"
    if value >= 1e3:
        return f"{value / 1e3:.2f} kops/s"
    return f"{value:.1f} ops/s"


def cell(result: dict | None, with_p99: bool) -> str:
    if result is None:
        return "—"
    text = fmt_throughput(result["throughput"])
    p99 = result.get("p99_us")
    if with_p99 and p99 is not None and p99 > 0:
        text = f"{text}, p99 {p99:.1f} µs"
    return text


def fmt_bytes(value: int) -> str:
    mib = value / (1024 * 1024)
    if mib >= 1024:
        return f"{mib / 1024:.2f} GiB"
    return f"{mib:.2f} MiB"


def fmt_ratio(value: float | None) -> str:
    return "null" if value is None else f'"{value:.3f}x"'


def size_cell(value: int | None) -> str:
    return "—" if value is None else fmt_bytes(value)


def build_rows(report: dict) -> list[str]:
    rows: list[str] = []
    for row in report["phases"]:
        label = row["phase"]
        # 点读两侧都给 p99；其余吞吐行只有 fkv 侧有 p99（C++ simple_bench 的
        # 周期窗口最大值不是同口径分位，JSON 里为 null 时就不写）。
        cpp = cell(row.get("cpp"), with_p99=True)
        fkv = cell(row.get("fkv"), with_p99=True)
        ratio = row.get("ratio")
        tone = ""
        if label in THROUGHPUT_PHASES and ratio is not None and ratio < BAD_RATIO_BELOW:
            tone = ', "bad"'
        rows.append(
            f'  ["{label}", "{cpp}", "{fkv}", {fmt_ratio(ratio)}{tone}]'
        )

    for label, key in (("uncompacted size", "uncompacted_size"),
                       ("compacted size", "compacted_size")):
        block = report[key]
        rows.append(
            f'  ["{label}", "{size_cell(block["cpp"])}", '
            f'"{size_cell(block["fkv"])}", {fmt_ratio(block.get("ratio"))}]'
        )

    cpp_secs = report["compaction"].get("cpp_secs")
    fkv_secs = report["compaction"].get("fkv_secs")
    cpp_cell = "—" if cpp_secs is None else f"{cpp_secs:.3f} s"
    fkv_cell = "—" if fkv_secs is None else f"{fkv_secs:.3f} s"
    rows.append(f'  ["compact wall time", "{cpp_cell}", "{fkv_cell}", null]')
    return rows


def build_meta(report: dict, run_id: str, date: str) -> str:
    cfg = report["config"]
    readers = "/".join(str(r) for r in cfg["readers"])
    return (
        "redb 风格阶段矩阵 · C++ EloqStore FFI vs Rust fkv · "
        f"{date} · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · "
        f"rows={cfg['rows']} · batch_size={cfg['batch_size']} · "
        f"kv_size={cfg['kv_size']} B · readers {readers} · "
        f"read_secs={cfg['read_secs']:g}"
    )


def replace_block(text: str, name: str, body: str) -> str:
    pattern = re.compile(
        rf"(export const {name} =\s*)(?:\[[\s\S]*?\];|\"(?:[^\"\\]|\\.)*\";)",
        re.MULTILINE,
    )
    if not pattern.search(text):
        raise SystemExit(f"{name}: 在 perf.js 里找不到可替换的块")
    return pattern.sub(lambda m: m.group(1) + body, text, count=1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", required=True)
    parser.add_argument("--perf-js", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--date", default=date_cls.today().isoformat())
    args = parser.parse_args()

    report = json.loads(Path(args.json).read_text())
    text = Path(args.perf_js).read_text()

    # 数据来源注释：既有页面在 PERF_META 上方有一行 `// 数据源：CI 运行 ...`
    text = re.sub(
        r"// 数据源：CI 运行 [^\n]*",
        f"// 数据源：CI 运行 {args.run_id} 的 perf-matrix-{args.run_id} "
        "artifact（fkv-matrix.md / .json）。",
        text,
        count=1,
    )
    text = replace_block(text, "PERF_META", f'"{build_meta(report, args.run_id, args.date)}";')
    rows = "[\n" + ",\n".join(build_rows(report)) + "\n];"
    text = replace_block(text, "PERF_MATRIX_ROWS", rows)
    Path(args.perf_js).write_text(text)
    print(f"updated {args.perf_js} from {args.json} (run {args.run_id})")


if __name__ == "__main__":
    main()
