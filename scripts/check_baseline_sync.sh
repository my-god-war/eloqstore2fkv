#!/usr/bin/env bash
# 核对「基线快照」是否与仓库根 COMPAT_BASELINE 同步：
#   1. fixtures/{cpp,rust}/**/meta.txt 的 `baseline=` 行（压行分隔符 " | "）；
#   2. doc/progress.md 的 `- 兼容基线：` 行（report 的压行分隔符 " / "）。
#
# 用法：
#   bash scripts/check_baseline_sync.sh [--check|--fix] [仓库根]
# 缺省：--check + 脚本所在仓库根。
# 注意：--fix 只原地刷新基线文本；progress.md 的生成时间 / 看板计数仍以
#       `cargo run -p fkv_ledger -- report --out doc/progress.md` 为准。
#       看板与 issue/ 目录的一致性用 `bash scripts/check_progress_board.sh --check`。
#
# 退出码：
#   0 = 全部同步（--fix 时表示已刷新并复查通过）；
#   1 = 有漂移；
#   2 = 用法错误 / 文件缺失 / COMPAT_BASELINE 为空。
set -euo pipefail

MODE=--check
ROOT=
for arg in "$@"; do
  case "$arg" in
    --check|--fix) MODE=$arg ;;
    -h|--help)
      sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
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
ROOT=${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}

python3 -I - "$MODE" "$ROOT" <<'PY'
import pathlib
import sys

mode, root_arg = sys.argv[1], sys.argv[2]
root = pathlib.Path(root_arg).resolve()

if mode not in ("--check", "--fix"):
    print(f"unknown mode: {mode}", file=sys.stderr)
    raise SystemExit(2)

baseline_path = root / "COMPAT_BASELINE"
if not baseline_path.is_file():
    print(f"COMPAT_BASELINE not found: {baseline_path}", file=sys.stderr)
    raise SystemExit(2)

baseline_lines = [
    line.strip()
    for line in baseline_path.read_text(encoding="utf-8").splitlines()
    if line.strip()
]
if not baseline_lines:
    print("COMPAT_BASELINE is empty", file=sys.stderr)
    raise SystemExit(2)

# meta 的压行规则见 tool/fkv_ledger/src/fixture.rs:baseline_text。
meta_baseline = " | ".join(baseline_lines)
# report 的压行规则见 tool/fkv_ledger/src/ledger.rs:build_report。
progress_baseline = " / ".join(baseline_lines)

meta_li = sorted((root / "fixtures").rglob("meta.txt"))
if not meta_li:
    print("no fixtures/**/meta.txt found", file=sys.stderr)
    raise SystemExit(2)

PROGRESS_PREFIX = "- 兼容基线："
progress_path = root / "doc" / "progress.md"
if not progress_path.is_file():
    print(f"doc/progress.md not found: {progress_path}", file=sys.stderr)
    raise SystemExit(2)
progress_lines = progress_path.read_text(encoding="utf-8").splitlines()


def meta_baseline_of(path):
    values = [
        line[len("baseline="):]
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.startswith("baseline=")
    ]
    if len(values) != 1:
        return None, f"expected exactly one baseline= line, found {len(values)}"
    return values[0], None


def progress_baseline_of(lines):
    values = [
        line[len(PROGRESS_PREFIX):]
        for line in lines
        if line.startswith(PROGRESS_PREFIX)
    ]
    if len(values) != 1:
        return None, f"expected exactly one '{PROGRESS_PREFIX}' line, found {len(values)}"
    return values[0], None


def write_meta_baseline(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    out = []
    changed = 0
    for line in lines:
        if line.startswith("baseline="):
            out.append("baseline=" + meta_baseline)
            changed += 1
        else:
            out.append(line)
    if changed != 1:
        raise SystemExit(f"{path}: expected exactly one baseline= line, found {changed}")
    path.write_text("\n".join(out) + "\n", encoding="utf-8")


def write_progress_baseline(path, lines):
    out = []
    changed = 0
    for line in lines:
        if line.startswith(PROGRESS_PREFIX):
            out.append(PROGRESS_PREFIX + progress_baseline)
            changed += 1
        else:
            out.append(line)
    if changed != 1:
        raise SystemExit(f"{path}: expected exactly one '{PROGRESS_PREFIX}' line, found {changed}")
    path.write_text("\n".join(out) + "\n", encoding="utf-8")


def scan():
    mismatches = []
    for path in meta_li:
        value, error = meta_baseline_of(path)
        if error is not None:
            mismatches.append((path, error))
        elif value != meta_baseline:
            mismatches.append((path, f"baseline mismatch (len {len(value)} != {len(meta_baseline)})"))

    value, error = progress_baseline_of(progress_lines)
    if error is not None:
        progress_error = error
    elif value != progress_baseline:
        progress_error = f"baseline mismatch (len {len(value)} != {len(progress_baseline)})"
    else:
        progress_error = None
    return mismatches, progress_error


mismatches, progress_error = scan()
count = len(meta_li)

if mode == "--fix" and (mismatches or progress_error):
    for path, _ in mismatches:
        if meta_baseline_of(path)[1] is None:
            write_meta_baseline(path)
    if progress_error is not None:
        if progress_baseline_of(progress_lines)[1] is None:
            write_progress_baseline(progress_path, progress_lines)
    # 复查，确保 --fix 真正收敛。
    progress_lines = progress_path.read_text(encoding="utf-8").splitlines()
    mismatches, progress_error = scan()

if mismatches or progress_error:
    for path, error in mismatches:
        print(f"MISMATCH {path.relative_to(root)}: {error}", file=sys.stderr)
    if progress_error is not None:
        print(f"MISMATCH doc/progress.md: {progress_error}", file=sys.stderr)
    print(
        f"baseline drift: {len(mismatches)}/{count} meta.txt + "
        f"doc/progress.md {'drift' if progress_error else 'ok'};"
        " run `bash scripts/check_baseline_sync.sh --fix` or regenerate fixtures",
        file=sys.stderr,
    )
    raise SystemExit(1)

cpp = sum(1 for path in meta_li if path.relative_to(root).parts[1] == "cpp")
rust = count - cpp
print(f"baseline sync ok: {count}/{count} meta.txt (cpp {cpp} / rust {rust}), doc/progress.md ok, {len(meta_baseline)} chars")
PY
