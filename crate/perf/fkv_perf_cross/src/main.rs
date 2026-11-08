//! `fkv_perf_cross` CLI：读取 C++ `simple_bench` 日志与 Rust `fkv_bench --history`，
//! 输出 Markdown 与/或 JSON 对比报告。
//!
//! 用法：
//!
//! ```text
//! fkv_perf_cross --cpp-log <log> --rust-history <json> [--markdown <out>] [--json <out>]
//! ```
//!
//! 两个输出文件都不传时，Markdown 打到 stdout。

use std::{fs, path::PathBuf, process::ExitCode};

use fkv_perf_cross::{CompareReport, parse_cpp_log, parse_rust_history};

const USAGE: &str = "\
用法: fkv_perf_cross --cpp-log <log> --rust-history <json> [--markdown <out>] [--json <out>]

把 C++ simple_bench 日志（baseline）与 Rust fkv_bench --history（current）做逐项对比。
两个输出文件都不传时，Markdown 打到 stdout。

选项:
  --cpp-log <log>        simple_bench 日志路径（必填）
  --rust-history <json>  fkv_bench --history 产出的 JSON（必填）
  --markdown <out>       写入 Markdown 表
  --json <out>           写入 JSON 报告
  -h, --help             显示本帮助
";

#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
  cpp_log: Option<PathBuf>,
  rust_history: Option<PathBuf>,
  markdown: Option<PathBuf>,
  json: Option<PathBuf>,
}

enum ParsedArgs {
  Run(Args),
  Help,
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(CliError::Usage(message)) => {
      eprintln!("fkv_perf_cross: {message}\n");
      eprint!("{USAGE}");
      ExitCode::from(2)
    }
    Err(CliError::Failed(message)) => {
      eprintln!("fkv_perf_cross: {message}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), CliError> {
  let args = match parse_args(std::env::args().skip(1))? {
    ParsedArgs::Help => {
      print!("{USAGE}");
      return Ok(());
    }
    ParsedArgs::Run(args) => args,
  };

  let cpp_log = args
    .cpp_log
    .as_ref()
    .ok_or_else(|| CliError::Usage("缺少 --cpp-log".to_owned()))?;
  let rust_history = args
    .rust_history
    .as_ref()
    .ok_or_else(|| CliError::Usage("缺少 --rust-history".to_owned()))?;

  let cpp_text = fs::read_to_string(cpp_log)
    .map_err(|err| CliError::Failed(format!("读取 {} 失败: {err}", cpp_log.display())))?;
  let rust_text = fs::read_to_string(rust_history)
    .map_err(|err| CliError::Failed(format!("读取 {} 失败: {err}", rust_history.display())))?;

  let cpp = parse_cpp_log(&cpp_text).map_err(|err| CliError::Failed(err.to_string()))?;
  let rust = parse_rust_history(&rust_text).map_err(|err| CliError::Failed(err.to_string()))?;
  let report = CompareReport::compare(&cpp, &rust);

  let mut wrote_file = false;
  if let Some(path) = &args.markdown {
    fs::write(path, report.to_markdown())
      .map_err(|err| CliError::Failed(format!("写入 {} 失败: {err}", path.display())))?;
    wrote_file = true;
  }
  if let Some(path) = &args.json {
    let json = report
      .to_json_pretty()
      .map_err(|err| CliError::Failed(format!("序列化 JSON 失败: {err}")))?;
    fs::write(path, json)
      .map_err(|err| CliError::Failed(format!("写入 {} 失败: {err}", path.display())))?;
    wrote_file = true;
  }
  if !wrote_file {
    print!("{}", report.to_markdown());
  }
  Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<ParsedArgs, CliError> {
  let mut parsed = Args::default();

  while let Some(arg) = args.next() {
    match arg.as_str() {
      "-h" | "--help" => return Ok(ParsedArgs::Help),
      "--cpp-log" | "--rust-history" | "--markdown" | "--json" => {
        let value = args
          .next()
          .ok_or_else(|| CliError::Usage(format!("{arg} 缺少参数值")))?;
        let slot = match arg.as_str() {
          "--cpp-log" => &mut parsed.cpp_log,
          "--rust-history" => &mut parsed.rust_history,
          "--markdown" => &mut parsed.markdown,
          _ => &mut parsed.json,
        };
        if slot.is_some() {
          return Err(CliError::Usage(format!("{arg} 重复出现")));
        }
        *slot = Some(PathBuf::from(value));
      }
      other => {
        return Err(CliError::Usage(format!("未知参数: {other}")));
      }
    }
  }

  Ok(ParsedArgs::Run(parsed))
}

#[derive(Debug)]
enum CliError {
  /// 用法错误，退出码 2。
  Usage(String),
  /// IO / 解析失败，退出码 1。
  Failed(String),
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(argv: &[&str]) -> Result<ParsedArgs, CliError> {
    parse_args(argv.iter().map(|arg| (*arg).to_owned()))
  }

  #[test]
  fn parses_all_options() {
    let ParsedArgs::Run(args) = parse(&[
      "--cpp-log",
      "cpp.log",
      "--rust-history",
      "rust.json",
      "--markdown",
      "out.md",
      "--json",
      "out.json",
    ])
    .expect("parses") else {
      panic!("expected Run");
    };
    assert_eq!(args.cpp_log, Some(PathBuf::from("cpp.log")));
    assert_eq!(args.rust_history, Some(PathBuf::from("rust.json")));
    assert_eq!(args.markdown, Some(PathBuf::from("out.md")));
    assert_eq!(args.json, Some(PathBuf::from("out.json")));
  }

  #[test]
  fn help_short_circuits() {
    assert!(matches!(
      parse(&["--cpp-log", "a", "--help"]),
      Ok(ParsedArgs::Help)
    ));
  }

  #[test]
  fn rejects_unknown_missing_and_duplicate() {
    assert!(matches!(parse(&["--nope"]), Err(CliError::Usage(_))));
    assert!(matches!(parse(&["--cpp-log"]), Err(CliError::Usage(_))));
    assert!(matches!(
      parse(&["--cpp-log", "a", "--cpp-log", "b"]),
      Err(CliError::Usage(_))
    ));
  }
}
