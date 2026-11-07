#!/usr/bin/env bash
# 核对 doc/progress.md 的「状态总览」「按 crate」与账本实际输出一致。
#
# 用法：
#   bash scripts/check_ledger_progress.sh [progress.md] [db路径]
# 缺省：doc/progress.md + .data/motedb.mote（可用 FKV_LEDGER_DB 覆盖）。
#
# 退出码：
#   0 = 表格与账本一致；1 = 有漂移或读取失败。
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PROGRESS=${1:-"$ROOT/doc/progress.md"}
DB=${2:-"${FKV_LEDGER_DB:-$ROOT/.data/motedb.mote}"}

if [ ! -f "$PROGRESS" ]; then
  echo "progress not found: $PROGRESS" >&2
  exit 1
fi
if [ ! -e "$DB" ]; then
  echo "ledger db not found: $DB（先跑 sync + classify）" >&2
  exit 1
fi

TMP=$(mktemp "${TMPDIR:-/tmp}/fkv-ledger-export.XXXXXX.json")
trap 'rm -f "$TMP"' EXIT

cd "$ROOT"
cargo run -q -p fkv_ledger -- export --db "$DB" --out "$TMP" >/dev/null

python3 - "$PROGRESS" "$TMP" <<'PY'
import json
import re
import sys

progress_path, export_path = sys.argv[1], sys.argv[2]
lines = open(progress_path, encoding="utf-8").read().splitlines()
data = json.load(open(export_path, encoding="utf-8"))
summary = data["summary"]

def parse_table(header):
    """返回紧跟在 `header` 后的第一张 markdown 表的 {首列: [其余列]}。"""
    start = None
    for idx, line in enumerate(lines):
        if line.strip() == header:
            start = idx + 1
            break
    if start is None:
        raise SystemExit(f"missing section: {header}")
    rows = {}
    for line in lines[start:]:
        stripped = line.strip()
        if stripped.startswith("## "):
            break
        if not stripped.startswith("|"):
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if not cells or set(cells[0]) <= set("-: "):
            continue
        if cells[0] in ("status", "crate", "原因"):
            continue
        rows[cells[0].strip("*")] = cells[1:]
    return rows

errors = []

status_rows = parse_table("## 状态总览")
expected_status = dict(summary["by_status"])
expected_status["total"] = summary["total"]
for key, expected in expected_status.items():
    row = status_rows.get(key)
    if row is None:
        errors.append(f"状态总览缺少 {key}")
        continue
    try:
        actual = int(row[0].strip("*"))
    except ValueError:
        errors.append(f"状态总览 {key} 不是整数: {row[0]}")
        continue
    if actual != expected:
        errors.append(f"状态总览 {key}: progress={actual} ledger={expected}")
for key in status_rows:
    value = status_rows[key][0].strip("*")
    if key not in expected_status and value not in ("0", ""):
        errors.append(f"状态总览出现账本没有的状态 {key}={value}")

crate_rows = parse_table("## 按 crate")
expected_crate = {}
for crate_name, stat in summary["by_crate"].items():
    name = crate_name if crate_name else "<unmapped>"
    expected_crate[name] = [str(stat["total"]), str(stat["todo"]), str(stat["done"]), str(stat["verified"])]
for name, expected in expected_crate.items():
    row = crate_rows.get(name)
    if row is None:
        errors.append(f"按 crate 缺少 {name}")
        continue
    if row[:4] != expected:
        errors.append(f"按 crate {name}: progress={row[:4]} ledger={expected}")
for name in crate_rows:
    if name not in expected_crate:
        errors.append(f"按 crate 出现账本没有的 crate {name}")

reason_rows = parse_table("## 范围外裁定")
ignored_sum = 0
for reason, row in reason_rows.items():
    try:
        count = int(row[0].strip("*"))
    except (ValueError, IndexError):
        errors.append(f"范围外裁定 {reason!r} 数量不是整数")
        continue
    ignored_sum += count
    if "COMPAT_BASELINE" not in reason:
        errors.append(f"范围外裁定原因未指向 COMPAT_BASELINE: {reason}")
if ignored_sum != summary["by_status"].get("ignored", 0):
    errors.append(
        f"范围外裁定合计 {ignored_sum} 与 status ignored "
        f"{summary['by_status'].get('ignored', 0)} 不一致（note 缺失或 author 不是 classify）"
    )

m = re.search(r"分流口径：todo (\d+) / ignored（范围外裁定）(\d+) / <unmapped> (\d+)", "\n".join(lines))
if not m:
    errors.append("缺少「分流口径」行")
else:
    todo, ignored, unmapped = (int(x) for x in m.groups())
    if todo != summary["by_status"].get("todo", 0):
        errors.append(f"分流口径 todo={todo} 与账本 {summary['by_status'].get('todo', 0)} 不一致")
    if ignored != summary["by_status"].get("ignored", 0):
        errors.append(f"分流口径 ignored={ignored} 与账本 {summary['by_status'].get('ignored', 0)} 不一致")
    if unmapped != summary["by_crate"].get("", {}).get("total", 0):
        errors.append(f"分流口径 <unmapped>={unmapped} 与账本不一致")

if errors:
    print("progress/ledger drift:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    raise SystemExit(1)

print(
    "progress 与账本一致："
    f"total={summary['total']} todo={summary['by_status'].get('todo', 0)} "
    f"ignored={summary['by_status'].get('ignored', 0)} "
    f"unmapped={summary['by_crate'].get('', {}).get('total', 0)}"
)
PY
