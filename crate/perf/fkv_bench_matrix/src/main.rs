//! redb-style benchmark matrix harness.
//!
//! One [`BenchEngine`] trait drives both the C++ store and the Rust `fkv_bench`
//! binary.  The preferred C++ backend loads `libfkv_bench_ffi` at runtime via
//! `dlopen` ([`CppFfiEngine`]); the legacy process adapter ([`CppEngine`]) is
//! kept as a fallback for `simple_bench`.  The CLI prints a single Markdown
//! table (`phase | cpp | fkv | ratio`) plus uncompacted / compacted directory
//! sizes.
//!
//! Nothing links the C++ library at build time: `--cpp-lib` is resolved with
//! `dlopen`/`dlsym`, so `cargo build`/`cargo test` work without the `.so`.

use std::{
  collections::BTreeMap,
  env,
  ffi::{CStr, CString, c_char, c_int, c_void},
  fmt::Write as _,
  fs,
  io::{self, Read},
  path::{Path, PathBuf},
  process::{Command, ExitCode, Stdio},
  sync::{Mutex, OnceLock},
  time::{Duration, Instant},
};

use regex::Regex;
use serde::Serialize;

const USAGE: &str = "\
用法: fkv_bench_matrix (--cpp-lib <libfkv_bench_ffi.so> | --cpp-bin <simple_bench>)
                       --rust-bin <fkv_bench>
                       [--profile redb|legacy] [--workdir <dir>]
                       [--rows <n>] [--bulk-rows <n>] [--batch-size <n>]
                       [--kv-size <bytes>] [--key-size <bytes>]
                       [--cache-bytes <bytes>] [--read-secs <secs>]
                       [--readers <n[,n...]>] [--scan-bytes <bytes>]
                       [--individual-writes <n>] [--batch-writes <n>]
                       [--reads <n>] [--scans <n>] [--scan-len <n>]
                       [--removals <n>] [--samples <n>]
                       [--markdown <out.md>] [--json <out.json>]

在隔离的 store 目录里分别运行 C++ 引擎与 Rust fkv_bench，输出
`phase | cpp | fkv | ratio` Markdown 表，以及 uncompacted/compacted 体积行。

C++ 引擎选择:
  --cpp-lib <path>      C++ FFI 共享库（libfkv_bench_ffi.so / .dylib），首选；
                        dlopen 载入，支持全部 phase（len 为全量扫描计数）
  --cpp-bin <path>      兼容回退: 旧的 simple_bench 进程适配器，仅在未提供
                        --cpp-lib 时使用；不支持 len()

必填:
  --rust-bin <path>     Rust fkv_bench 可执行文件

选项:
  --workdir <dir>       工作目录（默认 /tmp/matrix）
  --profile <name>      redb（默认，5M/1M/1k/100x1000/1M/500k/10/500k）
                        或 legacy（旧的 200k + 时间窗读 / scan）
  --rows <n>            数据集 key 数（默认 1000000）
  --bulk-rows <n>       bulk load 写入的 key 数（默认 5000000）
  --batch-size <n>      批量写批大小（默认 1000）
  --kv-size <bytes>     整对 KV 宽度（redb 默认 174；legacy 128）
  --key-size <bytes>    key 宽度（redb 默认 24；legacy 8）
  --cache-bytes <bytes> 每 store 的总缓存预算（redb 默认 64 MiB；0=引擎默认）
  --read-secs <secs>    时间窗档的读/scan 窗口（redb 档用固定次数时忽略）
  --readers <list>      点读并发列表，逗号分隔（默认 1,4,8,16,32）
  --scan-bytes <bytes>  每个 scan 请求的字节窗口，0=扫到表尾
                        （redb 默认 scan_len × value_size = 1500）
  --individual-writes <n> 单条写次数（redb 默认 1000）
  --batch-writes <n>    批量写请求数（redb 默认 100）
  --reads <n>           点读总次数（redb 默认 1000000；0=时间窗）
  --scans <n>            range scan 请求数（redb 默认 500000；0=时间窗）
  --scan-len <n>         每次 range scan 的条数（redb 默认 10）
  --removals <n>         删除 key 数（redb 默认 500000）
  --phase-timeout-secs <n> 单个外部进程的墙钟上限（默认 1800；0 关闭）
  --samples <n>           读/scan/len 重复次数，取中位数（redb 默认 3）
  --markdown <out.md>   额外写一份 Markdown 报告
  --json <out.json>     额外写一份 JSON 报告
  -h, --help            显示本帮助
";

// ---------------------------------------------------------------------------
// Trait and data model
// ---------------------------------------------------------------------------

/// One benchmark phase.  `RandomReads` carries the reader concurrency so the
/// matrix can emit one row per `--readers` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
  BulkLoad,
  IndividualWrites,
  BatchWrites,
  RandomReads { readers: usize },
  RangeReads,
  Removals,
  Len,
}

impl Phase {
  fn slug(self) -> String {
    match self {
      Self::BulkLoad => "bulk_load".to_owned(),
      Self::IndividualWrites => "individual_writes".to_owned(),
      Self::BatchWrites => "batch_writes".to_owned(),
      Self::RandomReads { readers } => format!("random_reads_{readers}"),
      Self::RangeReads => "range_reads".to_owned(),
      Self::Removals => "removals".to_owned(),
      Self::Len => "len".to_owned(),
    }
  }

  fn label(self) -> String {
    match self {
      Self::BulkLoad => "bulk load".to_owned(),
      Self::IndividualWrites => "individual writes".to_owned(),
      Self::BatchWrites => "batch writes".to_owned(),
      Self::RandomReads { readers } => format!(
        "random reads ({readers} thread{})",
        if readers == 1 { "" } else { "s" }
      ),
      Self::RangeReads => "random range reads".to_owned(),
      Self::Removals => "removals".to_owned(),
      Self::Len => "len()".to_owned(),
    }
  }
}

/// Result of one phase on one engine.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseResult {
  pub ops: u64,
  #[serde(with = "duration_secs")]
  pub duration: Duration,
  /// Logical operations per second.
  pub throughput: f64,
  pub p99_us: Option<f64>,
  /// Optional store size (only useful for phases that leave a store behind).
  pub bytes: Option<u64>,
}

mod duration_secs {
  use std::time::Duration;

  use serde::Serializer;

  pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_f64(duration.as_secs_f64())
  }
}

/// Configuration shared by all engines.
#[derive(Debug, Clone)]
pub struct MatrixConfig {
  pub workdir: PathBuf,
  pub profile: Profile,
  pub rows: usize,
  /// Key count written by the bulk-load phase (redb: 5,000,000).
  pub bulk_rows: usize,
  pub batch_size: usize,
  pub kv_size: usize,
  /// Key width; the value width is `kv_size - key_size`.
  pub key_size: usize,
  /// Cache budget per store in bytes; 0 = engine default.
  pub cache_bytes: u64,
  pub read_secs: f64,
  pub readers: Vec<usize>,
  pub scan_bytes: u64,
  /// Single-key writes in the individual-writes phase.
  pub individual_writes: u64,
  /// Number of batch requests in the batch-writes phase.
  pub batch_writes: u64,
  /// Total point reads (0 = time window).
  pub reads: u64,
  /// Total range-scan requests (0 = time window).
  pub scans: u64,
  /// Entries per range scan (0 = `--scan-bytes` window).
  pub scan_len: usize,
  /// Keys removed in the removals phase.
  pub removals: u64,
  /// Repetitions for read / scan / len phases; the median is reported.
  pub samples: usize,
  /// 每个外部进程的墙钟上限（秒）；0 表示不设上限。
  pub phase_timeout_secs: u64,
}

/// Benchmark profile.  `Redb` mirrors redb's published workload; `Legacy`
/// keeps the pre-redb 200k / time-window shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
  Redb,
  Legacy,
}

impl Profile {
  fn parse(value: &str) -> Result<Self, String> {
    match value {
      "redb" => Ok(Self::Redb),
      "legacy" => Ok(Self::Legacy),
      other => Err(format!("--profile {other:?} 非法：可选 redb / legacy")),
    }
  }

  fn as_str(self) -> &'static str {
    match self {
      Self::Redb => "redb",
      Self::Legacy => "legacy",
    }
  }
}

impl MatrixConfig {
  fn batch_count(&self) -> usize {
    self.batch_writes.max(1) as usize
  }

  fn value_size(&self) -> usize {
    self.kv_size.saturating_sub(self.key_size)
  }

  fn engine_dir(&self, engine: &str) -> PathBuf {
    self.workdir.join(engine)
  }
}

/// The only abstraction the matrix needs.  Implementations may be process
/// adapters (as here) or in-process engines later.
pub trait BenchEngine {
  fn name(&self) -> &'static str;
  fn run(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String>;
  fn store_bytes(&self, cfg: &MatrixConfig) -> Result<u64, String>;
  fn compact(&self, cfg: &MatrixConfig) -> Result<Option<Duration>, String>;

  /// Whether the wrapped binary exposes this phase.  The legacy `simple_bench`
  /// process adapter ([`CppEngine`]) is the one engine without `Len`; `fkv` and
  /// the C++ FFI shim both report it via a full-scan count.
  fn supports(&self, _phase: Phase) -> bool {
    true
  }
}

// ---------------------------------------------------------------------------
// Small infrastructure helpers
// ---------------------------------------------------------------------------

fn io_err(context: &str, err: io::Error) -> String {
  format!("{context}: {err}")
}

fn reset_dir(dir: &Path) -> Result<(), String> {
  if dir.exists() {
    fs::remove_dir_all(dir).map_err(|err| io_err(&format!("remove {}", dir.display()), err))?;
  }
  fs::create_dir_all(dir).map_err(|err| io_err(&format!("create {}", dir.display()), err))
}

fn ensure_parent(path: &Path) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .map_err(|err| io_err(&format!("create {}", parent.display()), err))?;
  }
  Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
  match fs::remove_file(path) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
    Err(err) => Err(io_err(&format!("remove {}", path.display()), err)),
  }
}

/// Recursively sum regular-file bytes below `path`.
///
/// A missing path is treated as zero bytes; a missing intermediate directory
/// during traversal (for example a concurrent cleanup) is also skipped.
pub fn dir_size_bytes(path: &Path) -> io::Result<u64> {
  let metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
    Err(err) => return Err(err),
  };
  if !metadata.is_dir() {
    return Ok(metadata.len());
  }

  let mut total = 0u64;
  for entry in fs::read_dir(path)? {
    let entry = match entry {
      Ok(entry) => entry,
      Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
      Err(err) => return Err(err),
    };
    let file_type = match entry.file_type() {
      Ok(file_type) => file_type,
      Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
      Err(err) => return Err(err),
    };
    if file_type.is_dir() {
      total = total.saturating_add(dir_size_bytes(&entry.path())?);
    } else if file_type.is_file() {
      total = total.saturating_add(entry.metadata()?.len());
    }
  }
  Ok(total)
}

fn shell_join(program: &Path, args: &[String]) -> String {
  let mut out = String::new();
  out.push_str(&shell_quote(&program.display().to_string()));
  for arg in args {
    out.push(' ');
    out.push_str(&shell_quote(arg));
  }
  out
}

fn shell_quote(value: &str) -> String {
  if !value.is_empty()
    && value
      .chars()
      .all(|ch| ch.is_ascii_alphanumeric() || "/._-:=+,".contains(ch))
  {
    value.to_owned()
  } else {
    format!("'{}'", value.replace('\'', "'\\''"))
  }
}

fn log_tail(text: &str, lines: usize) -> String {
  let all: Vec<&str> = text.lines().collect();
  let start = all.len().saturating_sub(lines);
  all[start..].join("\n")
}

/// Run a process, tee stdout/stderr into `log_path`, and return the combined
/// text.  A non-zero exit status is reported with the command and log tail.
/// Run one benchmark process, capturing stdout/stderr into `log_path`.
///
/// `timeout` is a wall-clock cap; on expiry the child is killed and the phase
/// fails loudly instead of hanging the whole job (a stuck reader phase
/// otherwise burns the CI job limit silently).
fn execute(
  bin: &Path,
  args: &[String],
  log_path: &Path,
  timeout: Option<Duration>,
) -> Result<String, String> {
  ensure_parent(log_path)?;
  let command_line = shell_join(bin, args);
  let mut child = Command::new(bin)
    .args(args)
    .env("GLOG_logtostderr", "1")
    .env("GLOG_minloglevel", "0")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| {
      format!(
        "failed to execute {}: {err}（检查二进制路径与可执行权限）",
        bin.display()
      )
    })?;

  // Drain both pipes on helper threads: a phase writing more than the pipe
  // buffer would otherwise deadlock against our wait loop.
  let mut child_stdout = child.stdout.take().expect("piped stdout");
  let mut child_stderr = child.stderr.take().expect("piped stderr");
  let stdout_reader = std::thread::spawn(move || {
    let mut buf = Vec::new();
    let _ = child_stdout.read_to_end(&mut buf);
    buf
  });
  let stderr_reader = std::thread::spawn(move || {
    let mut buf = Vec::new();
    let _ = child_stderr.read_to_end(&mut buf);
    buf
  });

  let started = Instant::now();
  let mut timed_out = false;
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) => {}
      Err(err) => {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("wait {}: {err}", bin.display()));
      }
    }
    if let Some(limit) = timeout {
      if started.elapsed() >= limit {
        let _ = child.kill();
        let _ = child.wait();
        timed_out = true;
        break Default::default();
      }
    }
    std::thread::sleep(Duration::from_millis(100));
  };
  let stdout = stdout_reader.join().unwrap_or_default();
  let stderr = stderr_reader.join().unwrap_or_default();

  let mut text = String::new();
  let _ = writeln!(text, "$ {command_line}");
  let _ = writeln!(text, "--- stdout ---");
  text.push_str(&String::from_utf8_lossy(&stdout));
  let _ = writeln!(text, "\n--- stderr ---");
  text.push_str(&String::from_utf8_lossy(&stderr));
  let _ = fs::write(log_path, &text);

  if timed_out {
    let limit = timeout.map(|t| t.as_secs()).unwrap_or_default();
    return Err(format!(
      "命令超过 {limit}s 未结束，已终止（--phase-timeout-secs，0 关闭）\n$ {command_line}\nlog: {}\n--- log tail ---\n{}",
      log_path.display(),
      log_tail(&text, 40)
    ));
  }
  if !status.success() {
    return Err(format!(
      "command failed ({status})\n$ {command_line}\nlog: {}\n--- log tail ---\n{}",
      log_path.display(),
      log_tail(&text, 40)
    ));
  }
  Ok(text)
}

/// Convert the config's seconds cap into an optional [`Duration`].
fn phase_timeout(cfg: &MatrixConfig) -> Option<Duration> {
  (cfg.phase_timeout_secs > 0).then(|| Duration::from_secs(cfg.phase_timeout_secs))
}

/// Verify that a binary's `--help` advertises the flags we are about to use.
///
/// This is the "binary too old" guard: the harness never silently drops a
/// flag or falls back to a different workload.
fn verify_binary(bin: &Path, required: &[&str]) -> Result<(), String> {
  let output = Command::new(bin).arg("--help").output().map_err(|err| {
    format!(
      "failed to execute {} --help: {err}（检查 --cpp-bin / --rust-bin）",
      bin.display()
    )
  })?;
  let mut help = String::from_utf8_lossy(&output.stdout).into_owned();
  help.push_str(&String::from_utf8_lossy(&output.stderr));

  let missing: Vec<&str> = required
    .iter()
    .copied()
    .filter(|flag| !help.contains(&format!("--{flag}")))
    .collect();
  if missing.is_empty() {
    Ok(())
  } else {
    Err(format!(
      "{} is too old: --help does not advertise {}; rebuild the benchmark binary",
      bin.display(),
      missing
        .iter()
        .map(|flag| format!("--{flag}"))
        .collect::<Vec<_>>()
        .join(", ")
    ))
  }
}

fn last_store(store: &Mutex<Option<PathBuf>>) -> Result<PathBuf, String> {
  store
    .lock()
    .map_err(|_| "internal error: store path lock poisoned".to_owned())?
    .clone()
    .ok_or_else(|| "no store from a previous run; run BulkLoad first".to_owned())
}

fn set_last_store(store: &Mutex<Option<PathBuf>>, path: &Path) -> Result<(), String> {
  *store
    .lock()
    .map_err(|_| "internal error: store path lock poisoned".to_owned())? = Some(path.to_owned());
  Ok(())
}

fn read_required_file(path: &Path) -> Result<String, String> {
  fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))
}

fn metric<'a>(metrics: &'a BTreeMap<String, f64>, name: &str) -> Option<f64> {
  metrics.get(name).copied().filter(|value| value.is_finite())
}

fn require_positive_throughput(metrics: &BTreeMap<String, f64>, name: &str) -> Result<f64, String> {
  match metric(metrics, name) {
    Some(value) if value > 0.0 => Ok(value),
    Some(value) => Err(format!("metric {name} must be positive, got {value}")),
    None => Err(format!(
      "missing metric {name}; parsed metrics: {}",
      metrics.keys().cloned().collect::<Vec<_>>().join(", ")
    )),
  }
}

fn result_from_total(
  metrics: &BTreeMap<String, f64>,
  throughput_key: &str,
  p99_key: &str,
  ops: u64,
) -> Result<PhaseResult, String> {
  let throughput = require_positive_throughput(metrics, throughput_key)?;
  if ops == 0 {
    return Err("cannot compute duration for zero operations".to_owned());
  }
  Ok(PhaseResult {
    ops,
    duration: Duration::from_secs_f64(ops as f64 / throughput),
    throughput,
    p99_us: metric(metrics, p99_key),
    bytes: None,
  })
}

fn result_from_window(
  metrics: &BTreeMap<String, f64>,
  throughput_key: &str,
  p99_key: &str,
  read_secs: f64,
) -> Result<PhaseResult, String> {
  let throughput = require_positive_throughput(metrics, throughput_key)?;
  Ok(PhaseResult {
    ops: (throughput * read_secs).round().max(0.0) as u64,
    duration: Duration::from_secs_f64(read_secs),
    throughput,
    p99_us: metric(metrics, p99_key),
    bytes: None,
  })
}

/// Map the `len` phase's full-scan history into a [`PhaseResult`].
///
/// `fkv_bench --read-only-existing --workload scan --test-secs 0` reports
/// `fullscan_num` (live entries counted in one full-scan round) and
/// `fullscan_total_us` (wall time of that round).  The resulting throughput is
/// therefore the scan rate, not an O(1) length-lookup rate.
fn result_from_fullscan(metrics: &BTreeMap<String, f64>) -> Result<PhaseResult, String> {
  let missing = |name: &str| {
    format!(
      "missing metric {name}; parsed metrics: {}",
      metrics.keys().cloned().collect::<Vec<_>>().join(", ")
    )
  };
  let num = metric(metrics, "fullscan_num").ok_or_else(|| missing("fullscan_num"))?;
  let total_us =
    metric(metrics, "fullscan_total_us").ok_or_else(|| missing("fullscan_total_us"))?;
  if total_us <= 0.0 {
    return Err(format!(
      "fullscan_total_us must be positive, got {total_us}"
    ));
  }
  let secs = total_us / 1_000_000.0;
  let ops = num.max(0.0) as u64;
  Ok(PhaseResult {
    ops,
    duration: Duration::from_secs_f64(secs),
    throughput: ops as f64 / secs,
    p99_us: None,
    bytes: None,
  })
}

// ---------------------------------------------------------------------------
// C++ simple_bench adapter
// ---------------------------------------------------------------------------

const CPP_REQUIRED_FLAGS: &[&str] = &[
  "load",
  "workload",
  "kvoptions",
  "kv_size",
  "batch_size",
  "write_batchs",
  "partitions",
  "max_key",
  "read_per_part",
  "read_thds",
  "test_secs",
  "scan_bytes",
  "no_delete",
  "delete_only",
  "show_write_perf",
];

/// Process adapter for the C++ `simple_bench` binary.
#[derive(Debug)]
pub struct CppEngine {
  bin: PathBuf,
  last_store: Mutex<Option<PathBuf>>,
  verified: OnceLock<Result<(), String>>,
}

impl CppEngine {
  #[must_use]
  pub fn new(bin: impl Into<PathBuf>) -> Self {
    Self {
      bin: bin.into(),
      last_store: Mutex::new(None),
      verified: OnceLock::new(),
    }
  }

  fn ensure_verified(&self) -> Result<(), String> {
    self
      .verified
      .get_or_init(|| verify_binary(&self.bin, CPP_REQUIRED_FLAGS))
      .clone()
  }

  fn store_dir(&self, cfg: &MatrixConfig, phase: Phase) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("stores")
      .join(phase.slug())
  }

  fn log_path(&self, cfg: &MatrixConfig, phase: Phase, label: &str) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("logs")
      .join(phase.slug())
      .join(format!("{label}.log"))
  }

  fn ini_path(&self, cfg: &MatrixConfig, phase: Phase) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("ini")
      .join(format!("{}.ini", phase.slug()))
  }

  fn write_ini(
    &self,
    cfg: &MatrixConfig,
    phase: Phase,
    store_dir: &Path,
  ) -> Result<PathBuf, String> {
    let path = self.ini_path(cfg, phase);
    ensure_parent(&path)?;
    let contents = format!(
      "[run]\nnum_threads = 1\n\n[permanent]\nstore_path = {}\n",
      store_dir.display()
    );
    fs::write(&path, contents).map_err(|err| io_err(&format!("write {}", path.display()), err))?;
    Ok(path)
  }

  fn exec(
    &self,
    cfg: &MatrixConfig,
    phase: Phase,
    label: &str,
    args: &[String],
  ) -> Result<String, String> {
    execute(
      &self.bin,
      args,
      &self.log_path(cfg, phase, label),
      phase_timeout(cfg),
    )
  }

  fn run_phase(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String> {
    let store_dir = self.store_dir(cfg, phase);
    reset_dir(&store_dir)?;
    let ini = self.write_ini(cfg, phase, &store_dir)?;

    let result = match phase {
      Phase::BulkLoad => {
        let args = cpp_load_args(cfg, &ini);
        let log = self.exec(cfg, phase, "load", &args)?;
        result_from_total(
          &cpp_metrics(&log)?,
          "put_kv_s",
          "put_p99_us",
          cfg.rows as u64,
        )
      }
      Phase::IndividualWrites => {
        let args = cpp_write_args(cfg, &ini, 1, cfg.rows);
        let log = self.exec(cfg, phase, "write", &args)?;
        result_from_total(
          &cpp_metrics(&log)?,
          "put_kv_s",
          "put_p99_us",
          cfg.rows as u64,
        )
      }
      Phase::BatchWrites => {
        let batches = cfg.batch_count();
        let ops = batches as u64 * cfg.batch_size as u64;
        let args = cpp_write_args(cfg, &ini, cfg.batch_size, batches);
        let log = self.exec(cfg, phase, "write", &args)?;
        result_from_total(&cpp_metrics(&log)?, "put_kv_s", "put_p99_us", ops)
      }
      Phase::RandomReads { readers } => {
        let load = cpp_load_args(cfg, &ini);
        self.exec(cfg, phase, "load", &load)?;
        let args = cpp_read_args(cfg, &ini, readers);
        let log = self.exec(cfg, phase, "read", &args)?;
        result_from_window(&cpp_metrics(&log)?, "get_kv_s", "get_p99_us", cfg.read_secs)
      }
      Phase::RangeReads => {
        let load = cpp_load_args(cfg, &ini);
        self.exec(cfg, phase, "load", &load)?;
        let args = cpp_scan_args(cfg, &ini);
        let log = self.exec(cfg, phase, "scan", &args)?;
        result_from_window(
          &cpp_metrics(&log)?,
          "scan_kv_s",
          "scan_p99_us",
          cfg.read_secs,
        )
      }
      Phase::Removals => {
        let load = cpp_load_args(cfg, &ini);
        self.exec(cfg, phase, "load", &load)?;
        let batches = cfg.batch_count();
        let ops = batches as u64 * cfg.batch_size as u64;
        let args = cpp_delete_args(cfg, &ini, cfg.batch_size, batches);
        let log = self.exec(cfg, phase, "delete", &args)?;
        result_from_total(&cpp_metrics(&log)?, "put_kv_s", "put_p99_us", ops)
      }
      Phase::Len => {
        return Err("C++ simple_bench has no len API".to_owned());
      }
    };
    set_last_store(&self.last_store, &store_dir)?;
    result
  }
}

impl BenchEngine for CppEngine {
  fn name(&self) -> &'static str {
    "cpp"
  }

  fn run(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String> {
    if phase == Phase::Len {
      return Err("C++ simple_bench has no len API".to_owned());
    }
    if cfg.profile != Profile::Legacy {
      return Err(
        "--cpp-bin simple_bench 只支持 --profile legacy；redb 口径请用 --cpp-lib \
         （FFI shim 支持固定次数与线程数）"
          .to_owned(),
      );
    }
    self.ensure_verified()?;
    self.run_phase(phase, cfg)
  }

  fn store_bytes(&self, _cfg: &MatrixConfig) -> Result<u64, String> {
    let dir = last_store(&self.last_store)?;
    if !dir.exists() {
      return Err(format!("store directory {} does not exist", dir.display()));
    }
    dir_size_bytes(&dir).map_err(|err| io_err(&format!("size {}", dir.display()), err))
  }

  fn compact(&self, cfg: &MatrixConfig) -> Result<Option<Duration>, String> {
    let dir = last_store(&self.last_store)?;
    if !dir.exists() {
      return Err(format!("store directory {} does not exist", dir.display()));
    }
    self.ensure_verified()?;
    let phase = Phase::BulkLoad;
    let ini = self.write_ini(cfg, phase, &dir)?;
    let args = cpp_compact_args(cfg, &ini);
    let log = self.exec(cfg, phase, "compact", &args)?;
    parse_cpp_compact_duration(&log).map(Some)
  }

  fn supports(&self, phase: Phase) -> bool {
    phase != Phase::Len
  }
}

// ---------------------------------------------------------------------------
// C++ FFI adapter
// ---------------------------------------------------------------------------
//
// The shared library is loaded at runtime so this crate never links the C++
// store at build time.  ABI v2 is declared in `cpp/bench_ffi/bench_ffi.h`:
//
//   uint32_t     fkv_bench_abi_version(void);
//   FkvBenchFfi *fkv_bench_open(const char *root,
//                               const FkvBenchConfig *config);
//   void         fkv_bench_close(FkvBenchFfi *h);
//   int          fkv_bench_run(FkvBenchFfi *h, const char *phase,
//                              uint32_t readers, double read_secs,
//                              FkvBenchResult *out);
//   const char  *fkv_bench_last_error(FkvBenchFfi *h);

/// ABI version this crate expects.  Bumped when `fkv_bench_open` changed from
/// positional arguments to `FkvBenchConfig`.
const FFI_ABI_VERSION: u32 = 2;

/// Must stay layout-identical to `FkvBenchConfig` in `bench_ffi.h`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FfiConfig {
  rows: u32,
  bulk_rows: u32,
  batch_size: u32,
  kv_size: u32,
  partitions: u32,
  key_size: u32,
  num_threads: u32,
  buffer_pool_size: u64,
  individual_writes: u32,
  batch_writes: u32,
  removals: u32,
  reads: u32,
  range_scans: u32,
  range_len: u32,
}

/// C ABI result struct.  Layout must stay identical to `FkvBenchResult` in the
/// C++ shim.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FfiBenchResult {
  ops: u64,
  secs: f64,
  p99_us: f64,
}

type FfiAbiFn = unsafe extern "C" fn() -> u32;
type FfiOpenFn = unsafe extern "C" fn(*const c_char, *const FfiConfig) -> *mut c_void;
type FfiCloseFn = unsafe extern "C" fn(*mut c_void);
type FfiRunFn =
  unsafe extern "C" fn(*mut c_void, *const c_char, u32, f64, *mut FfiBenchResult) -> c_int;
type FfiLastErrorFn = unsafe extern "C" fn(*mut c_void) -> *const c_char;

/// Resolved function pointers from `libfkv_bench_ffi`.
struct FfiApi {
  open: FfiOpenFn,
  close: FfiCloseFn,
  run: FfiRunFn,
  last_error: FfiLastErrorFn,
}

/// Owns the `dlopen` handle; dropping it unloads the library.
struct FfiLibrary {
  raw: *mut c_void,
  api: FfiApi,
}

// The library is only touched from the matrix thread that owns this engine.
unsafe impl Send for FfiLibrary {}

impl Drop for FfiLibrary {
  fn drop(&mut self) {
    unsafe {
      libc::dlclose(self.raw);
    }
  }
}

/// Opaque `FkvBenchFfi *` wrapped so it can live in a `Mutex`.
struct FfiHandle(*mut c_void);

unsafe impl Send for FfiHandle {}

fn ffi_dl_error() -> String {
  let err = unsafe { libc::dlerror() };
  if err.is_null() {
    "unknown dynamic loader error".to_owned()
  } else {
    unsafe { CStr::from_ptr(err) }
      .to_string_lossy()
      .into_owned()
  }
}

fn ffi_resolve<T: Copy>(lib: *mut c_void, name: &str) -> Result<T, String> {
  let cname =
    CString::new(name).map_err(|_| format!("symbol name {name:?} contains a NUL byte"))?;
  let ptr = unsafe { libc::dlsym(lib, cname.as_ptr()) };
  if ptr.is_null() {
    return Err(format!(
      "libfkv_bench_ffi does not export {name}: {}",
      ffi_dl_error()
    ));
  }
  // Function pointers and `void *` are pointer-sized on all supported targets.
  Ok(unsafe { std::mem::transmute_copy(&ptr) })
}

impl FfiApi {
  fn load(lib: *mut c_void) -> Result<Self, String> {
    let abi_version: FfiAbiFn = ffi_resolve(lib, "fkv_bench_abi_version").map_err(|_| {
      "libfkv_bench_ffi predates ABI v2 (no fkv_bench_abi_version); \
       rebuild cpp/bench_ffi"
        .to_owned()
    })?;
    let version = unsafe { abi_version() };
    if version != FFI_ABI_VERSION {
      return Err(format!(
        "libfkv_bench_ffi ABI {version} does not match expected {FFI_ABI_VERSION}; \
         rebuild cpp/bench_ffi"
      ));
    }
    Ok(Self {
      open: ffi_resolve(lib, "fkv_bench_open")?,
      close: ffi_resolve(lib, "fkv_bench_close")?,
      run: ffi_resolve(lib, "fkv_bench_run")?,
      last_error: ffi_resolve(lib, "fkv_bench_last_error")?,
    })
  }
}

fn ffi_phase_name(phase: Phase) -> &'static str {
  match phase {
    Phase::BulkLoad => "bulk_load",
    Phase::IndividualWrites => "individual_writes",
    Phase::BatchWrites => "batch_writes",
    Phase::RandomReads { .. } => "random_reads",
    Phase::RangeReads => "range_reads",
    Phase::Removals => "removals",
    Phase::Len => "len",
  }
}

/// 点读阶段的客户端读线程数（作为 FFI `run(readers)` 的实参）。
///
/// 不要把它和 EloqStore 的 `num_threads` 混用：后者是 **shard / worker 线程数**。
/// 本矩阵只有 partition 0，按 `partition_id % shard_num` 恒路由到 shard 0；把
/// `num_threads` 设成 readers（最多 32）等于为单分区起 32 个 shard，每个都要
/// io_uring 环与注册内存，实测在 2 vCPU runner 上 io_uring 初始化返回 -12
/// (ENOMEM)、整个 store 起不来（run 42385622423 的 32 线程阶段）。
fn ffi_readers(phase: Phase) -> u32 {
  match phase {
    Phase::RandomReads { readers } => readers.min(u32::MAX as usize) as u32,
    _ => 1,
  }
}

/// Per-phase `FkvBenchConfig`.
fn ffi_config(_phase: Phase, cfg: &MatrixConfig) -> FfiConfig {
  FfiConfig {
    rows: cfg.rows as u32,
    bulk_rows: cfg.bulk_rows as u32,
    batch_size: cfg.batch_size as u32,
    kv_size: cfg.kv_size as u32,
    partitions: 1,
    key_size: cfg.key_size as u32,
    num_threads: 1,
    buffer_pool_size: cfg.cache_bytes,
    individual_writes: cfg.individual_writes as u32,
    batch_writes: cfg.batch_writes as u32,
    removals: cfg.removals as u32,
    reads: cfg.reads as u32,
    range_scans: cfg.scans as u32,
    range_len: cfg.scan_len as u32,
  }
}

/// Repetitions of one phase; only the read / scan / len lanes are repeated.
fn phase_samples(phase: Phase, cfg: &MatrixConfig) -> usize {
  match phase {
    Phase::RandomReads { .. } | Phase::RangeReads | Phase::Len => cfg.samples.max(1),
    _ => 1,
  }
}

/// Median (by throughput) of repeated phase runs.  `results` is never empty.
fn median_phase_result(mut results: Vec<PhaseResult>) -> PhaseResult {
  results.sort_by(|left, right| left.throughput.total_cmp(&right.throughput));
  results.remove(results.len() / 2)
}

fn result_from_ffi(raw: FfiBenchResult, phase: &str) -> Result<PhaseResult, String> {
  if !raw.secs.is_finite() || raw.secs <= 0.0 {
    return Err(format!(
      "fkv_bench_run({phase}) returned invalid duration: {} s",
      raw.secs
    ));
  }
  Ok(PhaseResult {
    ops: raw.ops,
    duration: Duration::from_secs_f64(raw.secs),
    throughput: raw.ops as f64 / raw.secs,
    p99_us: (raw.p99_us.is_finite() && raw.p99_us > 0.0).then_some(raw.p99_us),
    bytes: None,
  })
}

/// In-process adapter over the C++ FFI shim (`--cpp-lib`).
///
/// `BulkLoad` keeps its handle alive so the following `compact` can reuse the
/// loaded store.  Every other phase opens and closes its own handle.  Starting
/// a new run always closes any retained handle first, so a `BulkLoad` re-run
/// (for example the compaction measurement) never shares a directory with a
/// live handle.
pub struct CppFfiEngine {
  lib_path: PathBuf,
  lib: FfiLibrary,
  handle: Mutex<Option<FfiHandle>>,
  last_root: Mutex<Option<PathBuf>>,
}

impl CppFfiEngine {
  pub fn new(path: impl Into<PathBuf>) -> Result<Self, String> {
    let lib_path = path.into();
    let cpath = CString::new(lib_path.display().to_string())
      .map_err(|_| format!("--cpp-lib path {:?} contains a NUL byte", lib_path))?;
    let raw = unsafe { libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW) };
    if raw.is_null() {
      return Err(format!(
        "dlopen {} failed: {}（检查 --cpp-lib 路径以及动态库依赖）",
        lib_path.display(),
        ffi_dl_error()
      ));
    }
    let api = match FfiApi::load(raw) {
      Ok(api) => api,
      Err(err) => {
        unsafe {
          libc::dlclose(raw);
        }
        return Err(err);
      }
    };
    Ok(Self {
      lib_path,
      lib: FfiLibrary { raw, api },
      handle: Mutex::new(None),
      last_root: Mutex::new(None),
    })
  }

  fn root_dir(cfg: &MatrixConfig, phase: Phase) -> PathBuf {
    cfg.workdir.join("cpp-ffi").join(phase.slug())
  }

  fn last_error(&self, handle: *mut c_void) -> String {
    let ptr = unsafe { (self.lib.api.last_error)(handle) };
    if ptr.is_null() {
      "no error message".to_owned()
    } else {
      unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
    }
  }

  fn close_handle(&self) -> Result<(), String> {
    let handle = self
      .handle
      .lock()
      .map_err(|_| "internal error: cpp-ffi handle lock poisoned".to_owned())?
      .take();
    if let Some(handle) = handle {
      unsafe {
        (self.lib.api.close)(handle.0);
      }
    }
    Ok(())
  }

  fn call_run(
    &self,
    handle: *mut c_void,
    phase: &str,
    readers: u32,
    read_secs: f64,
  ) -> Result<FfiBenchResult, String> {
    let cphase =
      CString::new(phase).map_err(|_| format!("phase name {phase:?} contains a NUL byte"))?;
    let mut out = FfiBenchResult::default();
    let rc = unsafe { (self.lib.api.run)(handle, cphase.as_ptr(), readers, read_secs, &mut out) };
    if rc != 0 {
      return Err(format!(
        "fkv_bench_run({phase}) failed (rc={rc}): {}",
        self.last_error(handle)
      ));
    }
    Ok(out)
  }
}

impl std::fmt::Debug for CppFfiEngine {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CppFfiEngine")
      .field("lib_path", &self.lib_path)
      .finish_non_exhaustive()
  }
}

impl BenchEngine for CppFfiEngine {
  fn name(&self) -> &'static str {
    "cpp"
  }

  fn run(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String> {
    // A retained BulkLoad handle must never share its directory with a new
    // run, which is about to reset that directory.
    self.close_handle()?;

    let root = Self::root_dir(cfg, phase);
    reset_dir(&root)?;
    let croot = CString::new(root.display().to_string())
      .map_err(|_| format!("store path {:?} contains a NUL byte", root))?;
    let ffi_cfg = ffi_config(phase, cfg);
    let handle = unsafe { (self.lib.api.open)(croot.as_ptr(), &ffi_cfg) };
    if handle.is_null() {
      return Err(format!(
        "fkv_bench_open({}) returned null: {}",
        root.display(),
        self.last_error(handle)
      ));
    }
    set_last_store(&self.last_root, &root)?;

    let phase_name = ffi_phase_name(phase);
    let readers = ffi_readers(phase);
    let mut phase_results = Vec::with_capacity(phase_samples(phase, cfg));
    let mut failure = None;
    for _ in 0..phase_samples(phase, cfg) {
      match self
        .call_run(handle, phase_name, readers, 0.0)
        .and_then(|raw| result_from_ffi(raw, phase_name))
      {
        Ok(result) => phase_results.push(result),
        Err(err) => {
          failure = Some(err);
          break;
        }
      }
    }
    match failure {
      None => {
        if phase == Phase::BulkLoad {
          *self
            .handle
            .lock()
            .map_err(|_| "internal error: cpp-ffi handle lock poisoned".to_owned())? =
            Some(FfiHandle(handle));
        } else {
          unsafe {
            (self.lib.api.close)(handle);
          }
        }
        Ok(median_phase_result(phase_results))
      }
      Some(err) => {
        unsafe {
          (self.lib.api.close)(handle);
        }
        Err(err)
      }
    }
  }

  fn store_bytes(&self, _cfg: &MatrixConfig) -> Result<u64, String> {
    let dir = last_store(&self.last_root)?;
    if !dir.exists() {
      return Err(format!("store directory {} does not exist", dir.display()));
    }
    dir_size_bytes(&dir).map_err(|err| io_err(&format!("size {}", dir.display()), err))
  }

  fn compact(&self, _cfg: &MatrixConfig) -> Result<Option<Duration>, String> {
    let handle = self
      .handle
      .lock()
      .map_err(|_| "internal error: cpp-ffi handle lock poisoned".to_owned())?
      .take()
      .ok_or_else(|| "no retained cpp-ffi handle; run BulkLoad first".to_owned())?;
    let outcome = self.call_run(handle.0, "compact", 1, 0.0);
    unsafe {
      (self.lib.api.close)(handle.0);
    }
    let raw = outcome?;
    if !raw.secs.is_finite() || raw.secs < 0.0 {
      return Err(format!(
        "fkv_bench_run(compact) returned invalid duration: {} s",
        raw.secs
      ));
    }
    Ok(Some(Duration::from_secs_f64(raw.secs)))
  }

  /// The FFI shim implements every phase; `len` is a full scan, not O(1).
  fn supports(&self, _phase: Phase) -> bool {
    true
  }
}

impl Drop for CppFfiEngine {
  fn drop(&mut self) {
    let handle = self.handle.lock().ok().and_then(|mut guard| guard.take());
    if let Some(handle) = handle {
      unsafe {
        (self.lib.api.close)(handle.0);
      }
    }
  }
}

fn cpp_base_args(cfg: &MatrixConfig, ini: &Path) -> Vec<String> {
  vec![
    "--kvoptions".to_owned(),
    ini.display().to_string(),
    "--kv_size".to_owned(),
    cfg.kv_size.to_string(),
    "--partitions".to_owned(),
    "1".to_owned(),
    "--max_key".to_owned(),
    cfg.rows.to_string(),
    "--scan_bytes".to_owned(),
    cfg.scan_bytes.to_string(),
    "--show_write_perf=true".to_owned(),
  ]
}

pub fn cpp_load_args(cfg: &MatrixConfig, ini: &Path) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend([
    "--load=1".to_owned(),
    "--batch_size".to_owned(),
    cfg.batch_size.to_string(),
  ]);
  args
}

pub fn cpp_write_args(
  cfg: &MatrixConfig,
  ini: &Path,
  batch_size: usize,
  batches: usize,
) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend([
    "--workload".to_owned(),
    "write".to_owned(),
    "--batch_size".to_owned(),
    batch_size.to_string(),
    "--write_batchs".to_owned(),
    batches.to_string(),
    "--no_delete=1".to_owned(),
  ]);
  args
}

pub fn cpp_read_args(cfg: &MatrixConfig, ini: &Path, readers: usize) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend([
    "--workload".to_owned(),
    "read".to_owned(),
    "--read_per_part".to_owned(),
    readers.to_string(),
    "--read_thds".to_owned(),
    "1".to_owned(),
    "--test_secs".to_owned(),
    fmt_secs(cfg.read_secs),
  ]);
  args
}

pub fn cpp_scan_args(cfg: &MatrixConfig, ini: &Path) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend([
    "--workload".to_owned(),
    "scan".to_owned(),
    "--read_per_part".to_owned(),
    "1".to_owned(),
    "--read_thds".to_owned(),
    "1".to_owned(),
    "--test_secs".to_owned(),
    fmt_secs(cfg.read_secs),
  ]);
  args
}

pub fn cpp_delete_args(
  cfg: &MatrixConfig,
  ini: &Path,
  batch_size: usize,
  batches: usize,
) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend([
    "--workload".to_owned(),
    "delete".to_owned(),
    "--delete_only=1".to_owned(),
    "--batch_size".to_owned(),
    batch_size.to_string(),
    "--write_batchs".to_owned(),
    batches.to_string(),
  ]);
  args
}

pub fn cpp_compact_args(cfg: &MatrixConfig, ini: &Path) -> Vec<String> {
  let mut args = cpp_base_args(cfg, ini);
  args.extend(["--workload".to_owned(), "compact".to_owned()]);
  args
}

fn cpp_metrics(log: &str) -> Result<BTreeMap<String, f64>, String> {
  let mut metrics = BTreeMap::new();

  let write_re = Regex::new(
    r"write summary (\d+) kvs/s .*?average latency (\d+) microseconds .*?p99 (\d+) microseconds",
  )
  .expect("write regex is valid");
  if let Some(caps) = write_re.captures(log) {
    metrics.insert("put_kv_s".to_owned(), capture_num(&caps, 1, "put_kv_s")?);
    metrics.insert(
      "put_avg_us".to_owned(),
      capture_num(&caps, 2, "put_avg_us")?,
    );
    metrics.insert(
      "put_p99_us".to_owned(),
      capture_num(&caps, 3, "put_p99_us")?,
    );
  }

  let read_qps_re = Regex::new(r"read average (\d+) QPS").expect("read qps regex is valid");
  let mut read_qps = 0.0;
  let mut saw_read = false;
  for caps in read_qps_re.captures_iter(log) {
    read_qps += capture_num(&caps, 1, "get_kv_s")?;
    saw_read = true;
  }
  if saw_read {
    metrics.insert("get_kv_s".to_owned(), read_qps);
  }

  if let Some(value) = max_capture(log, r"read speed .*?p99 (\d+) microseconds", "get_p99_us")? {
    metrics.insert("get_p99_us".to_owned(), value);
  }

  // Scan logs are periodic windows.  The last window is the closest equivalent
  // to an end-of-run throughput sample; summing windows would overcount.
  if let Some(value) = last_capture(log, r"scan throughput \d+ MB/s (\d+) kvs/s", "scan_kv_s")? {
    metrics.insert("scan_kv_s".to_owned(), value);
  }
  if let Some(value) = max_capture(log, r"scan speed .*?p99 (\d+) microseconds", "scan_p99_us")? {
    metrics.insert("scan_p99_us".to_owned(), value);
  }

  if metrics.is_empty() {
    return Err(format!(
      "no C++ metrics parsed; check simple_bench log format\n--- log tail ---\n{}",
      log_tail(log, 25)
    ));
  }
  Ok(metrics)
}

fn parse_cpp_compact_duration(log: &str) -> Result<Duration, String> {
  let re = Regex::new(r"compact total (\d+) ms").expect("compact regex is valid");
  let caps = re.captures(log).ok_or_else(|| {
    format!(
      "missing `compact total N ms` in log\n--- log tail ---\n{}",
      log_tail(log, 25)
    )
  })?;
  let millis = capture_num(&caps, 1, "compact_ms")?;
  Ok(Duration::from_secs_f64(millis / 1000.0))
}

fn capture_num(caps: &regex::Captures<'_>, index: usize, name: &str) -> Result<f64, String> {
  let raw = caps
    .get(index)
    .ok_or_else(|| format!("missing capture {index} for {name}"))?
    .as_str();
  raw
    .parse::<f64>()
    .map_err(|_| format!("{name} is not a number: {raw}"))
}

fn max_capture(log: &str, pattern: &str, name: &str) -> Result<Option<f64>, String> {
  let re = Regex::new(pattern).map_err(|err| format!("invalid {name} regex: {err}"))?;
  let mut max: Option<f64> = None;
  for caps in re.captures_iter(log) {
    let value = capture_num(&caps, 1, name)?;
    max = Some(max.map_or(value, |current| current.max(value)));
  }
  Ok(max)
}

fn last_capture(log: &str, pattern: &str, name: &str) -> Result<Option<f64>, String> {
  let re = Regex::new(pattern).map_err(|err| format!("invalid {name} regex: {err}"))?;
  let mut last = None;
  for caps in re.captures_iter(log) {
    last = Some(capture_num(&caps, 1, name)?);
  }
  Ok(last)
}

// ---------------------------------------------------------------------------
// Rust fkv_bench adapter
// ---------------------------------------------------------------------------

const FKV_REQUIRED_FLAGS: &[&str] = &[
  "load-only",
  "read-only-existing",
  "workload",
  "rows",
  "batch-size",
  "write-batchs",
  "kv-size",
  "key-size",
  "buffer-pool-size",
  "read-per-part",
  "read-count",
  "scan-count",
  "trials",
  "test-secs",
  "scan-bytes",
  "store-dir",
  "history",
  "rng",
  "del-ratio",
  "partitions",
];

/// Process adapter for the Rust `fkv_bench` binary.
#[derive(Debug)]
pub struct FkvEngine {
  bin: PathBuf,
  last_store: Mutex<Option<PathBuf>>,
  verified: OnceLock<Result<(), String>>,
}

impl FkvEngine {
  #[must_use]
  pub fn new(bin: impl Into<PathBuf>) -> Self {
    Self {
      bin: bin.into(),
      last_store: Mutex::new(None),
      verified: OnceLock::new(),
    }
  }

  fn ensure_verified(&self) -> Result<(), String> {
    self
      .verified
      .get_or_init(|| verify_binary(&self.bin, FKV_REQUIRED_FLAGS))
      .clone()
  }

  fn store_dir(&self, cfg: &MatrixConfig, phase: Phase) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("stores")
      .join(phase.slug())
  }

  fn log_path(&self, cfg: &MatrixConfig, phase: Phase, label: &str) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("logs")
      .join(phase.slug())
      .join(format!("{label}.log"))
  }

  fn history_path(&self, cfg: &MatrixConfig, phase: Phase, label: &str) -> PathBuf {
    cfg
      .engine_dir(self.name())
      .join("history")
      .join(phase.slug())
      .join(format!("{label}.json"))
  }

  fn exec(
    &self,
    cfg: &MatrixConfig,
    phase: Phase,
    label: &str,
    args: &[String],
  ) -> Result<String, String> {
    execute(
      &self.bin,
      args,
      &self.log_path(cfg, phase, label),
      phase_timeout(cfg),
    )
  }

  fn load_metrics(&self, history: &Path) -> Result<BTreeMap<String, f64>, String> {
    let text = read_required_file(history)?;
    let parsed = fkv_perf_cross::parse_rust_history(&text)
      .map_err(|err| format!("parse {}: {err}", history.display()))?;
    let metrics: BTreeMap<String, f64> = parsed
      .metrics
      .iter()
      .map(|(k, v)| (k.to_owned(), v))
      .collect();
    if metrics.is_empty() {
      return Err(format!("{} contains no rust metrics", history.display()));
    }
    Ok(metrics)
  }

  fn run_phase(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String> {
    let store_dir = self.store_dir(cfg, phase);
    reset_dir(&store_dir)?;
    let history = self.history_path(cfg, phase, "run");
    remove_file_if_exists(&history)?;

    let result = match phase {
      Phase::BulkLoad => {
        let args = fkv_load_args(cfg, &store_dir, &history);
        let _ = self.exec(cfg, phase, "load", &args)?;
        let metrics = self.load_metrics(&history)?;
        result_from_total(&metrics, "put_kv_s", "put_p99_us", cfg.bulk_rows as u64)
      }
      Phase::IndividualWrites => {
        let args = fkv_write_args(cfg, &store_dir, &history, 1, cfg.individual_writes as usize);
        let _ = self.exec(cfg, phase, "write", &args)?;
        let metrics = self.load_metrics(&history)?;
        result_from_total(&metrics, "put_kv_s", "put_p99_us", cfg.individual_writes)
      }
      Phase::BatchWrites => {
        let batches = cfg.batch_count();
        let ops = batches as u64 * cfg.batch_size as u64;
        let args = fkv_write_args(cfg, &store_dir, &history, cfg.batch_size, batches);
        let _ = self.exec(cfg, phase, "write", &args)?;
        let metrics = self.load_metrics(&history)?;
        result_from_total(&metrics, "put_kv_s", "put_p99_us", ops)
      }
      Phase::RandomReads { readers } => {
        let load_history = self.history_path(cfg, phase, "load");
        remove_file_if_exists(&load_history)?;
        let load = fkv_prepare_args(cfg, &store_dir, &load_history);
        self.exec(cfg, phase, "load", &load)?;
        let read = fkv_read_args(cfg, &store_dir, &history, readers);
        self.exec(cfg, phase, "read", &read)?;
        let metrics = self.load_metrics(&history)?;
        if cfg.reads > 0 {
          result_from_total(&metrics, "get_kv_s", "get_p99_us", cfg.reads)
        } else {
          result_from_window(&metrics, "get_kv_s", "get_p99_us", cfg.read_secs)
        }
      }
      Phase::RangeReads => {
        let load_history = self.history_path(cfg, phase, "load");
        remove_file_if_exists(&load_history)?;
        let load = fkv_prepare_args(cfg, &store_dir, &load_history);
        self.exec(cfg, phase, "load", &load)?;
        let scan = fkv_scan_args(cfg, &store_dir, &history);
        self.exec(cfg, phase, "scan", &scan)?;
        let metrics = self.load_metrics(&history)?;
        if cfg.scans > 0 {
          // One page request per range scan: `scan_page_s` is requests/s, the
          // same physical unit the C++ shim reports.
          result_from_total(&metrics, "scan_page_s", "scan_p99_us", cfg.scans)
        } else {
          result_from_window(&metrics, "scan_kv_s", "scan_p99_us", cfg.read_secs)
        }
      }
      Phase::Removals => {
        let args = fkv_delete_args(cfg, &store_dir, &history);
        self.exec(cfg, phase, "delete", &args)?;
        let metrics = self.load_metrics(&history)?;
        result_from_total(&metrics, "delete_kv_s", "delete_p99_us", cfg.removals)
      }
      Phase::Len => {
        // `len()` is a full-scan count: load once, then run one scan round.
        let load_history = self.history_path(cfg, phase, "load");
        remove_file_if_exists(&load_history)?;
        let load = fkv_prepare_args(cfg, &store_dir, &load_history);
        self.exec(cfg, phase, "load", &load)?;
        let len = fkv_len_args(cfg, &store_dir, &history);
        self.exec(cfg, phase, "len", &len)?;
        let metrics = self.load_metrics(&history)?;
        result_from_fullscan(&metrics)
      }
    };
    set_last_store(&self.last_store, &store_dir)?;
    result
  }
}

impl BenchEngine for FkvEngine {
  fn name(&self) -> &'static str {
    "fkv"
  }

  fn run(&self, phase: Phase, cfg: &MatrixConfig) -> Result<PhaseResult, String> {
    self.ensure_verified()?;
    self.run_phase(phase, cfg)
  }

  fn store_bytes(&self, _cfg: &MatrixConfig) -> Result<u64, String> {
    let dir = last_store(&self.last_store)?;
    if !dir.exists() {
      return Err(format!("store directory {} does not exist", dir.display()));
    }
    dir_size_bytes(&dir).map_err(|err| io_err(&format!("size {}", dir.display()), err))
  }

  fn compact(&self, cfg: &MatrixConfig) -> Result<Option<Duration>, String> {
    let dir = last_store(&self.last_store)?;
    if !dir.exists() {
      return Err(format!("store directory {} does not exist", dir.display()));
    }
    self.ensure_verified()?;
    let phase = Phase::BulkLoad;
    let history = self.history_path(cfg, phase, "compact");
    remove_file_if_exists(&history)?;
    let args = fkv_compact_args(cfg, &dir, &history);
    self.exec(cfg, phase, "compact", &args)?;
    let metrics = self.load_metrics(&history)?;
    let micros = metric(&metrics, "compact_total_us").ok_or_else(|| {
      format!(
        "compact-only history missing compact_total_us; metrics: {}",
        metrics.keys().cloned().collect::<Vec<_>>().join(", ")
      )
    })?;
    if micros < 0.0 {
      return Err(format!("compact_total_us is negative: {micros}"));
    }
    Ok(Some(Duration::from_secs_f64(micros / 1_000_000.0)))
  }

  fn supports(&self, _phase: Phase) -> bool {
    true
  }
}

fn fkv_base_args(cfg: &MatrixConfig, rows: usize, store_dir: &Path, history: &Path) -> Vec<String> {
  fkv_base_args_with_test_secs(cfg, rows, store_dir, history, cfg.read_secs)
}

fn fkv_base_args_with_test_secs(
  cfg: &MatrixConfig,
  rows: usize,
  store_dir: &Path,
  history: &Path,
  test_secs: f64,
) -> Vec<String> {
  let mut args = vec![
    "--impl".to_owned(),
    "rust".to_owned(),
    "--rows".to_owned(),
    rows.to_string(),
    "--max-key".to_owned(),
    rows.to_string(),
    "--partitions".to_owned(),
    "1".to_owned(),
    "--kv-size".to_owned(),
    cfg.kv_size.to_string(),
    "--key-size".to_owned(),
    cfg.key_size.to_string(),
    "--key-interval".to_owned(),
    "1".to_owned(),
    "--rng".to_owned(),
    "grid".to_owned(),
    "--del-ratio".to_owned(),
    "0".to_owned(),
    "--scan-bytes".to_owned(),
    cfg.scan_bytes.to_string(),
    "--test-secs".to_owned(),
    fmt_secs(test_secs),
    "--store-dir".to_owned(),
    store_dir.display().to_string(),
    "--history".to_owned(),
    history.display().to_string(),
  ];
  if cfg.cache_bytes > 0 {
    args.push("--buffer-pool-size".to_owned());
    args.push(cfg.cache_bytes.to_string());
  }
  args
}

pub fn fkv_load_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let mut args = fkv_base_args(cfg, cfg.bulk_rows, store_dir, history);
  args.extend([
    "--batch-size".to_owned(),
    cfg.batch_size.to_string(),
    "--load-only".to_owned(),
    "--workload".to_owned(),
    "write-only".to_owned(),
  ]);
  args
}

/// Untimed dataset load used to prepare read / scan / len phases.
/// Uses `--rows` (the 1M redb dataset), not `--bulk-rows`.
pub fn fkv_prepare_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let mut args = fkv_base_args(cfg, cfg.rows, store_dir, history);
  args.extend([
    "--batch-size".to_owned(),
    cfg.batch_size.to_string(),
    "--load-only".to_owned(),
    "--workload".to_owned(),
    "write-only".to_owned(),
  ]);
  args
}

pub fn fkv_write_args(
  cfg: &MatrixConfig,
  store_dir: &Path,
  history: &Path,
  batch_size: usize,
  batches: usize,
) -> Vec<String> {
  let mut args = fkv_base_args(cfg, cfg.rows, store_dir, history);
  args.extend([
    "--batch-size".to_owned(),
    batch_size.to_string(),
    "--write-batchs".to_owned(),
    batches.to_string(),
    "--workload".to_owned(),
    "write-only".to_owned(),
  ]);
  args
}

pub fn fkv_read_args(
  cfg: &MatrixConfig,
  store_dir: &Path,
  history: &Path,
  readers: usize,
) -> Vec<String> {
  // Fixed-count mode has no meaningful time window; `--read-count` alone
  // drives the lane.  A zero read count keeps the legacy time window.
  let test_secs = if cfg.reads > 0 { 0.0 } else { cfg.read_secs };
  let mut args = fkv_base_args_with_test_secs(cfg, cfg.rows, store_dir, history, test_secs);
  args.extend([
    "--read-per-part".to_owned(),
    readers.to_string(),
    "--read-count".to_owned(),
    cfg.reads.to_string(),
    "--trials".to_owned(),
    cfg.samples.max(1).to_string(),
    "--read-only-existing".to_owned(),
    "--workload".to_owned(),
    "read-only".to_owned(),
  ]);
  args
}

pub fn fkv_scan_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let test_secs = if cfg.scans > 0 { 0.0 } else { cfg.read_secs };
  let mut args = fkv_base_args_with_test_secs(cfg, cfg.rows, store_dir, history, test_secs);
  args.extend([
    "--read-per-part".to_owned(),
    "1".to_owned(),
    "--scan-count".to_owned(),
    cfg.scans.to_string(),
    "--trials".to_owned(),
    cfg.samples.max(1).to_string(),
    "--read-only-existing".to_owned(),
    "--workload".to_owned(),
    "scan".to_owned(),
  ]);
  args
}

/// Args for the `len` phase.
///
/// `fkv_bench` has no O(1) length API, so `len()` is a full-scan count:
/// `--test-secs 0` asks for exactly one full-scan round, whose history reports
/// `fullscan_num` (live entries) and `fullscan_total_us` (round wall time).
/// Everything else mirrors [`fkv_scan_args`]; only the time window differs.
pub fn fkv_len_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let mut args = fkv_base_args_with_test_secs(cfg, cfg.rows, store_dir, history, 0.0);
  args.extend([
    "--read-per-part".to_owned(),
    "1".to_owned(),
    "--trials".to_owned(),
    cfg.samples.max(1).to_string(),
    "--read-only-existing".to_owned(),
    "--workload".to_owned(),
    "scan".to_owned(),
  ]);
  args
}

pub fn fkv_delete_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let mut args = fkv_base_args(cfg, cfg.rows, store_dir, history);
  let batches = cfg.removals.div_ceil(cfg.batch_size.max(1) as u64);
  args.extend([
    "--batch-size".to_owned(),
    cfg.batch_size.to_string(),
    "--write-batchs".to_owned(),
    batches.to_string(),
    "--workload".to_owned(),
    "delete-only".to_owned(),
  ]);
  args
}

pub fn fkv_compact_args(cfg: &MatrixConfig, store_dir: &Path, history: &Path) -> Vec<String> {
  let mut args = fkv_base_args(cfg, cfg.bulk_rows, store_dir, history);
  args.extend([
    "--batch-size".to_owned(),
    cfg.batch_size.to_string(),
    "--read-only-existing".to_owned(),
    "--workload".to_owned(),
    "compact-only".to_owned(),
  ]);
  args
}

fn fmt_secs(secs: f64) -> String {
  if secs.fract() == 0.0 {
    format!("{}", secs as u64)
  } else {
    format!("{secs}")
  }
}

// ---------------------------------------------------------------------------
// Matrix orchestration and report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct PhaseRow {
  phase: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  cpp: Option<PhaseResult>,
  #[serde(skip_serializing_if = "Option::is_none")]
  fkv: Option<PhaseResult>,
  #[serde(skip_serializing_if = "Option::is_none")]
  ratio: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  cpp_note: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  fkv_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SizeRow {
  cpp: Option<u64>,
  fkv: Option<u64>,
  ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct CompactionTiming {
  cpp_secs: Option<f64>,
  fkv_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct ConfigSummary {
  workdir: String,
  profile: String,
  rows: usize,
  bulk_rows: usize,
  batch_size: usize,
  kv_size: usize,
  key_size: usize,
  value_size: usize,
  cache_bytes: u64,
  read_secs: f64,
  readers: Vec<usize>,
  scan_bytes: u64,
  individual_writes: u64,
  batch_writes: u64,
  reads: u64,
  scans: u64,
  scan_len: usize,
  removals: u64,
  samples: usize,
}

#[derive(Debug, Clone, Serialize)]
struct MatrixReport {
  config: ConfigSummary,
  phases: Vec<PhaseRow>,
  uncompacted_size: SizeRow,
  compacted_size: SizeRow,
  compaction: CompactionTiming,
}

fn build_phases(readers: &[usize]) -> Vec<Phase> {
  let mut phases = vec![Phase::BulkLoad, Phase::IndividualWrites, Phase::BatchWrites];
  phases.extend(
    readers
      .iter()
      .map(|readers| Phase::RandomReads { readers: *readers }),
  );
  phases.push(Phase::RangeReads);
  phases.push(Phase::Removals);
  phases.push(Phase::Len);
  phases
}

fn run_optional(
  engine: &dyn BenchEngine,
  phase: Phase,
  cfg: &MatrixConfig,
) -> Result<Option<PhaseResult>, String> {
  if !engine.supports(phase) {
    return Ok(None);
  }
  engine
    .run(phase, cfg)
    .map(Some)
    .map_err(|err| format!("{} {} failed: {err}", engine.name(), phase.label()))
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
  match (numerator, denominator) {
    (Some(num), Some(den)) if den > 0.0 && num.is_finite() => Some(num / den),
    _ => None,
  }
}

fn measure_compaction(
  engine: &dyn BenchEngine,
  cfg: &MatrixConfig,
) -> Result<(u64, u64, Option<Duration>), String> {
  // Exactly the requested C++/Rust shape: load, size, compact, size.
  engine
    .run(Phase::BulkLoad, cfg)
    .map_err(|err| format!("{} compact-load failed: {err}", engine.name()))?;
  let before = engine
    .store_bytes(cfg)
    .map_err(|err| format!("{} size before compact failed: {err}", engine.name()))?;
  let duration = engine
    .compact(cfg)
    .map_err(|err| format!("{} compact failed: {err}", engine.name()))?;
  let after = engine
    .store_bytes(cfg)
    .map_err(|err| format!("{} size after compact failed: {err}", engine.name()))?;
  Ok((before, after, duration))
}

fn run_matrix(
  cpp: &dyn BenchEngine,
  fkv: &dyn BenchEngine,
  cfg: &MatrixConfig,
) -> Result<MatrixReport, String> {
  let mut phases = Vec::new();
  for phase in build_phases(&cfg.readers) {
    eprintln!("[matrix] {} ...", phase.label());
    let cpp_result = run_optional(cpp, phase, cfg)?;
    let fkv_result = run_optional(fkv, phase, cfg)?;
    let cpp_note = cpp_result.is_none().then(|| "N/A".to_owned());
    let fkv_note = fkv_result.is_none().then(|| "N/A".to_owned());
    phases.push(PhaseRow {
      phase: phase.label(),
      ratio: ratio(
        fkv_result.as_ref().map(|result| result.throughput),
        cpp_result.as_ref().map(|result| result.throughput),
      ),
      cpp: cpp_result,
      fkv: fkv_result,
      cpp_note,
      fkv_note,
    });
  }

  eprintln!("[matrix] compaction sizes ...");
  let (cpp_before, cpp_after, cpp_compact) = measure_compaction(cpp, cfg)?;
  let (fkv_before, fkv_after, fkv_compact) = measure_compaction(fkv, cfg)?;

  Ok(MatrixReport {
    config: ConfigSummary {
      workdir: cfg.workdir.display().to_string(),
      profile: cfg.profile.as_str().to_owned(),
      rows: cfg.rows,
      bulk_rows: cfg.bulk_rows,
      batch_size: cfg.batch_size,
      kv_size: cfg.kv_size,
      key_size: cfg.key_size,
      value_size: cfg.value_size(),
      cache_bytes: cfg.cache_bytes,
      read_secs: cfg.read_secs,
      readers: cfg.readers.clone(),
      scan_bytes: cfg.scan_bytes,
      individual_writes: cfg.individual_writes,
      batch_writes: cfg.batch_writes,
      reads: cfg.reads,
      scans: cfg.scans,
      scan_len: cfg.scan_len,
      removals: cfg.removals,
      samples: cfg.samples,
    },
    phases,
    uncompacted_size: SizeRow {
      cpp: Some(cpp_before),
      fkv: Some(fkv_before),
      ratio: ratio(Some(fkv_before as f64), Some(cpp_before as f64)),
    },
    compacted_size: SizeRow {
      cpp: Some(cpp_after),
      fkv: Some(fkv_after),
      ratio: ratio(Some(fkv_after as f64), Some(cpp_after as f64)),
    },
    compaction: CompactionTiming {
      cpp_secs: cpp_compact.map(|duration| duration.as_secs_f64()),
      fkv_secs: fkv_compact.map(|duration| duration.as_secs_f64()),
    },
  })
}

fn fmt_throughput(value: f64) -> String {
  if value >= 1e9 {
    format!("{:.2} Gops/s", value / 1e9)
  } else if value >= 1e6 {
    format!("{:.2} Mops/s", value / 1e6)
  } else if value >= 1e3 {
    format!("{:.2} kops/s", value / 1e3)
  } else {
    format!("{value:.1} ops/s")
  }
}

fn fmt_phase_cell(result: Option<&PhaseResult>, note: Option<&str>) -> String {
  let Some(result) = result else {
    return note.unwrap_or("N/A").to_owned();
  };
  match result.p99_us {
    Some(p99) => format!("{}, p99 {p99:.1} µs", fmt_throughput(result.throughput)),
    None => fmt_throughput(result.throughput),
  }
}

fn fmt_ratio(value: Option<f64>) -> String {
  value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.3}x"))
}

fn fmt_bytes(value: u64) -> String {
  const MIB: f64 = 1024.0 * 1024.0;
  const GIB: f64 = MIB * 1024.0;
  let value_f = value as f64;
  if value_f >= GIB {
    format!("{:.2} GiB", value_f / GIB)
  } else if value_f >= MIB {
    format!("{:.2} MiB", value_f / MIB)
  } else if value_f >= 1024.0 {
    format!("{:.2} KiB", value_f / 1024.0)
  } else {
    format!("{value} B")
  }
}

fn size_cell(value: Option<u64>) -> String {
  value.map_or_else(|| "N/A".to_owned(), fmt_bytes)
}

impl MatrixReport {
  fn to_markdown(&self) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# fkv benchmark matrix\n");
    let _ = writeln!(out, "- workdir: `{}`", self.config.workdir);
    let _ = writeln!(
      out,
      "- profile={} rows={} bulk_rows={} batch_size={} kv_size={} key_size={} value_size={} cache_bytes={} readers={:?}",
      self.config.profile,
      self.config.rows,
      self.config.bulk_rows,
      self.config.batch_size,
      self.config.kv_size,
      self.config.key_size,
      self.config.value_size,
      self.config.cache_bytes,
      self.config.readers
    );
    let _ = writeln!(
      out,
      "- individual_writes={} batch_writes={}x{} reads={} scans={}x{} removals={} samples={} read_secs={} scan_bytes={}",
      self.config.individual_writes,
      self.config.batch_writes,
      self.config.batch_size,
      self.config.reads,
      self.config.scans,
      self.config.scan_len,
      self.config.removals,
      self.config.samples,
      self.config.read_secs,
      self.config.scan_bytes
    );
    out.push('\n');
    out.push_str("| phase | cpp | fkv | ratio |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for row in &self.phases {
      let _ = writeln!(
        out,
        "| {} | {} | {} | {} |",
        row.phase,
        fmt_phase_cell(row.cpp.as_ref(), row.cpp_note.as_deref()),
        fmt_phase_cell(row.fkv.as_ref(), row.fkv_note.as_deref()),
        fmt_ratio(row.ratio)
      );
    }
    let _ = writeln!(
      out,
      "| uncompacted size | {} | {} | {} |",
      size_cell(self.uncompacted_size.cpp),
      size_cell(self.uncompacted_size.fkv),
      fmt_ratio(self.uncompacted_size.ratio)
    );
    let _ = writeln!(
      out,
      "| compacted size | {} | {} | {} |",
      size_cell(self.compacted_size.cpp),
      size_cell(self.compacted_size.fkv),
      fmt_ratio(self.compacted_size.ratio)
    );
    let cpp_compact = self
      .compaction
      .cpp_secs
      .map_or_else(|| "N/A".to_owned(), |secs| format!("{secs:.3} s"));
    let fkv_compact = self
      .compaction
      .fkv_secs
      .map_or_else(|| "N/A".to_owned(), |secs| format!("{secs:.3} s"));
    let compact_ratio = match (self.compaction.fkv_secs, self.compaction.cpp_secs) {
      (Some(fkv), Some(cpp)) if cpp > 0.0 => Some(fkv / cpp),
      _ => None,
    };
    let _ = writeln!(
      out,
      "| compact wall time | {cpp_compact} | {fkv_compact} | {} |",
      fmt_ratio(compact_ratio)
    );
    out.push('\n');
    out.push_str(
      "> throughput is logical ops/s; ratio = fkv / cpp (>1 means fkv faster).\n\
       > size ratio = fkv bytes / cpp bytes. `N/A` means the engine has no API for the phase.\n\
       > RandomReads rows use `--readers N` on both engines (partitions=1).\n",
    );
    if self.phases.iter().any(|row| row.phase == "len()") {
      out.push_str(
        "> `len()` is a full-scan key count, not an O(1) length lookup: the FFI C++\n\
         > engine has no O(1) len API and counts keys by scanning, and fkv runs one\n\
         > full-scan round (`--workload scan --test-secs 0`); the reported ops/s is\n\
         > the scan rate, not an O(1) lookup rate, on both engines.\n",
      );
    }
    if let Some(cpp) = self.compaction.cpp_secs {
      let _ = writeln!(out, "> C++ compact: {cpp:.3} s");
    }
    if let Some(fkv) = self.compaction.fkv_secs {
      let _ = writeln!(out, "> fkv compact-only: {fkv:.3} s");
    }
    out.push('\n');
    out.push_str("## 口径\n\n");
    let _ = writeln!(
      out,
      "- **redb 工作负载**：bulk load {bulk} 条；读 / scan / len 数据集 {rows} 条；\
       individual writes {individual} 条；batch writes {batch_writes} × {batch_size}；\
       random reads {reads} 次；range scans {scans} 次 × {scan_len} 条；removals {removals} 条；\
       读 / scan / len 各重复 {samples} 次取中位数。",
      bulk = self.config.bulk_rows,
      rows = self.config.rows,
      individual = self.config.individual_writes,
      batch_writes = self.config.batch_writes,
      batch_size = self.config.batch_size,
      reads = self.config.reads,
      scans = self.config.scans,
      scan_len = self.config.scan_len,
      removals = self.config.removals,
      samples = self.config.samples,
    );
    let _ = writeln!(
      out,
      "- **KV 形状**：key {key} B，value {value} B（kv_size {kv}）；两侧 key 都是\
       「末尾 8B 大端下标 + 前导零」的确定性编码，可复现且字典序与下标序一致。\
       这不是 redb 的随机 24B key（redb 用 RNG_SEED=3 生成随机 key），B-tree 分裂形状会不同。",
      key = self.config.key_size,
      value = self.config.value_size,
      kv = self.config.kv_size,
    );
    let data_mib = (self.config.rows * self.config.kv_size) as f64 / (1024.0 * 1024.0);
    let cache_mib = self.config.cache_bytes as f64 / (1024.0 * 1024.0);
    // redb 的 CACHE_SIZE=4 GiB 大于其 870 MB 数据集，即读阶段在“全驻内存”下测量；
    // 缓存口径必须跟着一起变，否则两侧都在慢盘上抖随机读，测到的是盘而不是引擎。
    let residency = if cache_mib >= data_mib {
      "**不低于**数据集，读阶段全驻内存，与 redb（4 GiB 缓存 / 870 MB 数据）同口径"
    } else {
      "**低于**数据集，读阶段随机读落盘（冷缓存口径，非 redb 发布条件）"
    };
    let _ = writeln!(
      out,
      "- **缓存预算**：两侧都设为 {cache} B（{cache_mib:.1} MiB，受控变量，{residency}）。\
       数据集为 {rows} × {kv} B ≈ {data_mib:.1} MiB。\
       C++ `buffer_pool_size` 是**每 shard 上限**、由 index + data page 共享（`enable_data_page_cache=true`）；\
       本矩阵只有 partition 0，只有 shard 0 承载数据并真正使用这份缓存（其余空 shard 不产生页缓存），\
       故工作集可用上限即 {cache_mib:.1} MiB。fkv 是单 store 总预算，对半拆成 index cache / page cache 各 {half_mib:.1} MiB。",
      cache = self.config.cache_bytes,
      cache_mib = cache_mib,
      residency = residency,
      rows = self.config.rows,
      kv = self.config.kv_size,
      data_mib = data_mib,
      half_mib = cache_mib / 2.0,
    );
    out.push_str(
      "- **C++ 并发口径**：`num_threads` 是 EloqStore 的 shard / worker 线程数，\
       本矩阵只有 partition 0（`partition_id % shard_num` 恒路由到 shard 0），因此两侧固定\
       `num_threads=1`；点读的并发由客户端读线程数 `--readers` 提供（C++ 侧 N 个线程向同一\
       shard 并发提交，fkv 侧 N 条独立异步读链），range scan / len 固定 1。\
       曾把 `num_threads` 设成 readers，结果单分区起 32 个 shard、io_uring 初始化 ENOMEM，\
       已回退；这也说明此前 C++ 单分区读吞吐 ~19.5 kops/s 是该配置下的真实上限，不是框架偏差。\n",
    );
    out.push_str(
      "- **len() 语义**：redb 的 `len()` 是 O(1) 元数据读取；本表两侧都没有 O(1) 计数 API，\
       C++ FFI 与 fkv 都做一次全量分页/全表扫描后计数，表中 len() 行是扫描速率，不是 O(1) 查找速率，\
       不能与 redb README 的 len() 数值直接比较。\n",
    );
    if self.config.profile == "legacy" {
      out.push_str(
        "- **legacy 档**：保留旧的 200k 数据集与时间窗读 / scan（`read_secs`），\
         不使用固定次数；C++ FFI 仍按其固定次数执行，故该档读 / scan 与 redb 档不可混比。\n",
      );
    }
    out
  }

  fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(self)?;
    json.push('\n');
    Ok(json)
  }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Args {
  cpp_lib: Option<PathBuf>,
  cpp_bin: Option<PathBuf>,
  rust_bin: Option<PathBuf>,
  workdir: Option<PathBuf>,
  profile: Option<String>,
  rows: Option<usize>,
  bulk_rows: Option<usize>,
  batch_size: Option<usize>,
  kv_size: Option<usize>,
  key_size: Option<usize>,
  cache_bytes: Option<u64>,
  read_secs: Option<f64>,
  readers: Option<Vec<usize>>,
  scan_bytes: Option<u64>,
  individual_writes: Option<u64>,
  batch_writes: Option<u64>,
  reads: Option<u64>,
  scans: Option<u64>,
  scan_len: Option<usize>,
  removals: Option<u64>,
  samples: Option<usize>,
  phase_timeout_secs: Option<u64>,
  markdown: Option<PathBuf>,
  json: Option<PathBuf>,
}

/// Which C++ backend `main` should construct.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CppBackend {
  /// In-process FFI shim (`--cpp-lib`), preferred.
  Ffi(PathBuf),
  /// Legacy `simple_bench` process adapter (`--cpp-bin`).
  Bin(PathBuf),
}

impl Args {
  /// `--cpp-lib` wins over the legacy `--cpp-bin` fallback.
  fn cpp_backend(&self) -> Result<CppBackend, String> {
    if let Some(lib) = &self.cpp_lib {
      return Ok(CppBackend::Ffi(lib.clone()));
    }
    if let Some(bin) = &self.cpp_bin {
      return Ok(CppBackend::Bin(bin.clone()));
    }
    Err(
      "缺少 C++ 引擎: 请提供 --cpp-lib <libfkv_bench_ffi.so>（推荐），\
       或使用 legacy --cpp-bin <simple_bench>"
        .to_owned(),
    )
  }
}

enum ParsedArgs {
  Run(Args),
  Help,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<ParsedArgs, String> {
  let mut parsed = Args::default();
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "-h" | "--help" => return Ok(ParsedArgs::Help),
      "--cpp-lib"
      | "--cpp-bin"
      | "--rust-bin"
      | "--workdir"
      | "--profile"
      | "--rows"
      | "--bulk-rows"
      | "--batch-size"
      | "--kv-size"
      | "--key-size"
      | "--cache-bytes"
      | "--read-secs"
      | "--readers"
      | "--scan-bytes"
      | "--individual-writes"
      | "--batch-writes"
      | "--reads"
      | "--scans"
      | "--scan-len"
      | "--removals"
      | "--samples"
      | "--phase-timeout-secs"
      | "--markdown"
      | "--json" => {
        let value = args.next().ok_or_else(|| format!("{arg} 缺少参数值"))?;
        match arg.as_str() {
          "--cpp-lib" => {
            ensure_unset(&parsed.cpp_lib, &arg)?;
            parsed.cpp_lib = Some(PathBuf::from(value));
          }
          "--cpp-bin" => {
            ensure_unset(&parsed.cpp_bin, &arg)?;
            parsed.cpp_bin = Some(PathBuf::from(value));
          }
          "--rust-bin" => {
            ensure_unset(&parsed.rust_bin, &arg)?;
            parsed.rust_bin = Some(PathBuf::from(value));
          }
          "--workdir" => {
            ensure_unset(&parsed.workdir, &arg)?;
            parsed.workdir = Some(PathBuf::from(value));
          }
          "--profile" => {
            ensure_unset(&parsed.profile, &arg)?;
            parsed.profile = Some(value);
          }
          "--rows" => {
            ensure_unset(&parsed.rows, &arg)?;
            parsed.rows = Some(parse_usize(&arg, &value)?);
          }
          "--bulk-rows" => {
            ensure_unset(&parsed.bulk_rows, &arg)?;
            parsed.bulk_rows = Some(parse_usize(&arg, &value)?);
          }
          "--batch-size" => {
            ensure_unset(&parsed.batch_size, &arg)?;
            parsed.batch_size = Some(parse_usize(&arg, &value)?);
          }
          "--kv-size" => {
            ensure_unset(&parsed.kv_size, &arg)?;
            parsed.kv_size = Some(parse_usize(&arg, &value)?);
          }
          "--key-size" => {
            ensure_unset(&parsed.key_size, &arg)?;
            parsed.key_size = Some(parse_usize(&arg, &value)?);
          }
          "--cache-bytes" => {
            ensure_unset(&parsed.cache_bytes, &arg)?;
            parsed.cache_bytes = Some(parse_u64(&arg, &value)?);
          }
          "--read-secs" => {
            ensure_unset(&parsed.read_secs, &arg)?;
            parsed.read_secs = Some(
              value
                .parse::<f64>()
                .map_err(|_| format!("{arg} 不是数字: {value}"))?,
            );
          }
          "--readers" => {
            ensure_unset(&parsed.readers, &arg)?;
            parsed.readers = Some(parse_readers(&value)?);
          }
          "--scan-bytes" => {
            ensure_unset(&parsed.scan_bytes, &arg)?;
            parsed.scan_bytes = Some(parse_u64(&arg, &value)?);
          }
          "--individual-writes" => {
            ensure_unset(&parsed.individual_writes, &arg)?;
            parsed.individual_writes = Some(parse_u64(&arg, &value)?);
          }
          "--batch-writes" => {
            ensure_unset(&parsed.batch_writes, &arg)?;
            parsed.batch_writes = Some(parse_u64(&arg, &value)?);
          }
          "--reads" => {
            ensure_unset(&parsed.reads, &arg)?;
            parsed.reads = Some(parse_u64(&arg, &value)?);
          }
          "--scans" => {
            ensure_unset(&parsed.scans, &arg)?;
            parsed.scans = Some(parse_u64(&arg, &value)?);
          }
          "--scan-len" => {
            ensure_unset(&parsed.scan_len, &arg)?;
            parsed.scan_len = Some(parse_usize(&arg, &value)?);
          }
          "--removals" => {
            ensure_unset(&parsed.removals, &arg)?;
            parsed.removals = Some(parse_u64(&arg, &value)?);
          }
          "--samples" => {
            ensure_unset(&parsed.samples, &arg)?;
            parsed.samples = Some(parse_usize(&arg, &value)?);
          }
          "--phase-timeout-secs" => {
            ensure_unset(&parsed.phase_timeout_secs, &arg)?;
            parsed.phase_timeout_secs = Some(parse_u64(&arg, &value)?);
          }
          "--markdown" => {
            ensure_unset(&parsed.markdown, &arg)?;
            parsed.markdown = Some(PathBuf::from(value));
          }
          "--json" => {
            ensure_unset(&parsed.json, &arg)?;
            parsed.json = Some(PathBuf::from(value));
          }
          _ => unreachable!("matched above"),
        }
      }
      other => return Err(format!("未知参数: {other}")),
    }
  }
  Ok(ParsedArgs::Run(parsed))
}

fn ensure_unset<T>(slot: &Option<T>, arg: &str) -> Result<(), String> {
  if slot.is_some() {
    Err(format!("{arg} 重复出现"))
  } else {
    Ok(())
  }
}

fn parse_usize(arg: &str, value: &str) -> Result<usize, String> {
  value
    .parse::<usize>()
    .map_err(|_| format!("{arg} 不是整数: {value}"))
}

fn parse_u64(arg: &str, value: &str) -> Result<u64, String> {
  value
    .parse::<u64>()
    .map_err(|_| format!("{arg} 不是整数: {value}"))
}

fn parse_readers(value: &str) -> Result<Vec<usize>, String> {
  let readers = value
    .split(',')
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .map(|part| {
      part
        .parse::<usize>()
        .map_err(|_| format!("--readers 中的 {part:?} 不是整数"))
    })
    .collect::<Result<Vec<_>, _>>()?;
  if readers.is_empty() {
    return Err("--readers 不能为空".to_owned());
  }
  if readers.contains(&0) {
    return Err("--readers 必须为正整数".to_owned());
  }
  Ok(readers)
}

fn resolve_config(args: Args) -> Result<(CppBackend, PathBuf, MatrixConfig), String> {
  const MIB: u64 = 1024 * 1024;
  let cpp_backend = args.cpp_backend()?;
  let rust_bin = args.rust_bin.ok_or_else(|| "缺少 --rust-bin".to_owned())?;
  let profile = match args.profile.as_deref() {
    Some(value) => Profile::parse(value)?,
    None => Profile::Redb,
  };
  let legacy = profile == Profile::Legacy;
  let rows = args
    .rows
    .unwrap_or(if legacy { 200_000 } else { 1_000_000 });
  let batch_size = args.batch_size.unwrap_or(1000);
  let kv_size = args.kv_size.unwrap_or(if legacy { 128 } else { 174 });
  let key_size = args.key_size.unwrap_or(if legacy { 8 } else { 24 });
  let cache_bytes = args
    .cache_bytes
    .unwrap_or(if legacy { 0 } else { 64 * MIB });
  let read_secs = args.read_secs.unwrap_or(if legacy { 5.0 } else { 0.0 });
  let readers = args.readers.unwrap_or_else(|| vec![1, 4, 8, 16, 32]);
  // 单阶段超过 30 分钟几乎必然是卡死（例如读线程与单 shard 争用崩溃），
  // 直接杀进程并报错，避免 CI 里静默挂满 job 上限。
  let phase_timeout_secs = args.phase_timeout_secs.unwrap_or(1800);
  let workdir = args.workdir.unwrap_or_else(|| PathBuf::from("/tmp/matrix"));
  let bulk_rows = args
    .bulk_rows
    .unwrap_or(if legacy { rows } else { 5_000_000 });
  let individual_writes =
    args
      .individual_writes
      .unwrap_or(if legacy { rows as u64 } else { 1_000 });
  let batch_writes = args.batch_writes.unwrap_or(if legacy {
    rows.div_ceil(batch_size) as u64
  } else {
    100
  });
  let reads = args.reads.unwrap_or(if legacy { 0 } else { 1_000_000 });
  let scans = args.scans.unwrap_or(if legacy { 0 } else { 500_000 });
  let scan_len = args.scan_len.unwrap_or(if legacy { 0 } else { 10 });
  let removals = args
    .removals
    .unwrap_or(if legacy { rows as u64 } else { 500_000 });
  let samples = args.samples.unwrap_or(if legacy { 1 } else { 3 });
  let value_size = kv_size.saturating_sub(key_size);
  let scan_bytes = args.scan_bytes.unwrap_or(if legacy {
    0
  } else {
    (scan_len * value_size) as u64
  });

  if rows == 0 {
    return Err("--rows 必须大于 0".to_owned());
  }
  if bulk_rows == 0 {
    return Err("--bulk-rows 必须大于 0".to_owned());
  }
  if batch_size == 0 {
    return Err("--batch-size 必须大于 0".to_owned());
  }
  if key_size < 8 {
    return Err("--key-size 必须 >= 8（key 末尾是 8B 大端下标记）".to_owned());
  }
  if kv_size <= key_size {
    return Err("--kv-size 必须大于 --key-size（key + value）".to_owned());
  }
  if samples == 0 {
    return Err("--samples 必须大于 0".to_owned());
  }
  if (reads == 0 || scans == 0) && !(read_secs.is_finite() && read_secs >= 1.0) {
    return Err(
      "--read-secs 必须 >= 1（时间窗档用；redb 档请给 --reads / --scans 固定次数）".to_owned(),
    );
  }
  for (name, value) in [("--rows", rows), ("--bulk-rows", bulk_rows)] {
    if value > u32::MAX as usize {
      return Err(format!(
        "{name} 超过 u32::MAX，两个 bench 的计数参数都无法表达"
      ));
    }
  }
  for (name, value) in [
    ("--individual-writes", individual_writes),
    ("--batch-writes", batch_writes),
    ("--reads", reads),
    ("--scans", scans),
    ("--removals", removals),
  ] {
    if value > u32::MAX as u64 {
      return Err(format!("{name} 超过 u32::MAX，C++ FFI 无法表达"));
    }
  }

  Ok((
    cpp_backend,
    rust_bin,
    MatrixConfig {
      workdir,
      profile,
      rows,
      bulk_rows,
      batch_size,
      kv_size,
      key_size,
      cache_bytes,
      read_secs,
      readers,
      scan_bytes,
      individual_writes,
      batch_writes,
      reads,
      scans,
      scan_len,
      removals,
      samples,
      phase_timeout_secs,
    },
  ))
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(CliError::Usage(message)) => {
      eprintln!("fkv_bench_matrix: {message}\n");
      eprint!("{USAGE}");
      ExitCode::from(2)
    }
    Err(CliError::Failed(message)) => {
      eprintln!("fkv_bench_matrix: {message}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), CliError> {
  let args = match parse_args(env::args().skip(1)).map_err(CliError::Usage)? {
    ParsedArgs::Help => {
      print!("{USAGE}");
      return Ok(());
    }
    ParsedArgs::Run(args) => args,
  };
  let markdown_path = args.markdown.clone();
  let json_path = args.json.clone();
  let (cpp_backend, rust_bin, cfg) = resolve_config(args).map_err(CliError::Usage)?;

  fs::create_dir_all(&cfg.workdir)
    .map_err(|err| CliError::Failed(format!("create workdir {}: {err}", cfg.workdir.display())))?;

  let cpp: Box<dyn BenchEngine> = match cpp_backend {
    CppBackend::Ffi(lib) => Box::new(
      CppFfiEngine::new(&lib)
        .map_err(|err| CliError::Failed(format!("load --cpp-lib {}: {err}", lib.display())))?,
    ),
    CppBackend::Bin(bin) => Box::new(CppEngine::new(bin)),
  };
  let fkv = FkvEngine::new(rust_bin);
  let report = run_matrix(cpp.as_ref(), &fkv, &cfg).map_err(CliError::Failed)?;

  let markdown = report.to_markdown();
  print!("{markdown}");
  if let Some(path) = markdown_path {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)
        .map_err(|err| CliError::Failed(format!("create {}: {err}", parent.display())))?;
    }
    fs::write(&path, &markdown)
      .map_err(|err| CliError::Failed(format!("write {}: {err}", path.display())))?;
  }
  if let Some(path) = json_path {
    let json = report
      .to_json_pretty()
      .map_err(|err| CliError::Failed(format!("serialize JSON: {err}")))?;
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)
        .map_err(|err| CliError::Failed(format!("create {}: {err}", parent.display())))?;
    }
    fs::write(&path, json)
      .map_err(|err| CliError::Failed(format!("write {}: {err}", path.display())))?;
  }
  Ok(())
}

#[derive(Debug)]
enum CliError {
  Usage(String),
  Failed(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use std::{fs, mem, time::SystemTime};

  use super::*;

  fn cfg() -> MatrixConfig {
    MatrixConfig {
      workdir: PathBuf::from("/tmp/matrix-test"),
      profile: Profile::Legacy,
      rows: 1000,
      bulk_rows: 1000,
      batch_size: 100,
      kv_size: 128,
      key_size: 8,
      cache_bytes: 0,
      read_secs: 10.0,
      readers: vec![1, 2, 4],
      scan_bytes: 0,
      individual_writes: 1000,
      batch_writes: 10,
      reads: 0,
      scans: 0,
      scan_len: 0,
      removals: 1000,
      samples: 1,
      phase_timeout_secs: 1800,
    }
  }

  fn has(args: &[String], needle: &str) -> bool {
    args.iter().any(|arg| arg == needle)
  }

  fn pair(args: &[String], key: &str) -> Option<String> {
    args
      .iter()
      .position(|arg| arg == key)
      .and_then(|index| args.get(index + 1).cloned())
  }

  #[test]
  fn cpp_commands_match_the_documented_mapping() {
    let cfg = cfg();
    let ini = Path::new("/tmp/x.ini");

    let load = cpp_load_args(&cfg, ini);
    assert!(has(&load, "--load=1"));
    assert_eq!(pair(&load, "--batch_size").as_deref(), Some("100"));

    let individual = cpp_write_args(&cfg, ini, 1, cfg.rows);
    assert_eq!(pair(&individual, "--workload").as_deref(), Some("write"));
    assert_eq!(pair(&individual, "--batch_size").as_deref(), Some("1"));
    assert_eq!(pair(&individual, "--write_batchs").as_deref(), Some("1000"));
    assert!(has(&individual, "--no_delete=1"));

    let batch = cpp_write_args(&cfg, ini, 100, 10);
    assert_eq!(pair(&batch, "--batch_size").as_deref(), Some("100"));
    assert_eq!(pair(&batch, "--write_batchs").as_deref(), Some("10"));

    let read = cpp_read_args(&cfg, ini, 4);
    assert_eq!(pair(&read, "--workload").as_deref(), Some("read"));
    assert_eq!(pair(&read, "--read_per_part").as_deref(), Some("4"));
    assert_eq!(pair(&read, "--read_thds").as_deref(), Some("1"));
    assert_eq!(pair(&read, "--test_secs").as_deref(), Some("10"));

    let scan = cpp_scan_args(&cfg, ini);
    assert_eq!(pair(&scan, "--workload").as_deref(), Some("scan"));

    let delete = cpp_delete_args(&cfg, ini, 100, 10);
    assert_eq!(pair(&delete, "--workload").as_deref(), Some("delete"));
    assert!(has(&delete, "--delete_only=1"));
    assert_eq!(pair(&delete, "--write_batchs").as_deref(), Some("10"));

    let compact = cpp_compact_args(&cfg, ini);
    assert_eq!(pair(&compact, "--workload").as_deref(), Some("compact"));
    assert_eq!(pair(&compact, "--kvoptions").as_deref(), Some("/tmp/x.ini"));
  }

  #[test]
  fn fkv_commands_match_the_documented_mapping() {
    let cfg = cfg();
    let store = Path::new("/tmp/store");
    let history = Path::new("/tmp/history.json");

    let load = fkv_load_args(&cfg, store, history);
    assert!(has(&load, "--load-only"));
    assert_eq!(pair(&load, "--store-dir").as_deref(), Some("/tmp/store"));
    assert_eq!(
      pair(&load, "--history").as_deref(),
      Some("/tmp/history.json")
    );

    let individual = fkv_write_args(&cfg, store, history, 1, cfg.rows);
    assert_eq!(
      pair(&individual, "--workload").as_deref(),
      Some("write-only")
    );
    assert_eq!(pair(&individual, "--batch-size").as_deref(), Some("1"));
    assert_eq!(pair(&individual, "--write-batchs").as_deref(), Some("1000"));
    assert_eq!(pair(&individual, "--rng").as_deref(), Some("grid"));
    assert_eq!(pair(&individual, "--del-ratio").as_deref(), Some("0"));

    let batch = fkv_write_args(&cfg, store, history, 100, 10);
    assert_eq!(pair(&batch, "--batch-size").as_deref(), Some("100"));
    assert_eq!(pair(&batch, "--write-batchs").as_deref(), Some("10"));

    let read = fkv_read_args(&cfg, store, history, 4);
    assert!(has(&read, "--read-only-existing"));
    assert_eq!(pair(&read, "--workload").as_deref(), Some("read-only"));
    assert_eq!(pair(&read, "--read-per-part").as_deref(), Some("4"));

    let scan = fkv_scan_args(&cfg, store, history);
    assert!(has(&scan, "--read-only-existing"));
    assert_eq!(pair(&scan, "--workload").as_deref(), Some("scan"));
    assert_eq!(pair(&scan, "--test-secs").as_deref(), Some("10"));

    let len = fkv_len_args(&cfg, store, history);
    assert!(has(&len, "--read-only-existing"));
    assert_eq!(pair(&len, "--workload").as_deref(), Some("scan"));
    assert_eq!(pair(&len, "--read-per-part").as_deref(), Some("1"));
    // `len` forces a single full-scan round regardless of `read_secs`.
    assert_eq!(pair(&len, "--test-secs").as_deref(), Some("0"));
    assert_eq!(pair(&len, "--scan-bytes").as_deref(), Some("0"));
    assert_eq!(pair(&len, "--store-dir").as_deref(), Some("/tmp/store"));
    assert_eq!(
      pair(&len, "--history").as_deref(),
      Some("/tmp/history.json")
    );

    let delete = fkv_delete_args(&cfg, store, history);
    assert_eq!(pair(&delete, "--workload").as_deref(), Some("delete-only"));

    let compact = fkv_compact_args(&cfg, store, history);
    assert!(has(&compact, "--read-only-existing"));
    assert_eq!(
      pair(&compact, "--workload").as_deref(),
      Some("compact-only")
    );
  }

  #[test]
  fn redb_commands_carry_the_aligned_counts() {
    let mut cfg = cfg();
    cfg.profile = Profile::Redb;
    cfg.rows = 1_000_000;
    cfg.bulk_rows = 5_000_000;
    cfg.batch_size = 1000;
    cfg.kv_size = 174;
    cfg.key_size = 24;
    cfg.cache_bytes = 64 * 1024 * 1024;
    cfg.individual_writes = 1000;
    cfg.batch_writes = 100;
    cfg.reads = 1_000_000;
    cfg.scans = 500_000;
    cfg.scan_len = 10;
    cfg.scan_bytes = 1500;
    cfg.removals = 500_000;
    cfg.samples = 3;
    let store = Path::new("/tmp/store");
    let history = Path::new("/tmp/h.json");

    let load = fkv_load_args(&cfg, store, history);
    assert_eq!(pair(&load, "--rows").as_deref(), Some("5000000"));
    assert_eq!(pair(&load, "--key-size").as_deref(), Some("24"));
    assert_eq!(pair(&load, "--kv-size").as_deref(), Some("174"));
    assert_eq!(
      pair(&load, "--buffer-pool-size").as_deref(),
      Some("67108864")
    );
    assert!(has(&load, "--load-only"));

    // Read / scan / len prepare the 1M dataset, not the 5M bulk-load set.
    let prepare = fkv_prepare_args(&cfg, store, history);
    assert_eq!(pair(&prepare, "--rows").as_deref(), Some("1000000"));
    assert_eq!(pair(&prepare, "--key-size").as_deref(), Some("24"));
    assert!(has(&prepare, "--load-only"));

    let individual = fkv_write_args(&cfg, store, history, 1, 1000);
    assert_eq!(pair(&individual, "--rows").as_deref(), Some("1000000"));
    assert_eq!(pair(&individual, "--batch-size").as_deref(), Some("1"));
    assert_eq!(pair(&individual, "--write-batchs").as_deref(), Some("1000"));

    let batch = fkv_write_args(&cfg, store, history, 1000, 100);
    assert_eq!(pair(&batch, "--batch-size").as_deref(), Some("1000"));
    assert_eq!(pair(&batch, "--write-batchs").as_deref(), Some("100"));

    let read = fkv_read_args(&cfg, store, history, 8);
    assert_eq!(pair(&read, "--read-count").as_deref(), Some("1000000"));
    assert_eq!(pair(&read, "--trials").as_deref(), Some("3"));
    assert_eq!(pair(&read, "--test-secs").as_deref(), Some("0"));
    assert_eq!(pair(&read, "--read-per-part").as_deref(), Some("8"));

    let scan = fkv_scan_args(&cfg, store, history);
    assert_eq!(pair(&scan, "--scan-count").as_deref(), Some("500000"));
    assert_eq!(pair(&scan, "--trials").as_deref(), Some("3"));
    assert_eq!(pair(&scan, "--test-secs").as_deref(), Some("0"));
    assert_eq!(pair(&scan, "--scan-bytes").as_deref(), Some("1500"));

    let delete = fkv_delete_args(&cfg, store, history);
    assert_eq!(pair(&delete, "--batch-size").as_deref(), Some("1000"));
    assert_eq!(pair(&delete, "--write-batchs").as_deref(), Some("500"));

    let len = fkv_len_args(&cfg, store, history);
    assert_eq!(pair(&len, "--trials").as_deref(), Some("3"));
  }

  #[test]
  fn fkv_len_args_matches_the_fullscan_lane() {
    // A non-default read window must not leak into the len lane: the point of
    // the lane is exactly one full-scan round, so `--test-secs` is pinned to 0.
    let mut cfg = cfg();
    cfg.read_secs = 3.5;

    let len = fkv_len_args(&cfg, Path::new("/tmp/store"), Path::new("/tmp/len.json"));
    assert!(has(&len, "--read-only-existing"));
    assert_eq!(pair(&len, "--workload").as_deref(), Some("scan"));
    assert_eq!(pair(&len, "--test-secs").as_deref(), Some("0"));
    assert_eq!(pair(&len, "--read-per-part").as_deref(), Some("1"));
    assert_eq!(pair(&len, "--impl").as_deref(), Some("rust"));
    assert_eq!(pair(&len, "--rows").as_deref(), Some("1000"));
    assert_eq!(pair(&len, "--kv-size").as_deref(), Some("128"));

    // The scan lane keeps the configured window; only `len` overrides it.
    let scan = fkv_scan_args(&cfg, Path::new("/tmp/store"), Path::new("/tmp/scan.json"));
    assert_eq!(pair(&scan, "--test-secs").as_deref(), Some("3.5"));
  }

  #[test]
  fn len_result_maps_fullscan_metrics() {
    let metrics: BTreeMap<String, f64> = [
      ("fullscan_num".to_owned(), 1_000_000.0),
      ("fullscan_total_us".to_owned(), 2_500_000.0),
    ]
    .into_iter()
    .collect();
    let result = result_from_fullscan(&metrics).expect("len result");
    assert_eq!(result.ops, 1_000_000);
    assert_eq!(result.duration, Duration::from_millis(2_500));
    assert_eq!(result.throughput, 400_000.0);
    assert_eq!(result.p99_us, None);
    assert_eq!(result.bytes, None);

    // Missing or non-positive inputs must be rejected, not silently zeroed.
    assert!(result_from_fullscan(&BTreeMap::new()).is_err());
    let no_total: BTreeMap<String, f64> = [("fullscan_num".to_owned(), 5.0)].into_iter().collect();
    assert!(result_from_fullscan(&no_total).is_err());
    let zero_total: BTreeMap<String, f64> = [
      ("fullscan_num".to_owned(), 5.0),
      ("fullscan_total_us".to_owned(), 0.0),
    ]
    .into_iter()
    .collect();
    assert!(result_from_fullscan(&zero_total).is_err());
  }

  #[test]
  fn fkv_engine_supports_len() {
    let engine = FkvEngine::new("fkv_bench");
    assert!(engine.supports(Phase::Len));
    assert!(engine.supports(Phase::BulkLoad));
    assert!(engine.supports(Phase::RandomReads { readers: 4 }));
  }

  #[test]
  fn dir_size_is_recursive() {
    let unique = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .expect("clock")
      .as_nanos();
    let root = env::temp_dir().join(format!("fkv_matrix_size_{unique}"));
    fs::create_dir_all(root.join("a/b")).expect("mkdir");
    fs::write(root.join("a/one"), vec![0u8; 10]).expect("write");
    fs::write(root.join("a/b/two"), vec![0u8; 25]).expect("write");
    assert_eq!(dir_size_bytes(&root).expect("size"), 35);
    assert_eq!(dir_size_bytes(&root.join("missing")).expect("missing"), 0);
    fs::remove_dir_all(&root).expect("cleanup");
  }

  #[test]
  fn parses_cpp_write_read_scan_and_compact_logs() {
    let write = "I2026 write summary 1234 kvs/s | cost 1 ms | average latency 8 microseconds | p50 5 microseconds | p90 6 microseconds | p99 77 microseconds | p99.9 99 microseconds";
    let metrics = cpp_metrics(write).expect("write metrics");
    assert_eq!(metrics.get("put_kv_s"), Some(&1234.0));
    assert_eq!(metrics.get("put_p99_us"), Some(&77.0));

    let read = "I2026 [0]read speed 10 QPS | average latency 1 microseconds | p50 1 microseconds | p90 1 microseconds | p99 42 microseconds\nI2026 [0]read average 500 QPS";
    let metrics = cpp_metrics(read).expect("read metrics");
    assert_eq!(metrics.get("get_kv_s"), Some(&500.0));
    assert_eq!(metrics.get("get_p99_us"), Some(&42.0));

    let scan = "I2026 [0]scan throughput 12 MB/s 100 kvs/s\nI2026 [0]scan speed 12 MB/s | average latency 2 microseconds | p50 1 microseconds | p90 1 microseconds | p99 31 microseconds";
    let metrics = cpp_metrics(scan).expect("scan metrics");
    assert_eq!(metrics.get("scan_kv_s"), Some(&100.0));
    assert_eq!(metrics.get("scan_p99_us"), Some(&31.0));

    let compact = "I2026 compact total 250 ms\nI2026 compact partitions 1";
    assert_eq!(
      parse_cpp_compact_duration(compact).expect("compact"),
      Duration::from_millis(250)
    );
  }

  #[test]
  fn parses_rust_history_shape() {
    let text = r#"{
      "runs": [
        {"impl": "rust", "metric": "put_kv_s", "value": 1234.0},
        {"impl": "rust", "metric": "put_p99_us", "value": 7.5}
      ]
    }"#;
    let parsed = fkv_perf_cross::parse_rust_history(text).expect("history");
    let metrics: BTreeMap<String, f64> = parsed
      .metrics
      .iter()
      .map(|(k, v)| (k.to_owned(), v))
      .collect();
    assert_eq!(metrics.get("put_kv_s"), Some(&1234.0));
    assert_eq!(metrics.get("put_p99_us"), Some(&7.5));
  }

  #[test]
  fn cli_parses_required_and_optional_flags() {
    let ParsedArgs::Run(args) = parse_args(
      [
        "--cpp-bin",
        "cpp",
        "--rust-bin",
        "rust",
        "--workdir",
        "/tmp/w",
        "--rows",
        "10",
        "--batch-size",
        "2",
        "--kv-size",
        "64",
        "--read-secs",
        "3.5",
        "--readers",
        "1,2,4",
        "--scan-bytes",
        "4096",
        "--markdown",
        "out.md",
        "--json",
        "out.json",
      ]
      .into_iter()
      .map(str::to_owned),
    )
    .expect("parse") else {
      panic!("expected Run");
    };
    assert_eq!(args.cpp_bin, Some(PathBuf::from("cpp")));
    assert_eq!(args.rust_bin, Some(PathBuf::from("rust")));
    assert_eq!(args.readers, Some(vec![1, 2, 4]));
    assert_eq!(args.read_secs, Some(3.5));
    assert_eq!(args.scan_bytes, Some(4096));
  }

  #[test]
  fn cli_help_and_validation() {
    assert!(matches!(
      parse_args(["--help"].into_iter().map(str::to_owned)),
      Ok(ParsedArgs::Help)
    ));
    assert!(parse_args(["--readers", "1,0"].into_iter().map(str::to_owned)).is_err());
    assert!(parse_args(["--nope"].into_iter().map(str::to_owned)).is_err());
    assert!(parse_args(["--cpp-bin"].into_iter().map(str::to_owned)).is_err());
  }

  #[test]
  fn batch_count_follows_the_explicit_batch_writes() {
    let mut cfg = cfg();
    cfg.batch_writes = 11;
    assert_eq!(cfg.batch_count(), 11);
    // The redb profile sets 100 batches explicitly, independent of rows.
    cfg.rows = 1_000_000;
    cfg.batch_writes = 100;
    assert_eq!(cfg.batch_count(), 100);
  }

  #[test]
  fn cli_parses_cpp_lib() {
    let ParsedArgs::Run(args) = parse_args(
      ["--cpp-lib", "libfkv_bench_ffi.dylib", "--rust-bin", "rust"]
        .into_iter()
        .map(str::to_owned),
    )
    .expect("parse") else {
      panic!("expected Run");
    };
    assert_eq!(args.cpp_lib, Some(PathBuf::from("libfkv_bench_ffi.dylib")));
    assert_eq!(args.cpp_bin, None);
    assert!(
      parse_args(
        ["--cpp-lib", "a", "--cpp-lib", "b"]
          .into_iter()
          .map(str::to_owned)
      )
      .is_err()
    );
  }

  #[test]
  fn backend_prefers_ffi_then_falls_back_to_bin() {
    let both = Args {
      cpp_lib: Some(PathBuf::from("libfkv_bench_ffi.so")),
      cpp_bin: Some(PathBuf::from("simple_bench")),
      ..Args::default()
    };
    assert_eq!(
      both.cpp_backend().expect("backend"),
      CppBackend::Ffi(PathBuf::from("libfkv_bench_ffi.so"))
    );

    let legacy = Args {
      cpp_bin: Some(PathBuf::from("simple_bench")),
      ..Args::default()
    };
    assert_eq!(
      legacy.cpp_backend().expect("backend"),
      CppBackend::Bin(PathBuf::from("simple_bench"))
    );

    let err = Args::default().cpp_backend().expect_err("no backend");
    assert!(err.contains("--cpp-lib"));
    assert!(err.contains("--cpp-bin"));
  }

  #[test]
  fn resolve_config_accepts_ffi_without_cpp_bin() {
    let ParsedArgs::Run(args) = parse_args(
      [
        "--cpp-lib",
        "/tmp/libfkv_bench_ffi.dylib",
        "--rust-bin",
        "/tmp/fkv_bench",
      ]
      .into_iter()
      .map(str::to_owned),
    )
    .expect("parse") else {
      panic!("expected Run");
    };
    let (backend, rust_bin, cfg) = resolve_config(args).expect("config");
    assert_eq!(
      backend,
      CppBackend::Ffi(PathBuf::from("/tmp/libfkv_bench_ffi.dylib"))
    );
    assert_eq!(rust_bin, PathBuf::from("/tmp/fkv_bench"));
    // Default profile is redb, whose README table uses 1/4/8/16/32 threads.
    assert_eq!(cfg.readers, vec![1, 4, 8, 16, 32]);
    assert_eq!(cfg.profile, Profile::Redb);
    assert_eq!(cfg.rows, 1_000_000);
    assert_eq!(cfg.bulk_rows, 5_000_000);
    assert_eq!(cfg.kv_size, 174);
    assert_eq!(cfg.key_size, 24);
    assert_eq!(cfg.cache_bytes, 64 * 1024 * 1024);
    assert_eq!(cfg.individual_writes, 1_000);
    assert_eq!(cfg.batch_writes, 100);
    assert_eq!(cfg.reads, 1_000_000);
    assert_eq!(cfg.scans, 500_000);
    assert_eq!(cfg.scan_len, 10);
    assert_eq!(cfg.removals, 500_000);
    assert_eq!(cfg.samples, 3);
    // redb: 10 entries per scan request, expressed as bytes for fkv_bench.
    assert_eq!(cfg.scan_bytes, 10 * 150);
  }

  #[test]
  fn legacy_profile_keeps_the_old_shape() {
    let ParsedArgs::Run(args) = parse_args(
      [
        "--profile",
        "legacy",
        "--cpp-lib",
        "libfkv_bench_ffi.so",
        "--rust-bin",
        "fkv_bench",
      ]
      .into_iter()
      .map(str::to_owned),
    )
    .expect("parse") else {
      panic!("expected Run");
    };
    let (_, _, cfg) = resolve_config(args).expect("config");
    assert_eq!(cfg.profile, Profile::Legacy);
    assert_eq!(cfg.rows, 200_000);
    assert_eq!(cfg.bulk_rows, 200_000);
    assert_eq!(cfg.kv_size, 128);
    assert_eq!(cfg.key_size, 8);
    assert_eq!(cfg.cache_bytes, 0);
    assert_eq!(cfg.read_secs, 5.0);
    assert_eq!(cfg.reads, 0);
    assert_eq!(cfg.scans, 0);
    assert_eq!(cfg.samples, 1);
    assert_eq!(cfg.individual_writes, 200_000);
    assert_eq!(cfg.batch_writes, 200);
    assert_eq!(cfg.removals, 200_000);
    assert_eq!(cfg.scan_bytes, 0);
  }

  #[test]
  fn ffi_phase_names_match_the_abi() {
    assert_eq!(ffi_phase_name(Phase::BulkLoad), "bulk_load");
    assert_eq!(ffi_phase_name(Phase::IndividualWrites), "individual_writes");
    assert_eq!(ffi_phase_name(Phase::BatchWrites), "batch_writes");
    assert_eq!(
      ffi_phase_name(Phase::RandomReads { readers: 4 }),
      "random_reads"
    );
    assert_eq!(ffi_phase_name(Phase::RangeReads), "range_reads");
    assert_eq!(ffi_phase_name(Phase::Removals), "removals");
    assert_eq!(ffi_phase_name(Phase::Len), "len");
  }

  #[test]
  fn ffi_config_carries_counts_threads_and_cache() {
    let mut cfg = cfg();
    cfg.profile = Profile::Redb;
    cfg.rows = 1_000_000;
    cfg.bulk_rows = 5_000_000;
    cfg.batch_size = 1000;
    cfg.kv_size = 174;
    cfg.key_size = 24;
    cfg.cache_bytes = 64 * 1024 * 1024;
    cfg.individual_writes = 1_000;
    cfg.batch_writes = 100;
    cfg.reads = 1_000_000;
    cfg.scans = 500_000;
    cfg.scan_len = 10;
    cfg.removals = 500_000;

    let read = ffi_config(Phase::RandomReads { readers: 8 }, &cfg);
    assert_eq!(read.rows, 1_000_000);
    assert_eq!(read.bulk_rows, 5_000_000);
    assert_eq!(read.key_size, 24);
    assert_eq!(read.kv_size, 174);
    assert_eq!(read.buffer_pool_size, 64 * 1024 * 1024);
    assert_eq!(read.num_threads, 1);
    assert_eq!(read.reads, 1_000_000);
    assert_eq!(read.range_scans, 500_000);
    assert_eq!(read.range_len, 10);
    assert_eq!(read.removals, 500_000);

    // Writes / scans / len stay single-threaded; only point reads fan out.
    assert_eq!(ffi_readers(Phase::BulkLoad), 1);
    assert_eq!(ffi_readers(Phase::IndividualWrites), 1);
    assert_eq!(ffi_readers(Phase::BatchWrites), 1);
    assert_eq!(ffi_readers(Phase::RangeReads), 1);
    assert_eq!(ffi_readers(Phase::Removals), 1);
    assert_eq!(ffi_readers(Phase::Len), 1);

    // Only the read / scan / len lanes are repeated.
    assert_eq!(cfg.samples, 1);
    cfg.samples = 3;
    assert_eq!(phase_samples(Phase::BulkLoad, &cfg), 1);
    assert_eq!(phase_samples(Phase::RandomReads { readers: 1 }, &cfg), 3);
    assert_eq!(phase_samples(Phase::RangeReads, &cfg), 3);
    assert_eq!(phase_samples(Phase::Len, &cfg), 3);
  }

  #[test]
  fn ffi_config_layout_matches_the_c_header() {
    // FkvBenchConfig: 7 x u32, u64 (8-byte aligned), 6 x u32 = 64 bytes.
    assert_eq!(mem::size_of::<FfiConfig>(), 64);
    assert_eq!(mem::align_of::<FfiConfig>(), 8);
    assert_eq!(FFI_ABI_VERSION, 2);
  }

  #[test]
  fn median_picks_the_middle_throughput() {
    let result = |ops: u64, secs: f64| PhaseResult {
      ops,
      duration: Duration::from_secs_f64(secs),
      throughput: ops as f64 / secs,
      p99_us: None,
      bytes: None,
    };
    let median = median_phase_result(vec![
      result(1000, 1.0),
      result(1000, 2.0),
      result(1000, 0.5),
    ]);
    assert!((median.throughput - 1000.0).abs() < 1e-9);
  }

  #[test]
  fn ffi_result_maps_to_phase_result() {
    let result = result_from_ffi(
      FfiBenchResult {
        ops: 1000,
        secs: 0.5,
        p99_us: 42.0,
      },
      "bulk_load",
    )
    .expect("result");
    assert_eq!(result.ops, 1000);
    assert_eq!(result.duration, Duration::from_millis(500));
    assert_eq!(result.throughput, 2000.0);
    assert_eq!(result.p99_us, Some(42.0));

    let no_p99 = result_from_ffi(
      FfiBenchResult {
        ops: 10,
        secs: 1.0,
        p99_us: 0.0,
      },
      "len",
    )
    .expect("result");
    assert_eq!(no_p99.p99_us, None);

    assert!(
      result_from_ffi(
        FfiBenchResult {
          ops: 1,
          secs: 0.0,
          p99_us: 0.0,
        },
        "len",
      )
      .is_err()
    );
  }

  #[test]
  fn ffi_engine_reports_missing_library_without_panicking() {
    let unique = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .expect("clock")
      .as_nanos();
    let path = env::temp_dir().join(format!("missing_libfkv_bench_ffi_{unique}.so"));
    let err = CppFfiEngine::new(&path).expect_err("missing library must fail");
    assert!(err.contains("dlopen"), "{err}");
  }

  /// 卡死的阶段必须在超时后被杀掉，而不是拖垮整个 job。
  #[test]
  fn execute_kills_a_hung_process() {
    let unique = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .expect("clock")
      .as_nanos();
    let dir = env::temp_dir().join(format!("fkv_matrix_timeout_{unique}"));
    fs::create_dir_all(&dir).expect("mkdtemp");
    let log = dir.join("sleep.log");

    let started = Instant::now();
    let err = execute(
      Path::new("/bin/sleep"),
      &["5".to_owned()],
      &log,
      Some(Duration::from_millis(300)),
    )
    .expect_err("sleep 5 must be killed by a 300ms cap");
    assert!(err.contains("未结束"), "{err}");
    assert!(
      started.elapsed() < Duration::from_secs(4),
      "kill 不够及时: {:?}",
      started.elapsed()
    );
    // 日志仍要落盘，便于事后定位卡在哪。
    assert!(log.exists(), "日志未写出");
  }
}
