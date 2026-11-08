//! 按可提交的分类清单批量归属 / 裁定 unit。
//!
//! `.data/motedb.mote` 不入库，清单 `doc/ledger-classification.tsv` 是可提交、
//! 可从空账本重放的分类声明。每行三列：
//!
//! ```text
//! unit_id<TAB>action<TAB>target_or_reason
//! ```
//!
//! - `action = map`：`target_or_reason` 为 `crate` 或 `crate:layer`；
//!   只写 `crate` / `layer`，不改状态。
//! - `action = ignore`：`target_or_reason` 为裁定原因，必须指向
//!   `COMPAT_BASELINE` / `doc/progress.md` 的范围块；写状态 `ignored` 并追加 note。
//! - `#` 开头为注释，空行跳过。
//! - 第一列支持 `\\` / `\t` / `\n` 转义（tree-sitter 的 symbol 可能含换行）。
//!
//! 幂等规则：
//! - `map` 的目标 crate/layer 已一致时计入 unchanged，不写库；
//! - `ignore` 已处于 `ignored` 时计入 unchanged，不重复追加 note；
//! - 终态 `done` / `verified` 不会被 `ignore` 降级（计入 skipped_terminal）。

use std::collections::BTreeMap;

use motedb::{Database, types::Value};

use crate::{
  error::{LedgerError, Result},
  util::{now_ns, now_sec, text_of},
};

const ACTION_MAP: &str = "map";
const ACTION_IGNORE: &str = "ignore";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifyLine {
  pub unit_id: String,
  pub action: String,
  pub target: String,
  pub line_no: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassifySummary {
  pub mapped: usize,
  pub ignored: usize,
  pub unchanged: usize,
  pub skipped_terminal: usize,
  pub missing: usize,
}

impl ClassifySummary {
  pub fn total(&self) -> usize {
    self.mapped + self.ignored + self.unchanged + self.skipped_terminal + self.missing
  }
}

#[derive(Debug, Clone)]
struct UnitState {
  crate_name: String,
  layer: String,
  status: String,
}

fn unescape(field: &str) -> Result<String> {
  let mut out = String::with_capacity(field.len());
  let mut char_iter = field.chars();
  while let Some(ch) = char_iter.next() {
    if ch != '\\' {
      out.push(ch);
      continue;
    }
    match char_iter.next() {
      Some('\\') => out.push('\\'),
      Some('t') => out.push('\t'),
      Some('n') => out.push('\n'),
      Some(other) => {
        return Err(LedgerError::Invalid(format!(
          "unknown escape `\\{other}`; expected \\\\, \\t or \\n"
        )));
      }
      None => {
        return Err(LedgerError::Invalid(
          "trailing `\\` in escaped field".to_owned(),
        ));
      }
    }
  }
  Ok(out)
}

/// 解析清单。格式错误立即报错并带行号，避免半份清单被应用。
pub fn parse(text: &str) -> Result<Vec<ClassifyLine>> {
  let mut line_li = Vec::new();
  for (idx, raw) in text.lines().enumerate() {
    let line_no = idx + 1;
    let line = raw.trim_end_matches('\r');
    if line.trim().is_empty() || line.trim_start().starts_with('#') {
      continue;
    }
    let field_li: Vec<&str> = line.split('\t').collect();
    if field_li.len() != 3 {
      return Err(LedgerError::Invalid(format!(
        "ledger-classification line {line_no}: 需要 3 列 (unit_id<TAB>action<TAB>target_or_reason)，实际 {} 列",
        field_li.len()
      )));
    }
    let unit_id = unescape(field_li[0]).map_err(|err| {
      LedgerError::Invalid(format!("ledger-classification line {line_no}: {err}"))
    })?;
    let action = field_li[1].trim();
    let target = field_li[2].trim();
    if unit_id.is_empty() {
      return Err(LedgerError::Invalid(format!(
        "ledger-classification line {line_no}: unit_id 为空"
      )));
    }
    if !matches!(action, ACTION_MAP | ACTION_IGNORE) {
      return Err(LedgerError::Invalid(format!(
        "ledger-classification line {line_no}: unknown action `{action}`, expected map|ignore"
      )));
    }
    if target.is_empty() {
      return Err(LedgerError::Invalid(format!(
        "ledger-classification line {line_no}: target_or_reason 为空"
      )));
    }
    line_li.push(ClassifyLine {
      unit_id,
      action: action.to_owned(),
      target: target.to_owned(),
      line_no,
    });
  }
  Ok(line_li)
}

fn load_states(db: &Database) -> Result<BTreeMap<String, UnitState>> {
  let result = db
    .execute("SELECT id, crate, layer, status FROM unit")?
    .materialize()?;
  let mut map = BTreeMap::new();
  if let Some((_, row_li)) = result.select_rows() {
    for row in row_li {
      let Some(id) = row.first().and_then(text_of) else {
        continue;
      };
      map.insert(
        id.to_owned(),
        UnitState {
          crate_name: row.get(1).and_then(text_of).unwrap_or_default().to_owned(),
          layer: row.get(2).and_then(text_of).unwrap_or_default().to_owned(),
          status: row.get(3).and_then(text_of).unwrap_or_default().to_owned(),
        },
      );
    }
  }
  Ok(map)
}

fn split_target(target: &str) -> (String, String) {
  match target.split_once(':') {
    Some((crate_name, layer)) => (crate_name.trim().to_owned(), layer.trim().to_owned()),
    None => (target.trim().to_owned(), String::new()),
  }
}

/// 静默版 ignore：批量分类时避免每行打印一次 `updated`。
/// 语义与 `ledger::ignore` 一致：写 `ignored`、清 verification、追加 reason note。
fn ignore_unit(db: &Database, unit_id: &str, reason: &str) -> Result<()> {
  db.execute_prepared(
    "UPDATE unit SET status = 'ignored', updated_at = ? WHERE id = ?",
    vec![Value::Integer(now_sec()), Value::text(unit_id.to_owned())],
  )?
  .materialize()?;
  db.execute_prepared(
    "DELETE FROM verification WHERE unit_id = ?",
    vec![Value::text(unit_id.to_owned())],
  )?
  .materialize()?;
  let note_id = format!("{unit_id}#ignore#{}", now_ns());
  db.execute_prepared(
    "INSERT INTO note (id, unit_id, author, body, created_at) VALUES (?, ?, ?, ?, ?)",
    vec![
      Value::text(note_id),
      Value::text(unit_id.to_owned()),
      Value::text("classify".to_owned()),
      Value::text(reason.to_owned()),
      Value::Integer(now_sec()),
    ],
  )?
  .materialize()?;
  Ok(())
}

/// 应用清单。`plan = true` 只累计计划，不写库。
pub fn apply(db: &Database, line_li: &[ClassifyLine], plan: bool) -> Result<ClassifySummary> {
  let state_map = load_states(db)?;
  let mut summary = ClassifySummary::default();
  for line in line_li {
    let Some(state) = state_map.get(&line.unit_id) else {
      eprintln!(
        "WARN: line {}: unit not found: {}",
        line.line_no, line.unit_id
      );
      summary.missing += 1;
      continue;
    };
    if line.action == ACTION_MAP {
      let (crate_name, layer) = split_target(&line.target);
      if crate_name.is_empty() || crate_name.contains(char::is_whitespace) {
        return Err(LedgerError::Invalid(format!(
          "line {}: invalid map target `{}` (want `crate` or `crate:layer`)",
          line.line_no, line.target
        )));
      }
      if state.crate_name == crate_name && state.layer == layer {
        summary.unchanged += 1;
        continue;
      }
      summary.mapped += 1;
      if plan {
        continue;
      }
      db.execute_prepared(
        "UPDATE unit SET crate = ?, layer = ?, updated_at = ? WHERE id = ?",
        vec![
          Value::text(crate_name),
          Value::text(layer),
          Value::Integer(now_sec()),
          Value::text(line.unit_id.clone()),
        ],
      )?
      .materialize()?;
      continue;
    }

    // ignore：终态不降级；已 ignored 不重复记 note。
    if matches!(state.status.as_str(), "done" | "verified") {
      summary.skipped_terminal += 1;
      continue;
    }
    if state.status == "ignored" {
      summary.unchanged += 1;
      continue;
    }
    summary.ignored += 1;
    if plan {
      continue;
    }
    ignore_unit(db, &line.unit_id, &line.target)?;
  }
  Ok(summary)
}

#[cfg(test)]
mod tests {
  use std::{env::temp_dir, fs::create_dir_all, path::PathBuf, process::id};

  use super::*;
  use crate::db::open;

  fn temp_db() -> (Database, PathBuf) {
    let root = temp_dir().join(format!("fkv_ledger_classify_{}_{}", id(), now_sec()));
    create_dir_all(&root).unwrap();
    let db = open(&root.join("ledger.mote")).unwrap();
    (db, root)
  }

  fn insert_unit(db: &Database, id: &str, status: &str) {
    db.execute_prepared(
      "INSERT INTO unit (id, path, symbol, kind, crate, layer, status, source_hash) \
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
      vec![
        Value::text(id.to_owned()),
        Value::text("include/pool.h".to_owned()),
        Value::text(id.to_owned()),
        Value::text("class_specifier".to_owned()),
        Value::text(String::new()),
        Value::text(String::new()),
        Value::text(status.to_owned()),
        Value::text("hash".to_owned()),
      ],
    )
    .unwrap()
    .materialize()
    .unwrap();
  }

  #[test]
  fn parse_supports_escaped_unit_id_and_rejects_bad_columns() {
    let text = concat!(
      "# comment\n",
      r"a\nb\tc",
      "\tmap\tfkv_buf:core\n",
      r"u2",
      "\tignore\tCOMPAT_BASELINE rev-41\n",
    );
    let line_li = parse(text).unwrap();
    assert_eq!(line_li.len(), 2);
    assert_eq!(line_li[0].unit_id, "a\nb\tc");
    assert_eq!(line_li[0].action, "map");
    assert_eq!(line_li[0].target, "fkv_buf:core");
    assert!(parse("u\tmap\n").is_err());
    assert!(parse("u\tdelete\treason\n").is_err());
  }

  #[test]
  fn apply_maps_and_ignores_idempotently() {
    let (db, root) = temp_db();
    insert_unit(&db, "u1", "todo");
    insert_unit(&db, "u2", "todo");
    insert_unit(&db, "u3", "done");
    let line_li = parse(
      "u1\tmap\tfkv_buf:core\n\
       u2\tignore\tCOMPAT_BASELINE rev-41\n\
       u3\tignore\tCOMPAT_BASELINE rev-41\n\
       missing\tmap\tfkv_buf\n",
    )
    .unwrap();
    let summary = apply(&db, &line_li, false).unwrap();
    assert_eq!(summary.mapped, 1);
    assert_eq!(summary.ignored, 1);
    assert_eq!(summary.skipped_terminal, 1);
    assert_eq!(summary.missing, 1);

    let summary = apply(&db, &line_li, false).unwrap();
    assert_eq!(summary.mapped, 0);
    assert_eq!(summary.ignored, 0);
    assert_eq!(summary.unchanged, 2);
    assert_eq!(summary.skipped_terminal, 1);
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
  }
}
