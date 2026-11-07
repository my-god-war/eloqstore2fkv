//! `fkv_perf_cross` CLI 端到端测试：真实读写临时文件，跑编译出的二进制。

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};

const CPP_LOG: &str = "\
[2026-11-05 10:00:05] write summary 120000 kvs/s average latency 83 microseconds p99 140 microseconds p99.9 420 microseconds
[2026-11-05 10:00:06] read average 100000 QPS
[2026-11-05 10:00:07] read average 150000 QPS
[2026-11-05 10:00:08] read speed 12 MB/s, p99 250 microseconds
";

const RUST_HISTORY: &str = r#"{
  "workload": "write-read",
  "rows": 1000000,
  "runner": "linux-x86_64",
  "runs": [
    {"impl": "rust", "metric": "put_kv_s", "value": 108000},
    {"impl": "rust", "metric": "get_kv_s", "value": 270000},
    {"impl": "rust", "metric": "get_p99_us", "value": 240}
  ]
}"#;

fn bin() -> &'static str {
  env!("CARGO_BIN_EXE_fkv_perf_cross")
}

struct TempDir(PathBuf);

impl TempDir {
  fn new(label: &str) -> Self {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock after epoch")
      .as_nanos();
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
      "fkv-perf-cross-{label}-{}-{nanos}-{unique}",
      std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }

  fn write(&self, name: &str, contents: &str) -> PathBuf {
    let path = self.0.join(name);
    fs::write(&path, contents).expect("write fixture");
    path
  }
}

impl Drop for TempDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn writes_markdown_and_json_files() {
  let temp = TempDir::new("outputs");
  let cpp = temp.write("cpp.log", CPP_LOG);
  let rust = temp.write("history.json", RUST_HISTORY);
  let markdown_path = temp.path().join("report.md");
  let json_path = temp.path().join("report.json");

  let output = Command::new(bin())
    .args(["--cpp-log", cpp.to_str().unwrap()])
    .args(["--rust-history", rust.to_str().unwrap()])
    .args(["--markdown", markdown_path.to_str().unwrap()])
    .args(["--json", json_path.to_str().unwrap()])
    .output()
    .expect("run binary");

  assert!(
    output.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(output.stdout.is_empty(), "有输出文件时不应再打 stdout");

  let markdown = fs::read_to_string(&markdown_path).expect("markdown exists");
  assert!(markdown.contains("| put_kv_s | 120000.00 | 108000.00 | -10.0% | 0.9000 |"));

  let json: serde_json::Value =
    serde_json::from_str(&fs::read_to_string(&json_path).expect("json exists"))
      .expect("valid json");
  assert_eq!(json["runner"], "linux-x86_64");
  assert_eq!(json["rust"]["put_kv_s"], 108000.0);
}

#[test]
fn prints_markdown_to_stdout_without_output_files() {
  let temp = TempDir::new("stdout");
  let cpp = temp.write("cpp.log", CPP_LOG);
  let rust = temp.write("history.json", RUST_HISTORY);

  let output = Command::new(bin())
    .args(["--cpp-log", cpp.to_str().unwrap()])
    .args(["--rust-history", rust.to_str().unwrap()])
    .output()
    .expect("run binary");

  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
  assert!(stdout.starts_with("# C++ simple_bench vs Rust fkv_bench\n"));
  assert!(stdout.contains("| get_kv_s | 250000.00 | 270000.00 | +8.0% | 1.0800 | 是 | improved |"));
}

#[test]
fn usage_errors_exit_with_code_2() {
  let output = Command::new(bin())
    .arg("--bogus")
    .output()
    .expect("run binary");
  assert_eq!(output.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&output.stderr).contains("--bogus"));
}

#[test]
fn io_and_parse_errors_exit_with_code_1() {
  let temp = TempDir::new("errors");
  let cpp = temp.write("cpp.log", CPP_LOG);
  let rust = temp.write("history.json", RUST_HISTORY);

  let missing = Command::new(bin())
    .args(["--cpp-log", "/definitely/not/here.log"])
    .args(["--rust-history", rust.to_str().unwrap()])
    .output()
    .expect("run binary");
  assert_eq!(missing.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&missing.stderr).contains("读取"));

  let bad_log = temp.write("bad.log", "no metrics here");
  let bad = Command::new(bin())
    .args(["--cpp-log", bad_log.to_str().unwrap()])
    .args(["--rust-history", rust.to_str().unwrap()])
    .output()
    .expect("run binary");
  assert_eq!(bad.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&bad.stderr).contains("no C++ metrics parsed"));

  let bad_json = temp.write("bad.json", "{}");
  let bad = Command::new(bin())
    .args(["--cpp-log", cpp.to_str().unwrap()])
    .args(["--rust-history", bad_json.to_str().unwrap()])
    .output()
    .expect("run binary");
  assert_eq!(bad.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&bad.stderr).contains("no rust metrics in history"));
}
