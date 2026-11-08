//! 批量把账本 unit 状态对齐到真实实现进度。
//!
//! 数据库 `.data/motedb.mote` 不入 git；`doc/adopt.tsv` 是可提交的进度声明：
//! 每行 `crate  status  evidence路径`，声明「该 crate 对应的 C++ unit 已迁到
//! 什么状态、证据在哪」。DB 重建后 `sync` + `adopt` + `report` 即可复现全部状态。
//!
//! 规则：
//! - 只推进 `todo / claimed / in_progress / review / blocked` 且当前状态不等于
//!   目标状态的 unit；`done / verified / ignored` 一律不动（不降级、不重复打证据）。
//! - `verified` 需要逐 unit 的兼容证据，禁止批量声明。
//! - 幂等：目标状态与当前状态相同的 unit 不计入 advanced，重放第二次起恒为 0。
//! - crate 无任何 unit 时打 WARN（多半是拼写错误或未映射），不中断重放链。

use std::collections::BTreeMap;

use motedb::{Database, types::Value};

use crate::{
  error::{LedgerError, Result},
  util::{now_ns, now_sec, text_of},
};

/// 允许被 adopt 推进的来源状态（终态 done/verified/ignored 不可再动）。
const ADVANCEABLE_LI: &[&str] = &["todo", "claimed", "in_progress", "review", "blocked"];

/// adopt.tsv 允许声明的目标状态（verified 必须逐 unit 走 `status` 命令）。
const TARGET_LI: &[&str] = &["done", "review", "in_progress", "blocked", "ignored"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptLine {
  pub crate_name: String,
  pub status: String,
  pub evidence_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct AdoptSummary {
  pub crate_name: String,
  pub total: usize,
  pub advanced: usize,
  pub evidence_added: usize,
}

/// 解析 adopt.tsv：`#` 注释、空行跳过；列 = crate [status] [evidence路径]。
/// 多余列与重复 crate 都是笔误，直接报错（带行号）。
pub fn parse(text: &str) -> Result<Vec<AdoptLine>> {
  let mut line_li = Vec::new();
  let mut line_no_map: BTreeMap<String, usize> = BTreeMap::new();
  for (idx, raw) in text.lines().enumerate() {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    let mut part_iter = line.split_whitespace();
    let Some(crate_name) = part_iter.next() else {
      continue;
    };
    let status = part_iter.next().unwrap_or("done");
    let evidence_path = part_iter.next().unwrap_or_default();
    if let Some(extra) = part_iter.next() {
      return Err(LedgerError::Invalid(format!(
        "adopt.tsv line {}: 多余列 {extra}；格式 = crate [status] [evidence路径]",
        idx + 1
      )));
    }
    if status == "verified" {
      return Err(LedgerError::Invalid(format!(
        "adopt.tsv line {}: verified 需逐 unit 兼容证据，不支持批量；用 `status <unit> verified`",
        idx + 1
      )));
    }
    if !TARGET_LI.contains(&status) {
      return Err(LedgerError::Invalid(format!(
        "adopt.tsv line {}: unknown status {status}, expected one of {TARGET_LI:?}",
        idx + 1
      )));
    }
    if let Some(prev) = line_no_map.insert(crate_name.to_owned(), idx + 1)
      && prev != idx + 1
    {
      return Err(LedgerError::Invalid(format!(
        "adopt.tsv line {}: crate {crate_name} 重复声明（首个在第 {prev} 行）",
        idx + 1
      )));
    }
    line_li.push(AdoptLine {
      crate_name: crate_name.to_owned(),
      status: status.to_owned(),
      evidence_path: evidence_path.to_owned(),
    });
  }
  Ok(line_li)
}

fn crate_unit_li(db: &Database, crate_name: &str) -> Result<Vec<(String, String)>> {
  let result = db
    .execute_prepared(
      "SELECT id, status FROM unit WHERE crate = ?",
      vec![Value::text(crate_name.to_owned())],
    )?
    .materialize()?;
  let mut unit_li = Vec::new();
  if let Some((_, rows)) = result.select_rows() {
    for row in rows {
      let Some(id) = row.first().and_then(text_of) else {
        continue;
      };
      let status = row.get(1).and_then(text_of).unwrap_or_default();
      unit_li.push((id.to_owned(), status.to_owned()));
    }
  }
  Ok(unit_li)
}

fn has_pass_evidence(db: &Database, unit_id: &str, path: &str) -> Result<bool> {
  let result = db
    .execute_prepared(
      "SELECT COUNT(*) FROM evidence WHERE unit_id = ? AND path = ? AND result = 'pass'",
      vec![
        Value::text(unit_id.to_owned()),
        Value::text(path.to_owned()),
      ],
    )?
    .materialize()?;
  Ok(
    result
      .select_rows()
      .and_then(|(_, rows)| rows.first())
      .and_then(|row| row.first())
      .is_some_and(|value| match value {
        Value::Integer(count) => *count > 0,
        _ => false,
      }),
  )
}

/// 按声明推进状态并补证据；返回逐 crate 汇总。
/// 失败时回滚当前 crate 的事务再上抛，避免「状态已改、证据丢失」的半对齐。
pub fn apply(db: &Database, line_li: &[AdoptLine], dry_run: bool) -> Result<Vec<AdoptSummary>> {
  let mut summary_li = Vec::new();
  for line in line_li {
    let unit_li = crate_unit_li(db, &line.crate_name)?;
    if unit_li.is_empty() {
      eprintln!(
        "WARN: adopt: crate {} 无任何 unit（拼写错误或尚未映射），跳过",
        line.crate_name
      );
    }
    // 目标状态与当前状态相同的 unit 不动，保证对非终态目标（in_progress 等）也幂等。
    let target_li: Vec<String> = unit_li
      .iter()
      .filter(|(_, status)| ADVANCEABLE_LI.contains(&status.as_str()) && status != &line.status)
      .map(|(id, _)| id.clone())
      .collect();
    let mut evidence_added = 0;
    if !dry_run {
      let tx = db.begin_transaction()?;
      let work = (|| -> Result<()> {
        for unit_id in &target_li {
          db.execute_prepared(
            "UPDATE unit SET status = ?, updated_at = ? WHERE id = ?",
            vec![
              Value::text(line.status.clone()),
              Value::Integer(now_sec()),
              Value::text(unit_id.clone()),
            ],
          )?
          .materialize()?;
          if !line.evidence_path.is_empty() && !has_pass_evidence(db, unit_id, &line.evidence_path)?
          {
            db.execute_prepared(
              "INSERT INTO evidence (id, unit_id, kind, path, result, commit_id, created_at) \
               VALUES (?, ?, ?, ?, ?, ?, ?)",
              vec![
                Value::text(format!("{unit_id}#adopt#{}", now_ns())),
                Value::text(unit_id.clone()),
                Value::text("test".to_owned()),
                Value::text(line.evidence_path.clone()),
                Value::text("pass".to_owned()),
                Value::text("".to_owned()),
                Value::Integer(now_sec()),
              ],
            )?
            .materialize()?;
            evidence_added += 1;
          }
        }
        Ok(())
      })();
      if let Err(err) = work {
        let _ = db.rollback_transaction(tx);
        return Err(err);
      }
      db.commit_transaction(tx)?;
    } else {
      // plan 模式：只预览将补的证据数，不开事务。
      for unit_id in &target_li {
        if !line.evidence_path.is_empty() && !has_pass_evidence(db, unit_id, &line.evidence_path)? {
          evidence_added += 1;
        }
      }
    }
    summary_li.push(AdoptSummary {
      crate_name: line.crate_name.clone(),
      total: unit_li.len(),
      advanced: target_li.len(),
      evidence_added,
    });
  }
  Ok(summary_li)
}

#[cfg(test)]
mod tests {
  use std::{
    env::temp_dir,
    fs::{create_dir_all, remove_dir_all},
    path::PathBuf,
    process::id,
    sync::atomic::{AtomicU64, Ordering},
  };

  use super::*;
  use crate::db::open;

  fn temp_db() -> (Database, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = temp_dir().join(format!("fkv_adopt_{}_{}_{}", id(), now_sec(), seq));
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
        Value::text("src/a.cpp".to_owned()),
        Value::text("a".to_owned()),
        Value::text("function".to_owned()),
        Value::text("fkv_page".to_owned()),
        Value::text("format".to_owned()),
        Value::text(status.to_owned()),
        Value::text("h".to_owned()),
      ],
    )
    .unwrap()
    .materialize()
    .unwrap();
  }

  fn unit_status(db: &Database, id: &str) -> String {
    let result = db
      .execute_prepared(
        "SELECT status FROM unit WHERE id = ?",
        vec![Value::text(id.to_owned())],
      )
      .unwrap()
      .materialize()
      .unwrap();
    result
      .select_rows()
      .and_then(|(_, rows)| rows.first())
      .and_then(|row| row.first())
      .and_then(text_of)
      .unwrap_or_default()
      .to_owned()
  }

  #[test]
  fn parse_skips_comments_and_defaults_status() {
    let line_li =
      parse("# comment\n\nfkv_page done crate/format/fkv_page/tests\nfkv_btree\n").unwrap();
    assert_eq!(line_li.len(), 2);
    assert_eq!(line_li[0].status, "done");
    assert_eq!(line_li[0].evidence_path, "crate/format/fkv_page/tests");
    assert_eq!(line_li[1].status, "done");
    assert_eq!(line_li[1].evidence_path, "");
    assert!(parse("fkv_page verified x").is_err());
    assert!(parse("fkv_page bogus x").is_err());
    assert!(parse("fkv_page done a b").is_err(), "多余列应报错");
    assert!(
      parse("fkv_page done a\nfkv_page ignored b").is_err(),
      "重复 crate 应报错"
    );
  }

  #[test]
  fn apply_advances_only_advanceable_and_is_idempotent() {
    let (db, root) = temp_db();
    insert_unit(&db, "u#1", "todo");
    insert_unit(&db, "u#2", "done");
    insert_unit(&db, "u#3", "verified");
    insert_unit(&db, "u#4", "ignored");
    let line_li = parse("fkv_page done crate/format/fkv_page/tests").unwrap();

    let summary_li = apply(&db, &line_li, false).unwrap();
    assert_eq!(summary_li[0].total, 4);
    assert_eq!(summary_li[0].advanced, 1);
    assert_eq!(summary_li[0].evidence_added, 1);
    assert_eq!(unit_status(&db, "u#1"), "done");
    assert_eq!(unit_status(&db, "u#2"), "done");
    assert_eq!(unit_status(&db, "u#3"), "verified");
    assert_eq!(unit_status(&db, "u#4"), "ignored", "ignored 不可被推进");

    // 第二次执行：无可推进 unit，不重复打证据。
    let again_li = apply(&db, &line_li, false).unwrap();
    assert_eq!(again_li[0].advanced, 0);
    assert_eq!(again_li[0].evidence_added, 0);

    // dry-run：状态与证据都只计数不落库。
    insert_unit(&db, "u#5", "todo");
    let dry_li = apply(&db, &line_li, true).unwrap();
    assert_eq!(dry_li[0].advanced, 1);
    assert_eq!(dry_li[0].evidence_added, 1);
    assert_eq!(unit_status(&db, "u#5"), "todo");
    drop(db);
    remove_dir_all(root).unwrap();
  }

  #[test]
  fn apply_non_terminal_target_is_idempotent() {
    let (db, root) = temp_db();
    insert_unit(&db, "u#1", "todo");
    let line_li = parse("fkv_page in_progress").unwrap();

    let first_li = apply(&db, &line_li, false).unwrap();
    assert_eq!(first_li[0].advanced, 1);
    assert_eq!(unit_status(&db, "u#1"), "in_progress");

    // 非终态目标重放：状态相同即不再推进。
    let second_li = apply(&db, &line_li, false).unwrap();
    assert_eq!(second_li[0].advanced, 0);
    drop(db);
    remove_dir_all(root).unwrap();
  }

  #[test]
  fn apply_empty_crate_is_skipped_with_zero_total() {
    let (db, root) = temp_db();
    insert_unit(&db, "u#1", "todo");
    let line_li = parse("fkv_missing done").unwrap();
    let summary_li = apply(&db, &line_li, false).unwrap();
    assert_eq!(summary_li[0].total, 0);
    assert_eq!(summary_li[0].advanced, 0);
    drop(db);
    remove_dir_all(root).unwrap();
  }
}
