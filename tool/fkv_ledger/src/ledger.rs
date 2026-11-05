use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  io::ErrorKind,
  path::Path,
};

use motedb::{Database, types::Value};
use serde_json::json;

use crate::{
  error::{LedgerError, Result},
  util::{cell_text, format_ts, now_ns, now_sec, text_of},
};

const STATUS_LI: &[&str] = &[
  "todo",
  "claimed",
  "in_progress",
  "review",
  "done",
  "verified",
  "blocked",
  "ignored",
];

/// issue/ 看板目录（to_markdown 与 issue_board 共用，顺序即展示顺序）。
const ISSUE_DIR_LI: &[&str] = &["todo", "new", "done", "reviewed", "reject"];

#[derive(Debug, Clone, Default)]
pub struct UnitRow {
  pub id: String,
  pub path: String,
  pub symbol: String,
  pub kind: String,
  pub crate_name: String,
  pub layer: String,
  pub status: String,
  pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedUnit {
  pub id: String,
  pub path: String,
  pub symbol: String,
  pub verified_hash: String,
  pub source_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateStat {
  pub total: usize,
  pub todo: usize,
  pub done: usize,
  pub verified: usize,
}

impl CrateStat {
  fn add(&mut self, status: &str) {
    self.total += 1;
    match status {
      "todo" => self.todo += 1,
      "done" => self.done += 1,
      "verified" => self.verified += 1,
      _ => {}
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
  pub generated_at: i64,
  pub baseline: String,
  pub total: usize,
  pub by_status: BTreeMap<String, usize>,
  pub by_crate: BTreeMap<String, CrateStat>,
  /// `classify` 写入的 ignore 原因 -> unit 数（只统计当前 status=ignored 的 unit）。
  pub ignored_by_reason: BTreeMap<String, usize>,
  pub by_issue: BTreeMap<String, usize>,
  pub changed_unit_li: Vec<ChangedUnit>,
  pub next_li: Vec<UnitRow>,
}

impl Report {
  pub fn verified_ratio(&self) -> f64 {
    if self.total == 0 {
      return 0.0;
    }
    self
      .by_status
      .get("verified")
      .copied()
      .unwrap_or(0)
      .saturating_add(self.by_status.get("ignored").copied().unwrap_or(0)) as f64
      / self.total as f64
  }

  pub fn to_markdown(&self) -> String {
    let mut out = String::new();
    out.push_str("# fkv 迁移进度\n\n");
    out.push_str("> 本文件由 `cargo run -p fkv_ledger -- report --out doc/progress.md` 生成，请勿手工编辑。\n\n");
    out.push_str(&format!(
      "- 生成时间（UTC）：{}\n",
      format_ts(self.generated_at)
    ));
    out.push_str(&format!(
      "- 兼容基线：{}\n",
      if self.baseline.is_empty() {
        "<未读取到 COMPAT_BASELINE>"
      } else {
        &self.baseline
      }
    ));
    out.push_str(&format!(
      "- verified + ignored 覆盖率：{:.1}%\n\n",
      self.verified_ratio() * 100.0
    ));
    let todo = self.by_status.get("todo").copied().unwrap_or(0);
    let ignored = self.by_status.get("ignored").copied().unwrap_or(0);
    let unmapped = self.by_crate.get("").map(|stat| stat.total).unwrap_or(0);
    out.push_str(&format!(
      "- 分流口径：todo {todo} / ignored（范围外裁定）{ignored} / <unmapped> {unmapped}；\
       真实剩余（todo）= {todo}\n\n"
    ));

    out.push_str("## 审查看板（issue/）\n\n");
    out.push_str("真实开发进度的另一来源：`issue/` 目录按 run.md 流程流转。\n\n");
    out.push_str("| 目录 | 条目 |\n|---|---:|\n");
    for dir in ISSUE_DIR_LI {
      let count = self.by_issue.get(*dir).copied().unwrap_or(0);
      out.push_str(&format!("| {dir} | {count} |\n"));
    }
    out.push('\n');

    out.push_str("## 状态总览\n\n");
    out.push_str("| status | count |\n|---|---:|\n");
    for status in STATUS_LI {
      let count = self.by_status.get(*status).copied().unwrap_or(0);
      out.push_str(&format!("| {status} | {count} |\n"));
    }
    out.push_str(&format!("| **total** | **{}** |\n\n", self.total));

    out.push_str("## 按 crate\n\n");
    out.push_str("| crate | total | todo | done | verified |\n|---|---:|---:|---:|---:|\n");
    for (crate_name, stat) in &self.by_crate {
      let name = if crate_name.is_empty() {
        "<unmapped>"
      } else {
        crate_name.as_str()
      };
      out.push_str(&format!(
        "| {name} | {} | {} | {} | {} |\n",
        stat.total, stat.todo, stat.done, stat.verified
      ));
    }
    out.push('\n');

    out.push_str("## 范围外裁定\n\n");
    let ignored_total: usize = self.ignored_by_reason.values().sum();
    out.push_str(&format!(
      "共 {ignored_total} 条（status=ignored）；原因必须指向 `COMPAT_BASELINE` 的范围块。\n\n"
    ));
    if self.ignored_by_reason.is_empty() {
      out.push_str("无。\n\n");
    } else {
      out.push_str("| 原因 | 数量 |\n|---|---:|\n");
      for (reason, count) in &self.ignored_by_reason {
        out.push_str(&format!("| {reason} | {count} |\n"));
      }
      out.push('\n');
    }

    out.push_str(&format!(
      "## 已变更需重验（{}）\n\n",
      self.changed_unit_li.len()
    ));
    if self.changed_unit_li.is_empty() {
      out.push_str("无。\n\n");
    } else {
      out.push_str("| unit | path | source_hash |\n|---|---|---|\n");
      for unit in self.changed_unit_li.iter().take(50) {
        out.push_str(&format!(
          "| `{}` | {} | {} -> {} |\n",
          unit.id, unit.path, unit.verified_hash, unit.source_hash
        ));
      }
      out.push('\n');
    }

    out.push_str("## 下一批任务\n\n");
    if self.next_li.is_empty() {
      out.push_str("无。\n");
    } else {
      out.push_str("| unit | crate | path |\n|---|---|---|\n");
      for unit in &self.next_li {
        out.push_str(&format!(
          "| `{}` | {} | {} |\n",
          unit.id,
          if unit.crate_name.is_empty() {
            "<unmapped>"
          } else {
            &unit.crate_name
          },
          unit.path
        ));
      }
    }
    out
  }
}

fn load_unit_li(db: &Database) -> Result<Vec<UnitRow>> {
  let result = db
    .execute("SELECT id, path, symbol, kind, crate, layer, status, source_hash FROM unit")?
    .materialize()?;
  let mut row_li = Vec::new();
  if let Some((_, rows)) = result.select_rows() {
    for row in rows {
      let text = |index: usize| row.get(index).and_then(text_of).unwrap_or_default();
      row_li.push(UnitRow {
        id: text(0).to_owned(),
        path: text(1).to_owned(),
        symbol: text(2).to_owned(),
        kind: text(3).to_owned(),
        crate_name: text(4).to_owned(),
        layer: text(5).to_owned(),
        status: text(6).to_owned(),
        source_hash: text(7).to_owned(),
      });
    }
  }
  Ok(row_li)
}

fn unit_row(db: &Database, unit_id: &str) -> Result<Option<UnitRow>> {
  let result = db
    .execute_prepared(
      "SELECT id, path, symbol, kind, crate, layer, status, source_hash \
       FROM unit WHERE id = ?",
      vec![Value::text(unit_id.to_owned())],
    )?
    .materialize()?;
  let Some((_, rows)) = result.select_rows() else {
    return Ok(None);
  };
  let Some(row) = rows.first() else {
    return Ok(None);
  };
  let text = |index: usize| row.get(index).and_then(text_of).unwrap_or_default();
  Ok(Some(UnitRow {
    id: text(0).to_owned(),
    path: text(1).to_owned(),
    symbol: text(2).to_owned(),
    kind: text(3).to_owned(),
    crate_name: text(4).to_owned(),
    layer: text(5).to_owned(),
    status: text(6).to_owned(),
    source_hash: text(7).to_owned(),
  }))
}

fn pass_evidence_count(db: &Database, unit_id: &str) -> Result<usize> {
  let result = db
    .execute_prepared(
      "SELECT COUNT(*) FROM evidence WHERE unit_id = ? AND result = 'pass'",
      vec![Value::text(unit_id.to_owned())],
    )?
    .materialize()?;
  Ok(count_of(&result))
}

fn count_of(result: &motedb::QueryResult) -> usize {
  result
    .select_rows()
    .and_then(|(_, rows)| rows.first())
    .and_then(|row| row.first())
    .map(|value| match value {
      Value::Integer(count) => usize::try_from(*count).unwrap_or(0),
      _ => 0,
    })
    .unwrap_or(0)
}

fn record_verification(
  db: &Database,
  unit_id: &str,
  source_hash: &str,
  commit_id: &str,
) -> Result<()> {
  db.execute_prepared(
    "DELETE FROM verification WHERE unit_id = ?",
    vec![Value::text(unit_id.to_owned())],
  )?
  .materialize()?;
  db.execute_prepared(
    "INSERT INTO verification (unit_id, source_hash, commit_id, created_at) \
     VALUES (?, ?, ?, ?)",
    vec![
      Value::text(unit_id.to_owned()),
      Value::text(source_hash.to_owned()),
      Value::text(commit_id.to_owned()),
      Value::Integer(now_sec()),
    ],
  )?
  .materialize()?;
  Ok(())
}

fn clear_verification(db: &Database, unit_id: &str) -> Result<()> {
  db.execute_prepared(
    "DELETE FROM verification WHERE unit_id = ?",
    vec![Value::text(unit_id.to_owned())],
  )?
  .materialize()?;
  Ok(())
}

fn verification_map(db: &Database) -> Result<BTreeMap<String, String>> {
  let result = db
    .execute("SELECT unit_id, source_hash FROM verification")?
    .materialize()?;
  let mut map = BTreeMap::new();
  if let Some((_, rows)) = result.select_rows() {
    for row in rows {
      let Some(unit_id) = row.first().and_then(text_of) else {
        continue;
      };
      let source_hash = row.get(1).and_then(text_of).unwrap_or_default();
      map.insert(unit_id.to_owned(), source_hash.to_owned());
    }
  }
  Ok(map)
}

pub fn list(db: &Database, status: Option<&str>, crate_name: Option<&str>) -> Result<()> {
  let mut sql = String::from("SELECT id, symbol, kind, status, owner FROM unit");
  let mut cond_li = Vec::new();
  let mut param_li = Vec::new();
  if let Some(status) = status {
    cond_li.push("status = ?");
    param_li.push(Value::text(status.to_owned()));
  }
  if let Some(crate_name) = crate_name {
    cond_li.push("crate = ?");
    param_li.push(Value::text(crate_name.to_owned()));
  }
  if !cond_li.is_empty() {
    sql.push_str(" WHERE ");
    sql.push_str(&cond_li.join(" AND "));
  }
  sql.push_str(" ORDER BY path, start_line LIMIT 200");

  let result = db.execute_prepared(&sql, param_li)?.materialize()?;
  if let Some((column_li, row_li)) = result.select_rows() {
    println!("{}", column_li.join(" | "));
    for row in row_li {
      let cell_li: Vec<String> = row.iter().map(cell_text).collect();
      println!("{}", cell_li.join(" | "));
    }
  }
  Ok(())
}

pub fn claim(db: &Database, unit_id: &str, owner: &str) -> Result<()> {
  let result = db
    .execute_prepared(
      "UPDATE unit SET owner = ?, status = 'claimed', updated_at = ? WHERE id = ?",
      vec![
        Value::text(owner.to_owned()),
        Value::Integer(now_sec()),
        Value::text(unit_id.to_owned()),
      ],
    )?
    .materialize()?;
  if result.affected_rows() == 0 {
    return Err(LedgerError::Invalid(format!("unit not found: {unit_id}")));
  }
  clear_verification(db, unit_id)?;
  println!("claimed {} unit", result.affected_rows());
  Ok(())
}

pub fn set_status(db: &Database, unit_id: &str, status: &str) -> Result<()> {
  if !STATUS_LI.contains(&status) {
    return Err(LedgerError::Invalid(format!(
      "unknown status {status}, expected one of {STATUS_LI:?}"
    )));
  }
  let Some(unit) = unit_row(db, unit_id)? else {
    return Err(LedgerError::Invalid(format!("unit not found: {unit_id}")));
  };
  if status == "verified" && pass_evidence_count(db, unit_id)? == 0 {
    return Err(LedgerError::Invalid(format!(
      "unit {unit_id} has no pass evidence; run `evidence add {unit_id} <kind> --result pass` first"
    )));
  }
  let result = db
    .execute_prepared(
      "UPDATE unit SET status = ?, updated_at = ? WHERE id = ?",
      vec![
        Value::text(status.to_owned()),
        Value::Integer(now_sec()),
        Value::text(unit_id.to_owned()),
      ],
    )?
    .materialize()?;
  if result.affected_rows() == 0 {
    return Err(LedgerError::Invalid(format!("unit not found: {unit_id}")));
  }
  if status == "verified" {
    record_verification(db, unit_id, &unit.source_hash, "")?;
  } else {
    clear_verification(db, unit_id)?;
  }
  println!("updated {} unit -> {status}", result.affected_rows());
  Ok(())
}

pub fn ignore(db: &Database, unit_id: &str, reason: &str) -> Result<()> {
  set_status(db, unit_id, "ignored")?;
  if !reason.is_empty() {
    let note_id = format!("{unit_id}#ignore#{}", now_ns());
    db.execute_prepared(
      "INSERT INTO note (id, unit_id, author, body, created_at) VALUES (?, ?, ?, ?, ?)",
      vec![
        Value::text(note_id),
        Value::text(unit_id.to_owned()),
        Value::text("cli".to_owned()),
        Value::text(reason.to_owned()),
        Value::Integer(now_sec()),
      ],
    )?
    .materialize()?;
  }
  Ok(())
}

pub fn status_board(db: &Database) -> Result<()> {
  let mut total = 0;
  for status in STATUS_LI {
    let result = db
      .execute_prepared(
        "SELECT COUNT(*) FROM unit WHERE status = ?",
        vec![Value::text((*status).to_owned())],
      )?
      .materialize()?;
    let count = count_of(&result);
    total += count;
    println!("{status}: {count}");
  }
  let result = db.execute("SELECT COUNT(*) FROM unit")?.materialize()?;
  println!("total: {}", count_of(&result));
  println!("status_sum: {total}");
  Ok(())
}

pub fn changed_units(db: &Database, crate_name: Option<&str>) -> Result<Vec<ChangedUnit>> {
  let verified = verification_map(db)?;
  let mut changed_li = Vec::new();
  for unit in load_unit_li(db)? {
    if unit.status != "verified" {
      continue;
    }
    if let Some(crate_name) = crate_name
      && unit.crate_name != crate_name
    {
      continue;
    }
    let Some(verified_hash) = verified.get(&unit.id) else {
      changed_li.push(ChangedUnit {
        id: unit.id,
        path: unit.path,
        symbol: unit.symbol,
        verified_hash: "<missing>".to_owned(),
        source_hash: unit.source_hash,
      });
      continue;
    };
    if verified_hash != &unit.source_hash {
      changed_li.push(ChangedUnit {
        id: unit.id,
        path: unit.path,
        symbol: unit.symbol,
        verified_hash: verified_hash.clone(),
        source_hash: unit.source_hash,
      });
    }
  }
  Ok(changed_li)
}

pub fn changed_unit_li(db: &Database, crate_name: Option<&str>, check: bool) -> Result<()> {
  let changed_li = changed_units(db, crate_name)?;
  println!("id | path | symbol | verified_hash | source_hash");
  for unit in &changed_li {
    println!(
      "{} | {} | {} | {} | {}",
      unit.id, unit.path, unit.symbol, unit.verified_hash, unit.source_hash
    );
  }
  println!("changed_unit_li: {}", changed_li.len());
  if check && !changed_li.is_empty() {
    return Err(LedgerError::Invalid(format!(
      "{} verified unit(s) changed_unit_li since verification; add evidence and re-run status verified",
      changed_li.len()
    )));
  }
  Ok(())
}

/// 统计 issue/ 看板各目录的条目文件数（跳过 `00-*` 索引）。
/// 报告在仓库根目录执行，root 传 `.`。目录不存在记 0；其它 IO 错误打 WARN 也记 0，
/// 不让看板统计中断报告生成。
pub fn issue_board(root: &Path) -> BTreeMap<String, usize> {
  let mut board = BTreeMap::new();
  for dir in ISSUE_DIR_LI {
    let dir_path = root.join("issue").join(dir);
    let count = match fs::read_dir(&dir_path) {
      Ok(entry_li) => entry_li
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
          entry.path().extension().is_some_and(|ext| ext == "md")
            && entry
              .file_name()
              .to_str()
              .is_some_and(|name| !name.starts_with("00-"))
        })
        .count(),
      Err(err) if err.kind() == ErrorKind::NotFound => 0,
      Err(err) => {
        eprintln!("WARN: read_dir {}: {err}", dir_path.display());
        0
      }
    };
    board.insert((*dir).to_owned(), count);
  }
  board
}

pub fn build_report(db: &Database) -> Result<Report> {
  let units = load_unit_li(db)?;
  let mut by_status = BTreeMap::new();
  let mut by_crate: BTreeMap<String, CrateStat> = BTreeMap::new();
  let mut next_li = Vec::new();
  for unit in &units {
    *by_status.entry(unit.status.clone()).or_insert(0) += 1;
    by_crate
      .entry(unit.crate_name.clone())
      .or_default()
      .add(&unit.status);
    if matches!(unit.status.as_str(), "todo" | "claimed") {
      next_li.push(unit.clone());
    }
  }
  next_li.sort_by(|left, right| {
    left
      .path
      .cmp(&right.path)
      .then_with(|| left.symbol.cmp(&right.symbol))
  });
  // 每个 crate 取一个代表，避免报告里全是同目录的 20 个相邻方法。
  let mut seen_crate = BTreeSet::new();
  next_li.retain(|unit| seen_crate.insert(unit.crate_name.clone()));
  next_li.truncate(20);

  let ignored_id_set: BTreeSet<&str> = units
    .iter()
    .filter(|unit| unit.status == "ignored")
    .map(|unit| unit.id.as_str())
    .collect();
  let mut reason_by_unit: BTreeMap<String, String> = BTreeMap::new();
  let note_result = db
    .execute("SELECT unit_id, body FROM note")?
    .materialize()?;
  if let Some((_, row_li)) = note_result.select_rows() {
    for row in row_li {
      let Some(unit_id) = row.first().and_then(text_of) else {
        continue;
      };
      if !ignored_id_set.contains(unit_id) {
        continue;
      }
      let Some(reason) = row.get(1).and_then(text_of) else {
        continue;
      };
      // 只认指向基线范围块的裁定原因；`cli` / `classify` 两种作者都兼容。
      if !reason.contains("COMPAT_BASELINE") {
        continue;
      }
      reason_by_unit.insert(unit_id.to_owned(), reason.to_owned());
    }
  }
  let mut ignored_by_reason = BTreeMap::new();
  for reason in reason_by_unit.into_values() {
    *ignored_by_reason.entry(reason).or_insert(0) += 1;
  }
  Ok(Report {
    generated_at: now_sec(),
    baseline: fs::read_to_string("COMPAT_BASELINE")
      .map(|text| {
        text
          .lines()
          .map(str::trim)
          .filter(|line| !line.is_empty())
          .collect::<Vec<_>>()
          .join(" / ")
      })
      .unwrap_or_default(),
    total: units.len(),
    by_status,
    by_crate,
    ignored_by_reason,
    by_issue: issue_board(Path::new(".")),
    changed_unit_li: changed_units(db, None)?,
    next_li,
  })
}

pub fn report(db: &Database, out: Option<&Path>) -> Result<()> {
  let markdown = build_report(db)?.to_markdown();
  match out {
    Some(path) => {
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::write(path, markdown)?;
      println!("wrote {}", path.display());
    }
    None => print!("{markdown}"),
  }
  Ok(())
}

pub fn export_json(db: &Database) -> Result<String> {
  let units = load_unit_li(db)?;
  let report = build_report(db)?;
  let crate_json: serde_json::Map<String, serde_json::Value> = report
    .by_crate
    .iter()
    .map(|(name, stat)| {
      (
        name.clone(),
        json!({
          "total": stat.total,
          "todo": stat.todo,
          "done": stat.done,
          "verified": stat.verified,
        }),
      )
    })
    .collect();
  let unit_json: Vec<serde_json::Value> = units
    .iter()
    .map(|unit| {
      json!({
        "id": unit.id,
        "path": unit.path,
        "symbol": unit.symbol,
        "kind": unit.kind,
        "crate": unit.crate_name,
        "layer": unit.layer,
        "status": unit.status,
        "source_hash": unit.source_hash,
      })
    })
    .collect();
  let output = json!({
    "generated_at": report.generated_at,
    "generated_at_iso": format_ts(report.generated_at),
    "baseline": &report.baseline,
    "summary": {
      "total": report.total,
      "by_status": &report.by_status,
      "by_crate": crate_json,
      "by_issue": &report.by_issue,
      "changed_unit_li": report.changed_unit_li.len(),
      "verified_ratio": report.verified_ratio(),
    },
    "units": unit_json,
  });
  serde_json::to_string_pretty(&output)
    .map_err(|err| LedgerError::Invalid(format!("export json: {err}")))
}

pub fn export(db: &Database, out: Option<&Path>) -> Result<()> {
  let text = export_json(db)?;
  match out {
    Some(path) => {
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::write(path, text)?;
      println!("wrote {}", path.display());
    }
    None => println!("{text}"),
  }
  Ok(())
}

pub fn next(db: &Database, limit: usize, crate_name: Option<&str>) -> Result<()> {
  let mut sql = String::from(
    "SELECT id, path, symbol, kind FROM unit WHERE (status = 'todo' OR status = 'claimed')",
  );
  let mut param_li = Vec::new();
  if let Some(crate_name) = crate_name {
    sql.push_str(" AND crate = ?");
    param_li.push(Value::text(crate_name.to_owned()));
  }
  sql.push_str(&format!(
    " ORDER BY priority, path, start_line LIMIT {limit}"
  ));

  let result = db.execute_prepared(&sql, param_li)?.materialize()?;
  if let Some((column_li, row_li)) = result.select_rows() {
    println!("{}", column_li.join(" | "));
    for row in row_li {
      let cell_li: Vec<String> = row.iter().map(cell_text).collect();
      println!("{}", cell_li.join(" | "));
    }
  }
  Ok(())
}

pub fn evidence_add(
  db: &Database,
  unit_id: &str,
  kind: &str,
  path: &str,
  result: &str,
) -> Result<()> {
  if !matches!(result, "pass" | "fail" | "skip") {
    return Err(LedgerError::Invalid(format!(
      "unknown evidence result {result}, expected pass|fail|skip"
    )));
  }
  let Some(unit) = unit_row(db, unit_id)? else {
    return Err(LedgerError::Invalid(format!("unit not found: {unit_id}")));
  };
  let evidence_id = format!("{unit_id}#{kind}#{}", now_ns());
  db.execute_prepared(
    "INSERT INTO evidence (id, unit_id, kind, path, result, commit_id, created_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?)",
    vec![
      Value::text(evidence_id),
      Value::text(unit_id.to_owned()),
      Value::text(kind.to_owned()),
      Value::text(path.to_owned()),
      Value::text(result.to_owned()),
      Value::text("".to_owned()),
      Value::Integer(now_sec()),
    ],
  )?
  .materialize()?;
  if result == "pass" && unit.status == "verified" {
    record_verification(db, unit_id, &unit.source_hash, "")?;
  }
  println!("evidence added for {unit_id}: {kind} {result}");
  Ok(())
}

pub fn evidence_list(db: &Database, unit_id: &str) -> Result<()> {
  let result = db
    .execute_prepared(
      "SELECT kind, path, result, created_at FROM evidence WHERE unit_id = ?",
      vec![Value::text(unit_id.to_owned())],
    )?
    .materialize()?;
  if let Some((column_li, row_li)) = result.select_rows() {
    println!("{}", column_li.join(" | "));
    for row in row_li {
      let cell_li: Vec<String> = row.iter().map(cell_text).collect();
      println!("{}", cell_li.join(" | "));
    }
  }
  Ok(())
}

pub fn edges(db: &Database, unit_id: &str) -> Result<()> {
  let path = unit_id.split('#').next().unwrap_or(unit_id);
  let result = db
    .execute_prepared(
      "SELECT inc_path FROM include_edge WHERE path = ?",
      vec![Value::text(path.to_owned())],
    )?
    .materialize()?;
  if let Some((_, row_li)) = result.select_rows() {
    for row in row_li {
      if let Some(value) = row.first() {
        println!("{}", cell_text(value));
      }
    }
  }
  Ok(())
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
    let root = temp_dir().join(format!("fkv_ledger_{}_{}_{}", id(), now_sec(), seq));
    create_dir_all(&root).unwrap();
    let db = open(&root.join("ledger.mote")).unwrap();
    (db, root)
  }

  fn insert_unit(db: &Database, id: &str, hash: &str) {
    db.execute_prepared(
      "INSERT INTO unit (id, path, symbol, kind, crate, layer, status, source_hash) \
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
      vec![
        Value::text(id.to_owned()),
        Value::text("include/storage/page.h".to_owned()),
        Value::text("eloqstore::Page".to_owned()),
        Value::text("class_specifier".to_owned()),
        Value::text("fkv_page".to_owned()),
        Value::text("format".to_owned()),
        Value::text("todo".to_owned()),
        Value::text(hash.to_owned()),
      ],
    )
    .unwrap()
    .materialize()
    .unwrap();
  }

  #[test]
  fn verified_requires_pass_evidence_and_tracks_changed_hash() {
    let (db, root) = temp_db();
    insert_unit(&db, "unit#1", "hash-1");
    assert!(set_status(&db, "unit#1", "verified").is_err());
    evidence_add(
      &db,
      "unit#1",
      "test",
      "crate/fkv_page/tests/page.rs",
      "pass",
    )
    .unwrap();
    set_status(&db, "unit#1", "verified").unwrap();
    assert!(changed_units(&db, None).unwrap().is_empty());

    db.execute_prepared(
      "UPDATE unit SET source_hash = ? WHERE id = ?",
      vec![
        Value::text("hash-2".to_owned()),
        Value::text("unit#1".to_owned()),
      ],
    )
    .unwrap()
    .materialize()
    .unwrap();
    let changed_li = changed_units(&db, None).unwrap();
    assert_eq!(changed_li.len(), 1);
    assert_eq!(changed_li[0].verified_hash, "hash-1");
    assert_eq!(changed_li[0].source_hash, "hash-2");
    assert!(super::changed_unit_li(&db, None, true).is_err());
    drop(db);
    remove_dir_all(root).unwrap();
  }

  #[test]
  fn report_and_export_include_summary() {
    let (db, root) = temp_db();
    insert_unit(&db, "unit#1", "hash-1");
    evidence_add(&db, "unit#1", "test", "x", "pass").unwrap();
    set_status(&db, "unit#1", "verified").unwrap();
    let report_data = build_report(&db).unwrap();
    assert_eq!(report_data.total, 1);
    assert_eq!(report_data.by_status.get("verified"), Some(&1));
    assert!(report_data.to_markdown().contains("fkv 迁移进度"));
    let export = export_json(&db).unwrap();
    let value: serde_json::Value = serde_json::from_str(&export).unwrap();
    assert_eq!(value["summary"]["total"], 1);
    assert_eq!(value["units"][0]["id"], "unit#1");

    let out = root.join("progress.md");
    super::report(&db, Some(&out)).unwrap();
    assert!(out.exists());
    drop(db);
    remove_dir_all(root).unwrap();
  }

  #[test]
  fn invalid_evidence_result_is_rejected() {
    let (db, root) = temp_db();
    insert_unit(&db, "unit#1", "hash-1");
    assert!(evidence_add(&db, "unit#1", "test", "x", "ok").is_err());
    drop(db);
    remove_dir_all(root).unwrap();
  }
}
