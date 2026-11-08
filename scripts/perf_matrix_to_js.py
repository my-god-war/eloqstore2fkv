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


def fmt_elapsed(secs: float) -> str:
    """阶段墙钟耗时（毫秒），与 redb benchmark 表的耗时口径一致。"""
    ms = secs * 1000.0
    if ms >= 100:
        return f"{ms:.0f}ms"
    if ms >= 1:
        return f"{ms:.1f}ms"
    return f"{ms:.2f}ms"


def cell(result: dict | None) -> str:
    if result is None:
        return "—"
    return fmt_elapsed(result["duration"])


def elapsed_ratio(row: dict) -> float | None:
    """耗时口径下 ratio = C++ 耗时 / fkv 耗时，> 1 表示 fkv 更快。"""
    cpp = row.get("cpp")
    fkv = row.get("fkv")
    if not cpp or not fkv:
        return None
    fkv_secs = fkv["duration"]
    if fkv_secs <= 0:
        return None
    return cpp["duration"] / fkv_secs


def fmt_bytes(value: int) -> str:
    mib = value / (1024 * 1024)
    if mib >= 1024:
        return f"{mib / 1024:.2f} GiB"
    return f"{mib:.2f} MiB"


def fmt_ratio(value: float | None) -> str:
    return "null" if value is None else f'"{value:.3f}x"'


def size_cell(value: int | None) -> str:
    return "—" if value is None else fmt_bytes(value)


# redb benchmark 表的行序与中文名（阶段 -> 中文）
ZH_LABELS = {
    "bulk load": "批量加载",
    "individual writes": "单条写入",
    "batch writes": "批量写",
    "len()": "len()",
    "random range reads": "随机范围读",
    "removals": "删除",
    "uncompacted size": "未压实体积",
    "compacted size": "压实后体积",
    "compact wall time": "压实耗时",
}

# 与 redb 的表同序：写三类 -> len() -> 各并发点读 -> 范围读 -> 删除 -> 体积。
_ORDER = ["bulk load", "individual writes", "batch writes", "len()",
          "random range reads", "removals"]


def _order_key(label: str) -> tuple[int, int]:
    if label.startswith("random reads ("):
        threads = "".join(ch for ch in label if ch.isdigit())
        return (3, int(threads or 0))
    if label in _ORDER:
        return (_ORDER.index(label), 0)
    return (99, 0)


def zh_label(label: str) -> str:
    if label in ZH_LABELS:
        return ZH_LABELS[label]
    if label.startswith("random reads ("):
        threads = "".join(ch for ch in label if ch.isdigit())
        return f"随机读（{threads} 线程）"
    return label


def build_rows(report: dict) -> list[str]:
    rows: list[str] = []
    phases = sorted(report["phases"], key=lambda row: _order_key(row["phase"]))
    for row in phases:
        label = zh_label(row["phase"])
        cpp = cell(row.get("cpp"))
        fkv = cell(row.get("fkv"))
        ratio = elapsed_ratio(row)
        tone = ""
        if row["phase"] in THROUGHPUT_PHASES and ratio is not None and ratio < BAD_RATIO_BELOW:
            tone = ', "bad"'
        rows.append(
            f'  ["{label}", "{cpp}", "{fkv}", {fmt_ratio(ratio)}{tone}]'
        )

    for label, key in (("未压实体积", "uncompacted_size"),
                       ("压实后体积", "compacted_size")):
        block = report[key]
        rows.append(
            f'  ["{label}", "{size_cell(block["cpp"])}", '
            f'"{size_cell(block["fkv"])}", {fmt_ratio(block.get("ratio"))}]'
        )

    cpp_secs = report["compaction"].get("cpp_secs")
    fkv_secs = report["compaction"].get("fkv_secs")
    cpp_cell = "—" if cpp_secs is None else f"{cpp_secs:.3f} s"
    fkv_cell = "—" if fkv_secs is None else f"{fkv_secs:.3f} s"
    rows.append(f'  ["压实耗时", "{cpp_cell}", "{fkv_cell}", null]')
    return rows


def build_meta(report: dict, run_id: str, date: str) -> str:
    """首页标题行：口径按 RocksDB 官方评测（数据 ≫ 缓存）。"""
    cfg = report["config"]
    readers = "/".join(str(r) for r in cfg["readers"])
    rows = cfg["rows"]
    data_mib = rows * cfg["kv_size"] / (1024 * 1024)
    cache_mib = cfg["cache_bytes"] / (1024 * 1024)
    ratio = data_mib / max(cache_mib, 1.0)
    return (
        "redb 阶段矩阵 · C++ EloqStore vs Rust fkv · "
        f"{date} · GitHub Actions ubuntu-latest (x86_64, 4 KiB 页) · "
        f"{rows} 行 × {cfg['kv_size']} B ≈ {data_mib:.0f} MiB 数据 / "
        f"{cache_mib:.0f} MiB 缓存（≈{ratio:.0f}:1，对齐 RocksDB 官方 97 GiB/6 GiB）· "
        f"readers {readers} · 每阶段 1 次采样"
    )


def replace_block(text: str, name: str, body: str) -> str:
    """把 `export const <name> = ...;` 整块替换成 `body`（其余内容原样保留）。"""
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
