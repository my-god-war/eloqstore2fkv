//! fkv 迁移账本。
//!
//! 纯 CLI：sync / scan / baseline / map / next / list / claim / status / ignore /
//! evidence / edges / stats / db。
//! 数据库固定 `.data/motedb.mote`，已在 `.gitignore` 中。

use std::{fs, path::Path};

use clap_args::{
  arg,
  clap::{Arg, ArgAction, ArgMatches, Command},
};
use fkv_ledger::{adopt, baseline, classify, corpus, db, error, fixture, ledger, map, scan};
// rusty_alloc_default 在被链接时安装为全局分配器。
use rusty_alloc_default as _;

fn arg_str<'a>(arg_match: &'a ArgMatches, key: &str, default: &'a str) -> &'a str {
  arg_match
    .get_one::<String>(key)
    .map(String::as_str)
    .unwrap_or(default)
}

fn db_path(arg_match: &ArgMatches) -> &str {
  arg_str(arg_match, "db", ".data/motedb.mote")
}

fn main() -> error::Result<()> {
  let Some(arg_match) = clap_args::parse!(|cmd: Command| {
    cmd
      .subcommand(
        Command::new("scan")
          .about("用 tree-sitter 扫描 eloqstore 并写入迁移账本")
          .arg(arg!(--root <ROOT> "eloqstore 根目录").default_value("../eloqstore"))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("baseline")
          .about("读取 / 更新 COMPAT_BASELINE 到最新 main")
          .arg(arg!(--repo <REPO> "eloqstore 仓库").default_value("../eloqstore"))
          .arg(arg!(--ref <REF> "git ref").default_value("main"))
          .arg(arg!(--file <FILE> "基线文件").default_value("COMPAT_BASELINE"))
          .arg(arg!(--write "写入基线文件").action(ArgAction::SetTrue))
          .arg(arg!(--offline "不执行 git fetch").action(ArgAction::SetTrue)),
      )
      .subcommand(
        Command::new("sync")
          .about("更新基线 + scan + crate 映射")
          .arg(arg!(--repo <REPO> "eloqstore 仓库").default_value("../eloqstore"))
          .arg(arg!(--root <ROOT> "eloqstore 根目录").default_value("../eloqstore"))
          .arg(arg!(--ref <REF> "git ref").default_value("main"))
          .arg(arg!(--file <FILE> "基线文件").default_value("COMPAT_BASELINE"))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote"))
          .arg(arg!(--offline "不执行 git fetch").action(ArgAction::SetTrue)),
      )
      .subcommand(
        Command::new("map")
          .about("按 path 规则映射 unit -> crate")
          .arg(arg!(--seed "重置并写入默认规则").action(ArgAction::SetTrue))
          .arg(arg!(--force "覆盖已有 crate").action(ArgAction::SetTrue))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("classify")
          .about("按分类清单批量 map / ignore（可从空账本重放）")
          .arg(arg!(--file <FILE> "分类清单").default_value("doc/ledger-classification.tsv"))
          .arg(arg!(--plan "只打印计划，不写库").action(ArgAction::SetTrue))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("next")
          .about("列出下一批迁移任务")
          .arg(arg!(--limit <LIMIT> "数量").default_value("20"))
          .arg(arg!(--crate <CRATE> "按 crate 过滤").action(ArgAction::Set))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("list")
          .about("列出迁移任务")
          .arg(arg!(--status <STATUS> "todo / claimed / ...").action(ArgAction::Set))
          .arg(arg!(--crate <CRATE> "按 crate 过滤").action(ArgAction::Set))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("claim")
          .about("认领任务")
          .arg(Arg::new("unit").required(true))
          .arg(
            arg!(--owner <OWNER> "认领人")
              .action(ArgAction::Set)
              .required(true),
          )
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("status")
          .about("修改 unit 状态")
          .arg(Arg::new("unit").required(true))
          .arg(Arg::new("status").required(true))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("ignore")
          .about("忽略不需要迁移的 unit")
          .arg(Arg::new("unit").required(true))
          .arg(arg!(--reason <REASON> "忽略原因").action(ArgAction::Set))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("evidence")
          .about("迁移证据")
          .subcommand(
            Command::new("add")
              .about("添加 evidence")
              .arg(Arg::new("unit").required(true))
              .arg(Arg::new("kind").required(true))
              .arg(arg!(--path <PATH> "证据路径").action(ArgAction::Set))
              .arg(arg!(--result <RESULT> "pass / fail / skip").action(ArgAction::Set))
              .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
          )
          .subcommand(
            Command::new("list")
              .about("列出 evidence")
              .arg(Arg::new("unit").required(true))
              .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
          ),
      )
      .subcommand(
        Command::new("edges")
          .about("查看 unit 所在文件的 include 依赖")
          .arg(Arg::new("unit").required(true))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("stats")
          .about("按状态统计")
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("changed_unit_li")
          .visible_alias("changed")
          .about("列出 verified 后 C++ 源码已变化的 unit")
          .arg(arg!(--crate <CRATE> "按 crate 过滤").action(ArgAction::Set))
          .arg(arg!(--check "有变化时返回失败").action(ArgAction::SetTrue))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("report")
          .about("生成 Markdown 迁移进度报告")
          .arg(arg!(--out <FILE> "输出文件；缺省打印 stdout").action(ArgAction::Set))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("export")
          .about("导出 JSON 账本数据")
          .arg(arg!(--out <FILE> "输出文件；缺省打印 stdout").action(ArgAction::Set))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("db")
          .about("打开 / 初始化迁移账本")
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("adopt")
          .about("按 doc/adopt.tsv 声明批量对齐 unit 状态（幂等，可重放）")
          .arg(arg!(--file <FILE> "声明文件").default_value("doc/adopt.tsv"))
          .arg(arg!(--plan "只打印计划，不写库").action(ArgAction::SetTrue))
          .arg(arg!(--db <DB> "motedb 文件").default_value(".data/motedb.mote")),
      )
      .subcommand(
        Command::new("issues")
          .about("统计 issue/ 审查看板各目录条目数")
          .arg(arg!(--root <ROOT> "fkv 仓库根目录").default_value(".")),
      )
      .subcommand(
        Command::new("fixture")
          .about("生成 C++ oracle fixture 清单（无 oracle 时写 skipped）")
          .arg(arg!(--out <DIR> "输出目录").default_value("fixtures/generated"))
          .arg(
            arg!(--kind <KIND> "page / index / overflow / manifest / compress / mapping / store")
              .default_value("page"),
          )
          .arg(arg!(--case <CASE> "case 名").action(ArgAction::Append))
          .arg(arg!(--matrix "按内置 case 矩阵全量生成").action(ArgAction::SetTrue))
          .arg(
            arg!(--rust "反向 fixture：用 Rust builder 写语料（C++ oracle 读回）")
              .action(ArgAction::SetTrue),
          )
          .arg(arg!(--oracle <PATH> "C++ oracle 可执行文件").action(ArgAction::Set))
          .arg(arg!(--baseline <FILE> "兼容基线文件").default_value("COMPAT_BASELINE")),
      )
      .subcommand(
        Command::new("verify")
          .about("遍历 fixture 目录并校验（无 oracle 时记为 skipped）")
          .arg(arg!(--fixtures <DIR> "fixture 根目录").default_value("fixtures/generated"))
          .arg(arg!(--oracle <PATH> "C++ oracle 可执行文件").action(ArgAction::Set))
          .arg(arg!(--report <FILE> "Markdown 报告输出").action(ArgAction::Set)),
      )
  }) else {
    return Ok(());
  };

  match arg_match.subcommand() {
    Some(("scan", m)) => {
      let root = arg_str(m, "root", "../eloqstore");
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let unit_count = scan::scan(Path::new(root), &db)?;
      println!("scan {root}: {unit_count} unit 已写入 {db_path}");
    }
    Some(("baseline", m)) => {
      let repo = arg_str(m, "repo", "../eloqstore");
      let reference = arg_str(m, "ref", "main");
      let file = arg_str(m, "file", "COMPAT_BASELINE");
      let old = baseline::read(Path::new(file))?;
      let (commit, date) = baseline::latest(Path::new(repo), reference, !m.get_flag("offline"))?;
      let old_desc = old
        .as_ref()
        .map(|value| format!("{} @ {}", value.commit, value.date))
        .unwrap_or_else(|| "<none>".to_owned());
      println!("baseline: {old_desc} -> {commit} ({date})");
      if m.get_flag("write") {
        let level = old
          .as_ref()
          .map(|value| value.level.as_str())
          .unwrap_or("L1");
        baseline::write(Path::new(file), reference, &commit, &date, level)?;
        println!("written {file}");
      }
    }
    Some(("sync", m)) => {
      let repo = arg_str(m, "repo", "../eloqstore");
      let root = arg_str(m, "root", "../eloqstore");
      let reference = arg_str(m, "ref", "main");
      let file = arg_str(m, "file", "COMPAT_BASELINE");
      let db_path = db_path(m);
      let old = baseline::read(Path::new(file))?;
      let (commit, date) = baseline::latest(Path::new(repo), reference, !m.get_flag("offline"))?;
      let level = old
        .as_ref()
        .map(|value| value.level.as_str())
        .unwrap_or("L1");
      baseline::write(Path::new(file), reference, &commit, &date, level)?;
      let db = db::open(Path::new(db_path))?;
      let unit_count = scan::scan(Path::new(root), &db)?;
      map::seed(&db)?;
      let mapped = map::apply(&db, false)?;
      println!("sync {commit}: {unit_count} unit, {mapped} mapped -> {db_path}");
    }
    Some(("map", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      if m.get_flag("seed") {
        let rule_count = map::seed(&db)?;
        println!("seeded {rule_count} rule");
      }
      let mapped = map::apply(&db, m.get_flag("force"))?;
      println!("mapped {mapped} unit");
    }
    Some(("classify", m)) => {
      let file = arg_str(m, "file", "doc/ledger-classification.tsv");
      let text = fs::read_to_string(file)
        .map_err(|err| error::LedgerError::Invalid(format!("read {file}: {err}")))?;
      let line_li = classify::parse(&text)?;
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let summary = classify::apply(&db, &line_li, m.get_flag("plan"))?;
      println!(
        "classify: {} rows, mapped {}, ignored {}, unchanged {}, skipped_terminal {}, missing {}",
        summary.total(),
        summary.mapped,
        summary.ignored,
        summary.unchanged,
        summary.skipped_terminal,
        summary.missing
      );
      if m.get_flag("plan") {
        println!("plan：未写库");
      }
      if summary.missing > 0 {
        return Err(error::LedgerError::Invalid(format!(
          "{} classification row(s) reference missing unit(s); 清单与账本不同步",
          summary.missing
        )));
      }
    }
    Some(("next", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let limit = arg_str(m, "limit", "20").parse().unwrap_or(20);
      ledger::next(&db, limit, m.get_one::<String>("crate").map(String::as_str))?;
    }
    Some(("list", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::list(
        &db,
        m.get_one::<String>("status").map(String::as_str),
        m.get_one::<String>("crate").map(String::as_str),
      )?;
    }
    Some(("claim", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::claim(&db, arg_str(m, "unit", ""), arg_str(m, "owner", ""))?;
    }
    Some(("status", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::set_status(&db, arg_str(m, "unit", ""), arg_str(m, "status", ""))?;
    }
    Some(("ignore", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::ignore(&db, arg_str(m, "unit", ""), arg_str(m, "reason", ""))?;
    }
    Some(("evidence", m)) => match m.subcommand() {
      Some(("add", sub)) => {
        let db_path = db_path(sub);
        let db = db::open(Path::new(db_path))?;
        ledger::evidence_add(
          &db,
          arg_str(sub, "unit", ""),
          arg_str(sub, "kind", ""),
          arg_str(sub, "path", ""),
          arg_str(sub, "result", ""),
        )?;
      }
      Some(("list", sub)) => {
        let db_path = db_path(sub);
        let db = db::open(Path::new(db_path))?;
        ledger::evidence_list(&db, arg_str(sub, "unit", ""))?;
      }
      _ => eprintln!("use evidence add / list"),
    },
    Some(("edges", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::edges(&db, arg_str(m, "unit", ""))?;
    }
    Some(("stats", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::status_board(&db)?;
    }
    Some(("changed_unit_li", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      ledger::changed_unit_li(
        &db,
        m.get_one::<String>("crate").map(String::as_str),
        m.get_flag("check"),
      )?;
    }
    Some(("report", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let out = m.get_one::<String>("out").map(String::as_str);
      ledger::report(&db, out.map(Path::new))?;
    }
    Some(("export", m)) => {
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let out = m.get_one::<String>("out").map(String::as_str);
      ledger::export(&db, out.map(Path::new))?;
    }
    Some(("fixture", m)) => {
      let out = Path::new(arg_str(m, "out", "fixtures/generated"));
      let baseline = fixture::baseline_text(Path::new(arg_str(m, "baseline", "COMPAT_BASELINE")));
      let oracle = m
        .get_one::<String>("oracle")
        .map(|path| Path::new(path.as_str()));
      // --rust：反向 fixture，用 Rust builder 写同一语料，C++ oracle 读回。
      if m.get_flag("rust") {
        let kind = arg_str(m, "kind", "page");
        let kind_case_li: Vec<(&str, &str)> = if m.get_flag("matrix") {
          fixture::MATRIX
            .iter()
            .filter(|(kind, _)| fixture::oracle_supported(kind))
            .flat_map(|(kind, case_li)| case_li.iter().map(move |case| (*kind, *case)))
            .collect()
        } else {
          match m.get_many::<String>("case") {
            Some(values) => values.map(|case| (kind, case.as_str())).collect(),
            None => {
              return Err(error::LedgerError::Invalid(
                "--rust 需要 --case 或 --matrix".to_owned(),
              ));
            }
          }
        };
        for (kind, case) in kind_case_li {
          let dir = corpus::generate_reverse(out, kind, case, &baseline, fixture::SCHEMA_VERSION)?;
          println!("fixture(rust) {kind}/{case} -> {}", dir.display());
        }
        return Ok(());
      }
      if m.get_flag("matrix") {
        for dir in fixture::generate_matrix(out, oracle, &baseline)? {
          println!("fixture -> {}", dir.display());
        }
        return Ok(());
      }
      let kind = arg_str(m, "kind", "page");
      let case_li: Vec<String> = match m.get_many::<String>("case") {
        Some(values) => values.cloned().collect(),
        None => vec!["smoke".to_owned()],
      };
      for case in &case_li {
        let dir = fixture::generate(out, kind, case, oracle, &baseline)?;
        println!("fixture {kind}/{case} -> {}", dir.display());
      }
    }
    Some(("verify", m)) => {
      let fixtures = Path::new(arg_str(m, "fixtures", "fixtures/generated"));
      let oracle = m
        .get_one::<String>("oracle")
        .map(|path| Path::new(path.as_str()));
      let report = fixture::verify(fixtures, oracle)?;
      print!("{}", report.to_markdown());
      if let Some(out) = m.get_one::<String>("report") {
        fs::write(out, report.to_markdown())?;
      }
      if report.failed() {
        return Err(error::LedgerError::Invalid(format!(
          "{} fixture case failed",
          report.fail
        )));
      }
    }
    Some(("db", m)) => {
      let db_path = db_path(m);
      let _db = db::open(Path::new(db_path))?;
      println!("motedb: {db_path}");
    }
    Some(("adopt", m)) => {
      let file = arg_str(m, "file", "doc/adopt.tsv");
      let text = fs::read_to_string(file)
        .map_err(|err| error::LedgerError::Invalid(format!("read {file}: {err}")))?;
      let line_li = adopt::parse(&text)?;
      if line_li.is_empty() {
        println!("adopt: {file} 无有效声明");
        return Ok(());
      }
      let db_path = db_path(m);
      let db = db::open(Path::new(db_path))?;
      let summary_li = adopt::apply(&db, &line_li, m.get_flag("plan"))?;
      println!("crate | total | advanced | evidence");
      for summary in &summary_li {
        println!(
          "{} | {} | {} | {}",
          summary.crate_name, summary.total, summary.advanced, summary.evidence_added
        );
      }
      if m.get_flag("plan") {
        println!("plan：未写库");
      }
    }
    Some(("issues", m)) => {
      let root = arg_str(m, "root", ".");
      let board = ledger::issue_board(Path::new(root));
      println!("dir | count");
      for (dir, count) in &board {
        println!("{dir} | {count}");
      }
    }
    _ => eprintln!("use --help"),
  }
  Ok(())
}
