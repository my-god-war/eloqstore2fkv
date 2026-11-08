//! C++ `simple_bench` 日志 ↔ Rust `fkv_bench --history` JSON 的跨实现性能对比。
//!
//! 本 crate **不发布**，只把 `scripts/perf_gate.sh` 里 `produce` 步骤使用的 Python
//! 解析逻辑与比值口径搬进 Rust，供 CI / 网站直接复用：
//!
//! - [`parse_cpp_log`]：按 `scripts/perf_gate.sh` 的正则解析 C++ 日志；
//! - [`parse_rust_history`]：解析 `fkv_bench --history` 的 `runs`，也兼容
//!   `perf-baseline.json` 的 `rust.metrics` 形状；
//! - [`CompareReport`]：统一的逐项 baseline/current/delta%/比值报告，可导出
//!   JSON 与 Markdown。
//!
//! 已知口径差异（与脚本一致，不隐藏）：
//!
//! - C++ `write-read` 是写线程与读线程并发；Rust 是「先写后读」两阶段，
//!   因此 `current rust / baseline cpp` 只作同 runner 趋势比值；
//! - `put_avg_us` 只在 C++ 侧出现，Rust 侧通常没有对应指标，报告会保留
//!   baseline 并标记为仅在 C++ 侧存在；
//! - 延迟与 RSS 指标方向相反（越低越好），不进入吞吐比值门槛。

use std::{
  collections::{BTreeMap, BTreeSet},
  error::Error,
  fmt,
};

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

/// 固定 workload 的字段，与 `scripts/perf_gate.sh` 的 `workload_keys` 保持一致。
pub const WORKLOAD_KEYS: [&str; 12] = [
  "workload",
  "rows",
  "max_key",
  "batch_size",
  "read_per_part",
  "partitions",
  "kv_size",
  "key_interval",
  "del_ratio",
  "test_secs",
  "write_batchs",
  "scan_bytes",
];

/// 报告里优先展示的指标顺序；其余指标按字典序追加。
const METRIC_ORDER: [&str; 8] = [
  "put_kv_s",
  "put_avg_us",
  "put_p99_us",
  "put_p999_us",
  "get_kv_s",
  "get_p99_us",
  "scan_kv_s",
  "scan_p99_us",
];

/// 解析失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
  message: String,
}

impl ParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }

  /// 错误文本。
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for ParseError {}

/// 一组具名指标（数值）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Metrics(BTreeMap<String, f64>);

impl Metrics {
  /// 读取指标；不存在时为 `None`。
  #[must_use]
  pub fn get(&self, name: &str) -> Option<f64> {
    self.0.get(name).copied()
  }

  /// 指标是否存在。
  #[must_use]
  pub fn contains(&self, name: &str) -> bool {
    self.0.contains_key(name)
  }

  /// 是否为空。
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// 指标数量。
  #[must_use]
  pub fn len(&self) -> usize {
    self.0.len()
  }

  /// 按名称升序遍历。
  pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> {
    self.0.iter().map(|(name, value)| (name.as_str(), *value))
  }
}

impl FromIterator<(String, f64)> for Metrics {
  fn from_iter<T: IntoIterator<Item = (String, f64)>>(iter: T) -> Self {
    Self(iter.into_iter().collect())
  }
}

/// `fkv_bench --history` 解析结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RustHistory {
  /// 产出该 history 的 runner（`fkv_bench` / baseline 的 `runner` 字段）。
  pub runner: Option<String>,
  /// 固定 workload 字段（只保留 [`WORKLOAD_KEYS`] 中出现的项）。
  pub workload: BTreeMap<String, Value>,
  /// `impl == "rust"` 的指标。
  pub metrics: Metrics,
}

/// 解析 C++ `simple_bench` 日志，正则与 `scripts/perf_gate.sh produce` 一致。
///
/// 支持 `put_kv_s` / `put_avg_us` / `put_p99_us` / `put_p999_us` /
/// `get_kv_s` / `get_p99_us`，并顺带解析 scan 指标：
///
/// - `get_kv_s` 是各 `read average N QPS` 之和；
/// - `get_p99_us` 是各 `read speed ... p99 N microseconds` 的最大值；
/// - `scan_kv_s` 是各 `scan throughput N MB/s M kvs/s` 的 kvs/s 之和；
/// - `scan_p99_us` 是各 `scan speed ... p99 N microseconds` 的最大值。
///
/// 一个指标都没解析到时返回 [`ParseError`]。
pub fn parse_cpp_log(text: &str) -> Result<Metrics, ParseError> {
  let mut metrics = BTreeMap::new();

  let write_re = Regex::new(
    r"write summary (\d+) kvs/s .*?average latency (\d+) microseconds .*?p99 (\d+) microseconds .*?p99\.9 (\d+) microseconds",
  )
  .expect("write summary regex is valid");
  if let Some(caps) = write_re.captures(text) {
    for (index, name) in ["put_kv_s", "put_avg_us", "put_p99_us", "put_p999_us"]
      .into_iter()
      .enumerate()
    {
      let raw = caps.get(index + 1).expect("capture group exists").as_str();
      let value = raw
        .parse::<f64>()
        .map_err(|_| ParseError::new(format!("C++ 日志中的 {name} 不是数字: {raw}")))?;
      metrics.insert(name.to_owned(), value);
    }
  }

  let read_qps_re = Regex::new(r"read average (\d+) QPS").expect("read qps regex is valid");
  let read_qps = read_qps_re
    .captures_iter(text)
    .map(|caps| caps[1].parse::<f64>())
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| ParseError::new("C++ 日志中的 read QPS 不是数字"))?;
  if !read_qps.is_empty() {
    metrics.insert("get_kv_s".to_owned(), read_qps.iter().sum());
  }

  let read_p99_re =
    Regex::new(r"read speed .*?p99 (\d+) microseconds").expect("read p99 regex is valid");
  let read_p99 = read_p99_re
    .captures_iter(text)
    .map(|caps| caps[1].parse::<f64>())
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| ParseError::new("C++ 日志中的 read p99 不是数字"))?;
  if let Some(value) = read_p99.into_iter().reduce(f64::max) {
    metrics.insert("get_p99_us".to_owned(), value);
  }

  let scan_kvs_re =
    Regex::new(r"scan throughput \d+ MB/s (\d+) kvs/s").expect("scan throughput regex is valid");
  let scan_kvs = scan_kvs_re
    .captures_iter(text)
    .map(|caps| caps[1].parse::<f64>())
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| ParseError::new("C++ 日志中的 scan kvs/s 不是数字"))?;
  if !scan_kvs.is_empty() {
    metrics.insert("scan_kv_s".to_owned(), scan_kvs.iter().sum());
  }

  let scan_p99_re =
    Regex::new(r"scan speed .*?p99 (\d+) microseconds").expect("scan p99 regex is valid");
  let scan_p99 = scan_p99_re
    .captures_iter(text)
    .map(|caps| caps[1].parse::<f64>())
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| ParseError::new("C++ 日志中的 scan p99 不是数字"))?;
  if let Some(value) = scan_p99.into_iter().reduce(f64::max) {
    metrics.insert("scan_p99_us".to_owned(), value);
  }

  if metrics.is_empty() {
    return Err(ParseError::new(
      "no C++ metrics parsed; check simple_bench log format",
    ));
  }
  Ok(Metrics(metrics))
}

/// 解析 Rust `fkv_bench --history` JSON。
///
/// 主形状是顶层 `runs: [{impl, metric, value}]`，只取 `impl == "rust"` 的项。
/// 若没有 `runs`（例如直接喂 `perf-baseline.json`），回退到 `rust.metrics` 对象。
/// 数值接受 JSON number 或可解析的数字字符串，与脚本的 `float(...)` 一致。
pub fn parse_rust_history(text: &str) -> Result<RustHistory, ParseError> {
  let root: Value = serde_json::from_str(text)
    .map_err(|err| ParseError::new(format!("Rust history JSON 无效: {err}")))?;
  let object = root
    .as_object()
    .ok_or_else(|| ParseError::new("Rust history 顶层必须是 JSON 对象"))?;

  let mut metrics = BTreeMap::new();
  if let Some(runs) = object.get("runs") {
    let runs = runs
      .as_array()
      .ok_or_else(|| ParseError::new("Rust history 的 runs 必须是数组"))?;
    for run in runs {
      if run.get("impl").and_then(Value::as_str) != Some("rust") {
        continue;
      }
      let name = run
        .get("metric")
        .and_then(Value::as_str)
        .ok_or_else(|| ParseError::new("Rust history 的 rust run 缺少字符串 metric"))?;
      let value = run
        .get("value")
        .ok_or_else(|| ParseError::new(format!("Rust history 的 {name} 缺少 value")))?;
      metrics.insert(name.to_owned(), number_value(value, name)?);
    }
  }

  if metrics.is_empty()
    && let Some(legacy) = object
      .get("rust")
      .and_then(Value::as_object)
      .and_then(|rust| rust.get("metrics"))
      .and_then(Value::as_object)
  {
    for (name, value) in legacy {
      metrics.insert(name.clone(), number_value(value, name)?);
    }
  }

  if metrics.is_empty() {
    return Err(ParseError::new("no rust metrics in history"));
  }

  let mut workload = BTreeMap::new();
  for key in WORKLOAD_KEYS {
    if let Some(value) = object.get(key) {
      workload.insert(key.to_owned(), value.clone());
    }
  }

  let runner = object
    .get("runner")
    .and_then(Value::as_str)
    .filter(|runner| !runner.is_empty())
    .map(str::to_owned);

  Ok(RustHistory {
    runner,
    workload,
    metrics: Metrics(metrics),
  })
}

fn number_value(value: &Value, name: &str) -> Result<f64, ParseError> {
  if let Some(number) = value.as_f64() {
    return Ok(number);
  }
  if let Some(raw) = value.as_str()
    && let Ok(number) = raw.trim().parse::<f64>()
  {
    return Ok(number);
  }
  Err(ParseError::new(format!(
    "Rust history 的 {name} value 不是数字: {value}"
  )))
}

/// 指标方向：`true` 表示越低越好。
#[must_use]
pub fn lower_is_better(metric: &str) -> bool {
  metric == "rss_peak_kb"
    || (metric.ends_with("_us")
      && ["avg", "p50", "p90", "p99", "max"]
        .iter()
        .any(|token| metric.contains(token)))
}

/// 是否进入同 runner 吞吐比值门槛（与脚本 `ratios` 的筛选一致）。
#[must_use]
pub fn ratio_gate_metric(metric: &str) -> bool {
  !metric.ends_with("_us") && metric != "rss_peak_kb"
}

/// 单项对比结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
  /// 数值更好。
  Improved,
  /// 数值更差。
  Regressed,
  /// 完全相等。
  Flat,
  /// 至少一侧缺失，无法比较。
  Missing,
}

/// 单项指标对比。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricCompare {
  /// 指标名。
  pub metric: String,
  /// 是否越低越好（延迟 / RSS）。
  pub lower_is_better: bool,
  /// C++ baseline；缺失时为 `None`。
  pub baseline: Option<f64>,
  /// Rust current；缺失时为 `None`。
  pub current: Option<f64>,
  /// `(current - baseline) / baseline * 100`；任一侧缺失或 baseline <= 0 时为 `None`。
  pub delta_pct: Option<f64>,
  /// `current / baseline`；任一侧缺失或 baseline <= 0 时为 `None`。
  pub ratio: Option<f64>,
  /// 该比值是否进入脚本的吞吐门槛（非 `*_us`、非 `rss_peak_kb`）。
  pub ratio_gate: bool,
  /// 按指标方向给出的结论。
  pub verdict: Verdict,
}

/// 统一对比报告。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompareReport {
  /// 报告 schema 版本，便于网站/CI 消费。
  pub schema: u32,
  /// 固定为 `"cpp"`。
  pub baseline_impl: &'static str,
  /// 固定为 `"rust"`。
  pub current_impl: &'static str,
  /// Rust history 里的 runner；缺失时为 `None`。
  #[serde(skip_serializing_if = "Option::is_none")]
  pub runner: Option<String>,
  /// Rust history 里的固定 workload 字段。
  pub workload: BTreeMap<String, Value>,
  /// C++ baseline 指标。
  pub cpp: Metrics,
  /// Rust current 指标。
  pub rust: Metrics,
  /// 逐项对比，固定指标优先、其余按字典序。
  pub metrics: Vec<MetricCompare>,
  /// 缺失指标 / 缺失 runner 等提示。
  pub warnings: Vec<String>,
}

impl CompareReport {
  /// 用 C++ 日志指标（baseline）与 Rust history（current）生成报告。
  #[must_use]
  pub fn compare(cpp: &Metrics, rust: &RustHistory) -> Self {
    let names = metric_union(cpp, &rust.metrics);
    let mut warnings = Vec::new();

    if rust.runner.is_none() {
      warnings.push("rust history 未提供 runner；同 runner 比值需调用方自行确认".to_owned());
    }
    if rust.workload.is_empty() {
      warnings.push("rust history 未提供 workload 字段；无法校验两侧 workload 一致".to_owned());
    }

    let metrics = names
      .into_iter()
      .map(|metric| {
        let baseline = cpp.get(&metric);
        let current = rust.metrics.get(&metric);
        if baseline.is_none() {
          warnings.push(format!("cpp 缺少指标 {metric}（仅 Rust 侧有）"));
        }
        if current.is_none() {
          warnings.push(format!("rust 缺少指标 {metric}（仅 C++ 侧有）"));
        }

        let (delta_pct, ratio) = match (baseline, current) {
          (Some(baseline), Some(current)) if baseline > 0.0 => (
            Some((current - baseline) / baseline * 100.0),
            Some(current / baseline),
          ),
          _ => (None, None),
        };

        let lower = lower_is_better(&metric);
        let ratio_gate = ratio_gate_metric(&metric);
        let verdict = match (baseline, current, delta_pct) {
          (Some(baseline), Some(current), _) if baseline == current => Verdict::Flat,
          (Some(_), Some(_), Some(delta)) => {
            let improved = if lower { delta < 0.0 } else { delta > 0.0 };
            if improved {
              Verdict::Improved
            } else {
              Verdict::Regressed
            }
          }
          _ => Verdict::Missing,
        };

        MetricCompare {
          metric,
          lower_is_better: lower,
          baseline,
          current,
          delta_pct,
          ratio,
          ratio_gate,
          verdict,
        }
      })
      .collect();

    Self {
      schema: 1,
      baseline_impl: "cpp",
      current_impl: "rust",
      runner: rust.runner.clone(),
      workload: rust.workload.clone(),
      cpp: cpp.clone(),
      rust: rust.metrics.clone(),
      metrics,
      warnings,
    }
  }

  /// 导出 Markdown 表（末行带换行）。
  #[must_use]
  pub fn to_markdown(&self) -> String {
    use fmt::Write as _;

    let mut out = String::new();
    out.push_str("# C++ simple_bench vs Rust fkv_bench\n\n");
    let _ = writeln!(
      out,
      "- baseline: C++ `simple_bench` 日志（{}）",
      self.baseline_impl
    );
    let _ = writeln!(
      out,
      "- current: Rust `fkv_bench --history`（{}）",
      self.current_impl
    );
    let _ = writeln!(
      out,
      "- runner: {}",
      self.runner.as_deref().unwrap_or("未知")
    );
    if self.workload.is_empty() {
      out.push_str("- workload: 未提供\n");
    } else {
      let ordered = WORKLOAD_KEYS
        .iter()
        .filter_map(|key| self.workload.get(*key).map(|value| (*key, value)))
        .chain(
          self
            .workload
            .iter()
            .filter(|(key, _)| !WORKLOAD_KEYS.contains(&key.as_str()))
            .map(|(key, value)| (key.as_str(), value)),
        );
      let fields = ordered
        .map(|(key, value)| format!("{key}={}", workload_value(value)))
        .collect::<Vec<_>>()
        .join(" ");
      let _ = writeln!(out, "- workload: {fields}");
    }
    out.push('\n');

    out.push_str("| metric | baseline (cpp) | current (rust) | delta% | ratio (cur/base) | 门槛比值 | 结论 |\n");
    out.push_str("|---|---:|---:|---:|---:|:---:|---|\n");
    for row in &self.metrics {
      let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} | {} | {} |",
        row.metric,
        fmt_metric(row.baseline),
        fmt_metric(row.current),
        row
          .delta_pct
          .map_or_else(|| "-".to_owned(), |delta| format!("{delta:+.1}%")),
        row
          .ratio
          .map_or_else(|| "-".to_owned(), |ratio| format!("{ratio:.4}")),
        if row.ratio_gate { "是" } else { "否" },
        verdict_label(row.verdict),
      );
    }

    out.push_str(
      "\n> delta% = (current - baseline) / baseline × 100；baseline <= 0 或一侧缺失时不计算。\n\
       > ratio 只在同一 runner、两侧都有该指标且 baseline > 0 时给出。\n\
       > 「门槛比值」仅对吞吐指标开放（不是 `*_us`、不是 `rss_peak_kb`），与 `scripts/perf_gate.sh` 的 `ratios` 口径一致。\n\
       > 延迟 / RSS 为越低越好；C++ write-read 并发、Rust 先写后读，跨实现绝对值不承诺等价。\n",
    );

    if !self.warnings.is_empty() {
      out.push_str("\n## 备注\n\n");
      for warning in &self.warnings {
        let _ = writeln!(out, "- {warning}");
      }
    }

    out
  }

  /// 导出缩进 JSON（末行带换行）。
  ///
  /// # Errors
  ///
  /// 仅在序列化失败时返回错误（正常数值不会失败）。
  pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(self)?;
    json.push('\n');
    Ok(json)
  }

  /// 导出 JSON 值，便于调用方嵌入更大的文档。
  ///
  /// # Errors
  ///
  /// 仅在序列化失败时返回错误。
  pub fn to_json_value(&self) -> Result<Value, serde_json::Error> {
    serde_json::to_value(self)
  }
}

fn metric_union(cpp: &Metrics, rust: &Metrics) -> Vec<String> {
  let names = cpp
    .iter()
    .map(|(name, _)| name)
    .chain(rust.iter().map(|(name, _)| name))
    .collect::<BTreeSet<_>>();

  let mut ordered = Vec::with_capacity(names.len());
  for name in METRIC_ORDER {
    if names.contains(name) {
      ordered.push(name.to_owned());
    }
  }
  for name in names {
    if !METRIC_ORDER.contains(&name) {
      ordered.push(name.to_owned());
    }
  }
  ordered
}

fn fmt_metric(value: Option<f64>) -> String {
  value.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}"))
}

fn verdict_label(verdict: Verdict) -> &'static str {
  match verdict {
    Verdict::Improved => "improved",
    Verdict::Regressed => "regressed",
    Verdict::Flat => "flat",
    Verdict::Missing => "missing",
  }
}

fn workload_value(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    Value::Null => "-".to_owned(),
    other => other.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  const CPP_LOG: &str = "\
[2026-11-05 10:00:00] workload write-read start
[2026-11-05 10:00:05] write summary 120000 kvs/s average latency 83 microseconds p99 140 microseconds p99.9 420 microseconds
[2026-11-05 10:00:06] read average 100000 QPS
[2026-11-05 10:00:07] read average 150000 QPS
[2026-11-05 10:00:08] read speed 12 MB/s, p99 210 microseconds
[2026-11-05 10:00:09] read speed 13 MB/s, p99 250 microseconds
[2026-11-05 10:00:10] scan throughput 24 MB/s 3600 kvs/s
[2026-11-05 10:00:11] scan throughput 25 MB/s 4000 kvs/s
[2026-11-05 10:00:12] scan speed 30 MB/s, p99 900 microseconds
[2026-11-05 10:00:13] scan speed 31 MB/s, p99 1100 microseconds
";

  const RUST_HISTORY: &str = r#"
{
  "workload": "write-read",
  "rows": 1000000,
  "max_key": 1000000,
  "batch_size": 100,
  "read_per_part": 4,
  "partitions": 1,
  "kv_size": 128,
  "key_interval": 4,
  "del_ratio": 4,
  "test_secs": 30,
  "write_batchs": 10000,
  "scan_bytes": 0,
  "runner": "linux-x86_64",
  "runs": [
    {"impl": "rust", "metric": "put_kv_s", "value": 108000},
    {"impl": "rust", "metric": "put_p99_us", "value": "150.5"},
    {"impl": "rust", "metric": "get_kv_s", "value": 270000},
    {"impl": "rust", "metric": "get_p99_us", "value": 240},
    {"impl": "cpp", "metric": "put_kv_s", "value": 999999},
    {"impl": "rust", "metric": "rss_peak_kb", "value": 204800}
  ]
}
"#;

  #[test]
  fn parses_cpp_log_like_perf_gate() {
    let metrics = parse_cpp_log(CPP_LOG).expect("cpp log parses");
    assert_eq!(metrics.get("put_kv_s"), Some(120_000.0));
    assert_eq!(metrics.get("put_avg_us"), Some(83.0));
    assert_eq!(metrics.get("put_p99_us"), Some(140.0));
    assert_eq!(metrics.get("put_p999_us"), Some(420.0));
    // 多个读线程 QPS 求和，p99 取最大值。
    assert_eq!(metrics.get("get_kv_s"), Some(250_000.0));
    assert_eq!(metrics.get("get_p99_us"), Some(250.0));
    assert_eq!(metrics.get("scan_kv_s"), Some(7_600.0));
    assert_eq!(metrics.get("scan_p99_us"), Some(1_100.0));
  }

  #[test]
  fn rejects_log_without_metrics() {
    let error = parse_cpp_log("nothing to see here").expect_err("no metrics");
    assert!(error.message().contains("no C++ metrics parsed"));
  }

  #[test]
  fn parses_rust_history_runs_and_workload() {
    let history = parse_rust_history(RUST_HISTORY).expect("history parses");
    assert_eq!(history.runner.as_deref(), Some("linux-x86_64"));
    assert_eq!(history.metrics.get("put_kv_s"), Some(108_000.0));
    assert_eq!(history.metrics.get("put_p99_us"), Some(150.5));
    assert_eq!(history.metrics.get("get_kv_s"), Some(270_000.0));
    assert_eq!(history.metrics.get("rss_peak_kb"), Some(204_800.0));
    assert_eq!(history.metrics.len(), 5, "非 rust run 被忽略");
    assert_eq!(history.workload.len(), WORKLOAD_KEYS.len());
    assert_eq!(history.workload["workload"], json!("write-read"));
    assert_eq!(history.workload["rows"], json!(1_000_000));
  }

  #[test]
  fn parses_baseline_shaped_rust_metrics() {
    let text = r#"{"runner":"r1","rust":{"metrics":{"put_kv_s":90000,"get_kv_s":"180000"}}}"#;
    let history = parse_rust_history(text).expect("baseline shape parses");
    assert_eq!(history.metrics.get("put_kv_s"), Some(90_000.0));
    assert_eq!(history.metrics.get("get_kv_s"), Some(180_000.0));
  }

  #[test]
  fn rejects_rust_history_without_metrics() {
    let error = parse_rust_history(r#"{"runs":[{"impl":"cpp","metric":"put_kv_s","value":1}]}"#)
      .expect_err("no rust metrics");
    assert_eq!(error.message(), "no rust metrics in history");

    let error = parse_rust_history("not json").expect_err("bad json");
    assert!(error.message().contains("JSON 无效"));
  }

  #[test]
  fn rejects_non_numeric_rust_value() {
    let error =
      parse_rust_history(r#"{"runs":[{"impl":"rust","metric":"put_kv_s","value":"fast"}]}"#)
        .expect_err("bad value");
    assert!(error.message().contains("put_kv_s"));
  }

  #[test]
  fn compares_metrics_with_delta_and_ratio() {
    let cpp = parse_cpp_log(CPP_LOG).expect("cpp log parses");
    let rust = parse_rust_history(RUST_HISTORY).expect("history parses");
    let report = CompareReport::compare(&cpp, &rust);

    assert_eq!(report.schema, 1);
    assert_eq!(report.runner.as_deref(), Some("linux-x86_64"));

    let put = report
      .metrics
      .iter()
      .find(|row| row.metric == "put_kv_s")
      .expect("put row");
    assert_eq!(put.baseline, Some(120_000.0));
    assert_eq!(put.current, Some(108_000.0));
    assert_eq!(put.delta_pct, Some(-10.0));
    assert!((put.ratio.expect("ratio") - 0.9).abs() < 1e-12);
    assert!(put.ratio_gate);
    assert!(!put.lower_is_better);
    assert_eq!(put.verdict, Verdict::Regressed);

    // 延迟方向相反：p99 变大是 regressed，且不进入吞吐比值门槛。
    let p99 = report
      .metrics
      .iter()
      .find(|row| row.metric == "put_p99_us")
      .expect("p99 row");
    assert!(p99.lower_is_better);
    assert!(!p99.ratio_gate);
    assert_eq!(p99.verdict, Verdict::Regressed);

    // 读 p99 变小则是 improved。
    let get_p99 = report
      .metrics
      .iter()
      .find(|row| row.metric == "get_p99_us")
      .expect("get p99 row");
    assert_eq!(get_p99.delta_pct, Some(-4.0));
    assert_eq!(get_p99.verdict, Verdict::Improved);

    // 只有 C++ 侧有的指标保留 baseline，current 为 None。
    let avg = report
      .metrics
      .iter()
      .find(|row| row.metric == "put_avg_us")
      .expect("avg row");
    assert_eq!(avg.baseline, Some(83.0));
    assert_eq!(avg.current, None);
    assert_eq!(avg.verdict, Verdict::Missing);
    assert!(report.warnings.iter().any(|w| w.contains("put_avg_us")));

    // 只有 Rust 侧有的指标。
    let rss = report
      .metrics
      .iter()
      .find(|row| row.metric == "rss_peak_kb")
      .expect("rss row");
    assert_eq!(rss.current, Some(204_800.0));
    assert!(!rss.ratio_gate);
    assert!(report.warnings.iter().any(|w| w.contains("rss_peak_kb")));

    // 固定顺序：put_* 在 get_* 之前。
    let index = |name: &str| {
      report
        .metrics
        .iter()
        .position(|row| row.metric == name)
        .expect("metric present")
    };
    assert!(index("put_kv_s") < index("get_kv_s"));
    assert!(index("get_kv_s") < index("rss_peak_kb"));
  }

  #[test]
  fn markdown_contains_table_rows_and_notes() {
    let cpp = parse_cpp_log(CPP_LOG).expect("cpp log parses");
    let rust = parse_rust_history(RUST_HISTORY).expect("history parses");
    let markdown = CompareReport::compare(&cpp, &rust).to_markdown();

    assert!(markdown.starts_with("# C++ simple_bench vs Rust fkv_bench\n"));
    assert!(markdown.contains("| metric | baseline (cpp) | current (rust) | delta% |"));
    assert!(
      markdown.contains("| put_kv_s | 120000.00 | 108000.00 | -10.0% | 0.9000 | 是 | regressed |")
    );
    assert!(
      markdown.contains("| put_p99_us | 140.00 | 150.50 | +7.5% | 1.0750 | 否 | regressed |")
    );
    assert!(markdown.contains("- runner: linux-x86_64"));
    assert!(markdown.contains("- workload: workload=write-read rows=1000000"));
    assert!(markdown.contains("## 备注"));
    assert!(markdown.ends_with('\n'));
  }

  #[test]
  fn json_round_trips_with_expected_shape() {
    let cpp = parse_cpp_log(CPP_LOG).expect("cpp log parses");
    let rust = parse_rust_history(RUST_HISTORY).expect("history parses");
    let report = CompareReport::compare(&cpp, &rust);
    let text = report.to_json_pretty().expect("json serializes");
    let value: Value = serde_json::from_str(&text).expect("json parses");

    assert_eq!(value["schema"], json!(1));
    assert_eq!(value["baseline_impl"], json!("cpp"));
    assert_eq!(value["current_impl"], json!("rust"));
    assert_eq!(value["runner"], json!("linux-x86_64"));
    assert_eq!(value["workload"]["kv_size"], json!(128));
    assert_eq!(value["cpp"]["put_kv_s"], json!(120_000.0));
    assert_eq!(value["rust"]["get_kv_s"], json!(270_000.0));

    let rows = value["metrics"].as_array().expect("metrics array");
    assert_eq!(rows.len(), report.metrics.len());
    let put = rows
      .iter()
      .find(|row| row["metric"] == json!("put_kv_s"))
      .expect("put row");
    assert_eq!(put["baseline"], json!(120_000.0));
    assert_eq!(put["current"], json!(108_000.0));
    assert_eq!(put["delta_pct"], json!(-10.0));
    assert_eq!(put["ratio"], json!(0.9));
    assert_eq!(put["ratio_gate"], json!(true));
    assert_eq!(put["verdict"], json!("regressed"));
    assert_eq!(report.to_json_value().expect("value"), value);
  }

  #[test]
  fn report_without_runner_or_workload_warns() {
    let cpp = Metrics::from_iter([("get_kv_s".to_owned(), 100.0)]);
    let rust = RustHistory {
      runner: None,
      workload: BTreeMap::new(),
      metrics: Metrics::from_iter([("get_kv_s".to_owned(), 90.0)]),
    };
    let report = CompareReport::compare(&cpp, &rust);
    assert_eq!(report.metrics.len(), 1);
    assert!(report.warnings.iter().any(|w| w.contains("runner")));
    assert!(report.warnings.iter().any(|w| w.contains("workload")));
    assert!(report.to_markdown().contains("- runner: 未知"));
  }

  #[test]
  fn direction_and_ratio_gate_helpers() {
    assert!(lower_is_better("rss_peak_kb"));
    assert!(lower_is_better("put_p99_us"));
    assert!(lower_is_better("get_avg_us"));
    assert!(lower_is_better("latency_p50_us"));
    assert!(!lower_is_better("put_kv_s"));
    assert!(!lower_is_better("put_avg_us_extra"));
    assert!(!ratio_gate_metric("put_p99_us"));
    assert!(!ratio_gate_metric("rss_peak_kb"));
    assert!(ratio_gate_metric("put_kv_s"));
  }
}
