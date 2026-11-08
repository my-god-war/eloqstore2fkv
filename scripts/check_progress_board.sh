#!/usr/bin/env bash
# 核对 doc/progress.md 的「审查看板（issue/）」与 issue/ 目录实际条目数，
# 并复查 ignored 口径；有本地账本 DB 时再整文件重放 report 做 diff。
#
# 看板漂移的根因是 issue 流转（todo/new/done/reviewed/reject）后没有重跑
# `fkv_ledger report`。本脚本是 oracle workflow 与本地共用的唯一实现，
# 不要在 CI 里再内联一份同样逻辑。
#
# 用法：
#   bash scripts/check_progress_board.sh [--check] [仓库根]
# 缺省：--check + 脚本所在仓库根。
#
# 退出码：
#   0 = 看板 / ignored 口径一致（有 DB 时整文件 diff 也一致）；
#   1 = 有漂移；
#   2 = 用法错误 / 文件缺失 / cargo 运行失败。
#
# 修复：不要手改表格，重新生成整份文档：
#   cargo run -p fkv_ledger -- report --out doc/progress.md
set -euo pipefail

MODE=--check
ROOT=
for arg in "$@"; do
  case "$arg" in
    --check) MODE=$arg ;;
    -h|--help)
      sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*)
      echo "unknown option: $arg" >&2
      exit 2
      ;;
    *)
      if [ -n "$ROOT" ]; then
        echo "unexpected argument: $arg" >&2
        exit 2
      fi
      ROOT=$arg
      ;;
  esac
done
SCRIPT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ROOT=${ROOT:-$SCRIPT_ROOT}
if [ ! -d "$ROOT" ]; then
  echo "repo root not found: $ROOT" >&2
  exit 2
fi
ROOT=$(cd "$ROOT" && pwd)

if [ "$MODE" != "--check" ]; then
  echo "unknown mode: $MODE" >&2
  exit 2
fi

PROGRESS="$ROOT/doc/progress.md"
if [ ! -f "$PROGRESS" ]; then
  echo "doc/progress.md not found: $PROGRESS" >&2
  exit 2
fi

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fkv-progress-check.XXXXXX")
trap 'rm -rf "$TMP_DIR"' EXIT
ISSUES="$TMP_DIR/issues.txt"

# issue/ 目录 -> 各状态条目数。口径与 fkv_ledger::issue_board 一致：
# 只数 *.md，排除 00- 开头的索引；缺目录记 0。
if ! (cd "$SCRIPT_ROOT" && cargo run -q -p fkv_ledger -- issues --root "$ROOT") > "$ISSUES"; then
  echo "fkv_ledger issues failed" >&2
  exit 2
fi

python3 -I - "$PROGRESS" "$ISSUES" <<'PY'
import re
import sys

progress_path, issues_path = sys.argv[1], sys.argv[2]
lines = open(progress_path, encoding="utf-8").read().splitlines()


def section_table(header):
    try:
        start = lines.index(header)
    except ValueError:
        raise SystemExit(f"missing section: {header}")
    rows = []
    for line in lines[start + 1:]:
        stripped = line.strip()
        if stripped.startswith("## "):
            break
        if not stripped.startswith("|"):
            continue
        cells = [cell.strip().strip("*") for cell in stripped.strip("|").split("|")]
        if not cells or set(cells[0]) <= set("-: "):
            continue
        if cells[0] in ("目录", "status", "crate", "原因"):
            continue
        rows.append(cells)
    return rows


errors = []

expected_board = {}
with open(issues_path, encoding="utf-8") as handle:
    for line in handle:
        line = line.strip()
        if not line or line.startswith("dir "):
            continue
        name, sep, count = line.partition("|")
        if sep:
            expected_board[name.strip()] = int(count.strip())

actual_board = {cells[0]: int(cells[1]) for cells in section_table("## 审查看板（issue/）")}
if actual_board != expected_board:
    errors.append(f"审查看板 drift: progress={actual_board} issues={expected_board}")

status = {cells[0]: int(cells[1]) for cells in section_table("## 状态总览")}
ignored_all = status.get("ignored", 0)

split = None
for line in lines:
    match = re.match(
        r"^- 分流口径：todo (\d+) / ignored（([^）]*)）(\d+) / <unmapped> (\d+)；", line
    )
    if match:
        split = match
        break
if split is None:
    errors.append("缺少「分流口径」行")
else:
    todo, label, ignored_split = int(split.group(1)), split.group(2), int(split.group(3))
    if ignored_split != ignored_all:
        errors.append(
            f"分流口径 ignored={ignored_split} 与状态总览 ignored={ignored_all} 不一致"
        )
    if "全部" not in label and "无 reason" not in label:
        errors.append(f"分流口径的 ignored 未标注口径: {label!r}")
    if todo != status.get("todo", 0):
        errors.append(f"分流口径 todo={todo} 与状态总览 todo={status.get('todo', 0)} 不一致")

range_header_idx = next(
    (idx for idx, line in enumerate(lines) if line.startswith("## 有 reason 的 ignored（")),
    None,
)
if range_header_idx is None:
    errors.append("缺少「有 reason 的 ignored（N / M）」小节")
else:
    header = lines[range_header_idx]
    match = re.match(r"^## 有 reason 的 ignored（(\d+) / (\d+)）$", header)
    if match is None:
        errors.append(f"ignored 小节标题口径不完整: {header!r}")
    else:
        with_reason, total = int(match.group(1)), int(match.group(2))
        if total != ignored_all:
            errors.append(
                f"ignored 小节 M={total} 与状态总览 ignored={ignored_all} 不一致"
            )
        reason_sum = 0
        for cells in section_table(header):
            reason_sum += int(cells[1])
            if "COMPAT_BASELINE" not in cells[0]:
                errors.append(f"ignored reason 未指向 COMPAT_BASELINE: {cells[0]!r}")
        if reason_sum != with_reason:
            errors.append(
                f"ignored reason 表合计 {reason_sum} 与标题 N={with_reason} 不一致"
            )
        without_reason = None
        for line in lines[range_header_idx + 1:]:
            if line.strip().startswith("## "):
                break
            found = re.search(r"另有 (\d+) 条无该 reason", line)
            if found:
                without_reason = int(found.group(1))
                break
        if without_reason is None:
            errors.append("ignored 小节缺少无 reason 数量说明")
        elif with_reason + without_reason != total:
            errors.append(
                f"有 reason {with_reason} + 无 reason {without_reason} "
                f"!= 全部 ignored {total}"
            )

if errors:
    print("doc/progress.md drift:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    print(
        "修复：cargo run -p fkv_ledger -- report --out doc/progress.md"
        "（不要手改表格）",
        file=sys.stderr,
    )
    raise SystemExit(1)
print(f"progress ok: board={actual_board} ignored={ignored_all}")
PY

# 账本 DB（.data/）不入 git，GitHub runner 上无法整文件重放；本地有 DB 时
# 再校验整份文件（忽略生成时间），防止手改生成物。
if [ -e "$ROOT/.data/motedb.mote" ]; then
  REPORT="$TMP_DIR/progress.md"
  if ! (cd "$ROOT" && cargo run -q -p fkv_ledger -- report --out "$REPORT" >/dev/null); then
    echo "fkv_ledger report failed" >&2
    exit 2
  fi
  grep -v '^- 生成时间（UTC）：' "$REPORT" > "$TMP_DIR/report.txt"
  grep -v '^- 生成时间（UTC）：' "$PROGRESS" > "$TMP_DIR/progress.txt"
  if ! diff -u "$TMP_DIR/report.txt" "$TMP_DIR/progress.txt"; then
    echo "doc/progress.md 与 report 生成物不一致；请运行 cargo run -p fkv_ledger -- report --out doc/progress.md" >&2
    exit 1
  fi
  echo "report diff ok: doc/progress.md 与 report 生成物一致（忽略生成时间）"
else
  echo "跳过整文件 diff：.data/motedb.mote 不存在（账本 DB 不入 git）"
fi
