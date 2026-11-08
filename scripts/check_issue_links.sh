#!/usr/bin/env bash
# 校验账本 / 文档里的 issue 路径引用：
#   提取 COMPAT_BASELINE、doc/**/*.md、fixtures/**/meta.txt、issue/**/*.md 中的
#   `issue/(done|new|reviewed|reject)/<name>.md`，要求目标文件存在，且引用的
#   状态目录与文件实际所在目录一致（done→reviewed 等移动后不跟随即失败）。
#
# 用法：
#   bash scripts/check_issue_links.sh [--check|--fix] [仓库根]
# 缺省：--check + 脚本所在仓库根。
#   --fix  把文件实际在另一个状态目录的引用（只改 done/new/reviewed/reject
#          前缀，不改文件名）原位改写；仍缺失的引用不猜，保持失败。
#
# 说明：文件名含省略号（如 `io-08-...md`）视为正文占位符，不参与校验。
#
# 退出码：
#   0 = 全部引用可打开且状态一致（--fix 时表示已收敛）；
#   1 = 有缺失 / 状态错位；
#   2 = 用法错误 / 文件缺失。
set -euo pipefail

MODE=--check
ROOT=
for arg in "$@"; do
  case "$arg" in
    --check|--fix) MODE=$arg ;;
    -h|--help)
      sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
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
import re
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

STATE_DIRS = ("done", "new", "reviewed", "reject")
# 覆盖两种写法：`issue/<state>/<name>.md` 与相对写法 `../<state>/<name>.md`
# （早期轮次索引表的 `../done/...` 形式——此前不被正则覆盖，是 doc-17 的门禁盲区）。
LINK_RE = re.compile(
    r"(?:issue/|\.\./)(done|new|reviewed|reject)/([A-Za-z0-9][A-Za-z0-9_.\-]*\.md)"
)


def target_files():
    files = [baseline_path]
    files.extend(sorted((root / "doc").rglob("*.md")))
    files.extend(sorted((root / "fixtures").rglob("meta.txt")))
    files.extend(sorted((root / "issue").rglob("*.md")))
    seen = set()
    for path in files:
        if path.is_file() and path not in seen:
            seen.add(path)
            yield path


def resolve(name):
    return [state for state in STATE_DIRS if (root / "issue" / state / name).is_file()]


def scan():
    refs = 0
    missing = []
    wrong = []
    skipped = 0
    for path in target_files():
        lines = path.read_text(encoding="utf-8").splitlines()
        for lineno, line in enumerate(lines, 1):
            for match in LINK_RE.finditer(line):
                state, name = match.group(1), match.group(2)
                if "..." in name:
                    skipped += 1
                    continue
                refs += 1
                if (root / "issue" / state / name).is_file():
                    continue
                others = resolve(name)
                if len(others) == 1:
                    wrong.append((path, lineno, state, name, others[0]))
                else:
                    missing.append((path, lineno, state, name, others))
    return refs, missing, wrong, skipped


def fix(wrong):
    rewritten = 0
    by_file = {}
    for record in wrong:
        by_file.setdefault(record[0], []).append(record)

    for path, items in by_file.items():
        lines = path.read_text(encoding="utf-8").splitlines()
        changed = 0
        for _path, lineno, state, name, actual in items:
            old = f"issue/{state}/{name}"
            new = f"issue/{actual}/{name}"
            index = lineno - 1
            if old in lines[index]:
                lines[index] = lines[index].replace(old, new)
                changed += 1
        if changed:
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
            rewritten += changed
    return rewritten


refs, missing, wrong, skipped = scan()
if mode == "--fix" and wrong:
    rewritten = fix(wrong)
    refs, missing, wrong, skipped = scan()
    print(f"rewrote {rewritten} issue link(s) to their actual state directory")

if missing or wrong:
    for path, lineno, state, name, others in missing:
        actual = f" (found in {', '.join(others)})" if others else ""
        print(f"MISSING {path.relative_to(root)}:{lineno}: "
              f"issue/{state}/{name}{actual}", file=sys.stderr)
    for path, lineno, state, name, actual in wrong:
        print(f"WRONG-STATE {path.relative_to(root)}:{lineno}: "
              f"issue/{state}/{name} (actual: issue/{actual}/{name})", file=sys.stderr)
    print(
        f"issue link check failed: {len(missing)} missing / {len(wrong)} wrong-state "
        f"of {refs} reference(s); run `bash scripts/check_issue_links.sh --fix` "
        "for state-directory moves (missing targets need manual edits)",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"issue links ok: {refs} reference(s), 0 missing / 0 wrong-state"
      + (f", {skipped} placeholder(s) skipped" if skipped else ""))
PY
