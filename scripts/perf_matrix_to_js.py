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


def fmt_ops(result: dict) -> str:
    """该阶段的操作数/秒；单位不逐格写（表下一行说明）。"""
    value = result["throughput"]
    return f"{value:.0f}" if value >= 1 else f"{value:.2f}"


def cell(result: dict | None) -> str:
    return "—" if result is None else fmt_ops(result)


def ops_ratio(row: dict) -> float | None:
    """ratio = fkv 操作数 / C++ 操作数，> 1 表示 fkv 更快。"""
    cpp = row.get("cpp")
    fkv = row.get("fkv")
    if not cpp or not fkv or cpp["throughput"] <= 0:
        return None
    return fkv["throughput"] / cpp["throughput"]


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


def build_rows(report: dict) -> tuple[list[str], bool]:
    """行格式：[阶段, fkv, C++, RocksDB, fkv/C++, fkv/RocksDB, tone?]。"""
    rows: list[str] = []
    has_rocksdb = any(row.get("rocksdb") for row in report["phases"])
    phases = sorted(report["phases"], key=lambda row: _order_key(row["phase"]))
    for row in phases:
        label = zh_label(row["phase"])
        fkv = cell(row.get("fkv"))
        cpp = cell(row.get("cpp"))
        rocks = cell(row.get("rocksdb"))
        ratio = ops_ratio(row)
        rocks_ratio = row.get("ratio_rocksdb")
        tone = ""
        if row["phase"] in THROUGHPUT_PHASES and ratio is not None and ratio < BAD_RATIO_BELOW:
            tone = ', "bad"'
        rows.append(
            f'  ["{label}", "{fkv}", "{cpp}", "{rocks}", '
            f'{fmt_ratio(ratio)}, {fmt_ratio(rocks_ratio)}{tone}]'
        )

    for label, key in (("未压实体积", "uncompacted_size"), ("压实后体积", "compacted_size")):
        block = report[key]
        rows.append(
            f'  ["{label}", "{size_cell(block.get("fkv"))}", '
            f'"{size_cell(block.get("cpp"))}", "{size_cell(block.get("rocksdb"))}", '
            f'{fmt_ratio(block.get("ratio"))}, {fmt_ratio(block.get("ratio_rocksdb"))}]'
        )

    cpp_secs = report["compaction"].get("cpp_secs")
    fkv_secs = report["compaction"].get("fkv_secs")
    rocks_secs = report["compaction"].get("rocksdb_secs")
    fkv_cell = "—" if fkv_secs is None else f"{fkv_secs:.3f} s"
    cpp_cell = "—" if cpp_secs is None else f"{cpp_secs:.3f} s"
    rocks_cell = "—" if rocks_secs is None else f"{rocks_secs:.3f} s"
    rows.append(f'  ["压实耗时", "{fkv_cell}", "{cpp_cell}", "{rocks_cell}", null, null]')
    return rows, has_rocksdb


def build_config(report: dict, run_id: str, date: str, has_rocksdb: bool) -> str:
    """表下三块配置：机器 / 测试负载 / RocksDB 列（各自成块，不挤成一行）。"""
    cfg = report["config"]
    rows = cfg["rows"]
    data_mib = rows * cfg["kv_size"] / (1024 * 1024)
    cache_mib = cfg["cache_bytes"] / (1024 * 1024)
    ratio = data_mib / max(cache_mib, 1.0)
    readers = "/".join(str(r) for r in cfg["readers"])
    blocks = [
        ("机器", [
            ["runner", "GitHub Actions ubuntu-latest (x86_64)"],
            ["页大小", "4 KiB"],
            ["日期", date],
            ["来源", f"CI run {run_id}"],
            ["口径", "三个引擎同机、同轮次、同一份数据形状"],
        ]),
        ("测试负载", [
            ["数据量", f"{rows:,} 行 × {cfg['kv_size']} B ≈ {data_mib:.0f} MiB"],
            ["缓存/引擎", f"{cache_mib:.0f} MiB（数据 ≈ {ratio:.1f}× 缓存）"],
            ["key / value", f"{cfg['key_size']} B / {cfg['kv_size'] - cfg['key_size']} B（db_bench 默认）"],
            ["点读", f"{cfg['reads']:,} 次（并发 {readers}）"],
            ["范围读", f"{cfg['scans']:,} 次 × {cfg['scan_len']} 条"],
            ["删除", f"{cfg['removals']:,} 条"],
            ["采样", f"每阶段 {max(cfg['samples'], 1)} 次"],
            ["分区", "单分区（EloqStore num_threads=1）"],
        ]),
        ("RocksDB 列", ([
            ["版本", "CI 系统包 librocksdb-dev"],
            ["block cache", f"{cache_mib:.0f} MiB LRU（与两侧缓存同额）"],
            ["block size", "4 KiB"],
            ["index/filter", "cache_index_and_filter_blocks=true（同池）"],
            ["压缩", "kNoCompression（三侧都不压缩）"],
            ["持久化", "WAL 开、WriteOptions.sync=false（RocksDB 默认）"],
            ["memtable", "write_buffer_size 64 MiB"],
            ["注意", "EloqStore 与 fkv 每批固定 fsync，写阶段多付同步"],
        ] if has_rocksdb else [["状态", "本次未启用（--rocksdb-lib 未加载）"]])),
    ]
    body = "{\n"
    for title, item_li in blocks:
        body += f"  {json.dumps(title, ensure_ascii=False)}: [\n"
        for name, value in item_li:
            body += (
                f"    [{json.dumps(name, ensure_ascii=False)}, "
                f"{json.dumps(value, ensure_ascii=False)}],\n"
            )
        body += "  ],\n"
    body += "}"
    return body


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
    # 支持数组 / 对象 / 字符串 / 布尔四种块。
    pattern = re.compile(
        r"(export const "
        + re.escape(name)
        + r" =\s*)(?:\[[\s\S]*?\];|\{[\s\S]*?\};|\"(?:[^\"\\]|\\.)*\";|true;|false;)",
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
    row_li, has_rocksdb = build_rows(report)
    rows = "[\n" + ",\n".join(row_li) + "\n];"
    text = replace_block(text, "PERF_MATRIX_ROWS", rows)
    text = replace_block(
        text, "PERF_HAS_ROCKSDB", "true;" if has_rocksdb else "false;"
    )
    text = replace_block(
        text, "PERF_CONFIG", build_config(report, args.run_id, args.date, has_rocksdb)
    )
    Path(args.perf_js).write_text(text)
    print(f"updated {args.perf_js} from {args.json} (run {args.run_id})")


if __name__ == "__main__":
    main()
