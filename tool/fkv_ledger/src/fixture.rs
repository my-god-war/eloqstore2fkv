//! C++ oracle fixture 生成 / 校验脚手架。
//!
//! 只定义目录与清单格式；真实字节由 C++ oracle 产生。没有 oracle 时只写
//! `status=skipped` 的清单，先把流水线打通，oracle 就绪后直接复用。
//!
//! 目录约定（`fixtures/README.md` 同步描述）：
//! ```text
//! <out>/<kind>/<case>/meta.txt      清单（key=value，逐行）
//! <out>/<kind>/<case>/payload.bin   字节由 oracle 生成；skipped 时为空
//! ```

use std::{
  fs,
  io::Write,
  path::{Path, PathBuf},
  process::{Command, ExitStatus},
};

use crate::{
  corpus::decode_case,
  error::{LedgerError, Result},
};

/// 清单格式版本，写入 `meta.txt` 的 `schema` 字段；不兼容演进时递增。
pub const SCHEMA_VERSION: u32 = 1;

/// case 语料矩阵：`kind` -> 规范 `case` 名。
///
/// `fixture --matrix` 按此全量生成；`--case` 仍可传自由名做临时试验，
/// 但验收覆盖以本表为准。
pub const MATRIX: [(&str, &[&str]); 7] = [
  (
    "page",
    &[
      "empty", "single", "multi", "restart", "ttl", "large", "overflow",
    ],
  ),
  ("index", &["leaf", "branch"]),
  // overflow 页自身格式：data entry + 单组连号 overflow 页。
  ("overflow", &["chain"]),
  ("manifest", &["empty", "branch", "checksum", "archive"]),
  ("compress", &["raw", "dict"]),
  ("mapping", &["empty", "single", "multi"]),
  ("store", &["empty", "single", "multi"]),
];

const META_NAME: &str = "meta.txt";
const PAYLOAD_NAME: &str = "payload.bin";

/// oracle 能产真实字节的 kind（与 [`MATRIX`] 一一对应）。
pub const ORACLE_KINDS: [&str; 7] = [
  "page", "index", "overflow", "manifest", "compress", "mapping", "store",
];

/// 该 kind 是否有 C++ oracle 字节实现。
pub fn oracle_supported(kind: &str) -> bool {
  ORACLE_KINDS.contains(&kind)
}

/// 全部 fixture 类别（即矩阵的 key）。
pub fn kind_li() -> impl Iterator<Item = &'static str> {
  MATRIX.iter().map(|(kind, _)| *kind)
}

/// 某类别的规范 case 列表；未知类别返回 `None`。
pub fn cases_of(kind: &str) -> Option<&'static [&'static str]> {
  MATRIX
    .iter()
    .find(|(name, _)| *name == kind)
    .map(|(_, cases)| *cases)
}

/// 生成一个 fixture case。返回 case 目录。
pub fn generate(
  out: &Path,
  kind: &str,
  case: &str,
  oracle: Option<&Path>,
  baseline: &str,
) -> Result<PathBuf> {
  if cases_of(kind).is_none() {
    return Err(LedgerError::Invalid(format!(
      "unknown fixture kind `{kind}`, expected one of {:?}",
      kind_li().collect::<Vec<_>>()
    )));
  }
  if case.is_empty() || case.contains('/') || case.contains('\\') {
    return Err(LedgerError::Invalid(format!("invalid case name `{case}`")));
  }
  let dir = out.join(kind).join(case);
  fs::create_dir_all(&dir)?;
  // 上一轮 oracle 留下的 payload 不能冒充本轮结果。
  let payload_path = dir.join(PAYLOAD_NAME);
  if payload_path.exists() {
    fs::remove_file(&payload_path)?;
  }
  let (status, payload_len, oracle_used) = match oracle {
    Some(oracle) => {
      let exit = run_oracle(
        oracle,
        &[
          "fixture",
          "--kind",
          kind,
          "--case",
          case,
          "--out",
          dir
            .to_str()
            .ok_or_else(|| LedgerError::Invalid("non-utf8 out dir".to_owned()))?,
        ],
      )?;
      // 退出码 0 但没写 / 写了空 payload 也算生成失败：verify 会消费这些字节，
      // 空文件不能让「没实现」被当成「通过」。
      let wrote_payload = fs::metadata(&payload_path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false);
      if exit.success() && !wrote_payload {
        eprintln!(
          "fixture {kind}/{case}: oracle 退出码 0 但未写出非空 {}",
          payload_path.display()
        );
      }
      (
        if exit.success() && wrote_payload {
          "pass"
        } else {
          "fail"
        },
        None,
        Some(oracle),
      )
    }
    None => ("skipped", Some(0usize), None),
  };
  if let Some(len) = payload_len {
    let mut file = fs::File::create(&payload_path)?;
    file.write_all(&vec![0u8; len])?;
  }
  let meta = format!(
    "schema={SCHEMA_VERSION}\nkind={kind}\ncase={case}\nbaseline={baseline}\nstatus={status}\noracle={}\npayload={PAYLOAD_NAME}\n",
    oracle_used.map_or("-", |p| p.to_str().unwrap_or("-")),
  );
  fs::write(dir.join(META_NAME), meta)?;
  Ok(dir)
}

/// 按 [`MATRIX`] 生成全部规范 case，返回生成的 case 目录列表。
pub fn generate_matrix(out: &Path, oracle: Option<&Path>, baseline: &str) -> Result<Vec<PathBuf>> {
  MATRIX
    .iter()
    .flat_map(|(kind, case_li)| case_li.iter().map(move |case| (*kind, *case)))
    .map(|(kind, case)| generate(out, kind, case, oracle, baseline))
    .collect()
}

/// 读取清单的 `key=value` 字段。
pub fn read_meta(dir: &Path) -> Result<Vec<(String, String)>> {
  let text = fs::read_to_string(dir.join(META_NAME))?;
  Ok(
    text
      .lines()
      .filter_map(|line| {
        let (key, value) = line.split_once('=')?;
        Some((key.trim().to_owned(), value.trim().to_owned()))
      })
      .collect(),
  )
}

fn meta_get<'a>(meta: &'a [(String, String)], key: &str) -> Option<&'a str> {
  meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// `compress/dict` 的字典文件；其余 kind 返回 `None`。
fn read_dict(case_dir: &Path) -> Option<Vec<u8>> {
  fs::read(case_dir.join("dict.bin")).ok()
}

fn run_oracle(oracle: &Path, args: &[&str]) -> Result<ExitStatus> {
  Command::new(oracle)
    .args(args)
    .status()
    .map_err(|err| LedgerError::Invalid(format!("run oracle {}: {err}", oracle.display())))
}

#[derive(Debug, Default)]
pub struct VerifyReport {
  pub pass: usize,
  pub fail: usize,
  pub skipped: usize,
  pub case_li: Vec<(String, String, String)>,
}

impl VerifyReport {
  pub fn total(&self) -> usize {
    self.pass + self.fail + self.skipped
  }

  /// 是否有失败项。
  pub fn failed(&self) -> bool {
    self.fail > 0
  }

  pub fn to_markdown(&self) -> String {
    let mut out = String::new();
    out.push_str("# fixture verify 报告\n\n");
    out.push_str(&format!(
      "总计 {}：pass {} / fail {} / skipped {}\n\n",
      self.total(),
      self.pass,
      self.fail,
      self.skipped
    ));
    out.push_str("| case | status | 说明 |\n|---|---|---|\n");
    for (case, status, note) in &self.case_li {
      out.push_str(&format!("| {case} | {status} | {note} |\n"));
    }
    out
  }
}

/// 遍历 fixtures 目录并逐 case 校验；有 oracle 时调用 `<oracle> verify --case-dir <dir>`。
pub fn verify(fixtures: &Path, oracle: Option<&Path>) -> Result<VerifyReport> {
  if !fixtures.is_dir() {
    return Err(LedgerError::Invalid(format!(
      "fixtures dir not found: {}",
      fixtures.display()
    )));
  }
  let mut report = VerifyReport::default();
  let mut kind_li: Vec<PathBuf> = fs::read_dir(fixtures)?
    .filter_map(|entry| entry.ok().map(|e| e.path()))
    .filter(|path| path.is_dir())
    .collect();
  kind_li.sort();
  for kind_dir in kind_li {
    let kind = kind_dir.file_name().unwrap_or_default().to_string_lossy();
    let mut case_li: Vec<PathBuf> = fs::read_dir(&kind_dir)?
      .filter_map(|entry| entry.ok().map(|e| e.path()))
      .filter(|path| path.is_dir())
      .collect();
    case_li.sort();
    for case_dir in case_li {
      let meta = read_meta(&case_dir)?;
      let case = meta_get(&meta, "case").unwrap_or("-").to_owned();
      let label = format!("{kind}/{case}");
      // 旧格式 fixture 不能静默通过：schema 缺失或不符一律判失败。
      if meta_get(&meta, "schema").and_then(|v| v.parse::<u32>().ok()) != Some(SCHEMA_VERSION) {
        report.fail += 1;
        report.case_li.push((
          label,
          "fail".to_owned(),
          format!(
            "schema={} 期望 {SCHEMA_VERSION}",
            meta_get(&meta, "schema").unwrap_or("缺失")
          ),
        ));
        continue;
      }
      match meta_get(&meta, "status") {
        // 生成时未用 oracle：即便此刻传了 oracle 也不能假装已验证。
        Some("skipped") => {
          report.skipped += 1;
          report.case_li.push((
            label,
            "skipped".to_owned(),
            if !oracle_supported(kind.as_ref()) {
              "该 kind 无 oracle 字节实现".to_owned()
            } else if oracle.is_some() {
              "生成时无 oracle，需重新生成".to_owned()
            } else {
              "需要 oracle".to_owned()
            },
          ));
        }
        // oracle 生成阶段就失败了，不能因为这次没传 oracle 而降级成 skipped。
        Some("fail") => {
          report.fail += 1;
          report
            .case_li
            .push((label, "fail".to_owned(), "生成阶段失败".to_owned()));
        }
        _ => {
          // Rust 侧直接解码 payload（不信任 oracle 退出码）：语义断言 + 与
          // Rust 重建字节的逐字节比对，失败给出首个差异 offset。
          let payload = fs::read(case_dir.join(PAYLOAD_NAME))?;
          if payload.is_empty() {
            report.fail += 1;
            report
              .case_li
              .push((label, "fail".to_owned(), "payload.bin 为空".to_owned()));
            continue;
          }
          let dict = read_dict(&case_dir);
          let decode_note =
            decode_case(kind.as_ref(), case.as_str(), &payload, dict.as_deref()).err();
          if let Some(diff) = decode_note {
            report.fail += 1;
            report.case_li.push((
              label,
              "fail".to_owned(),
              format!("Rust 解码 offset {}: {}", diff.offset, diff.reason),
            ));
            continue;
          }
          match oracle {
            Some(oracle) => {
              let status = run_oracle(
                oracle,
                &[
                  "verify",
                  "--case-dir",
                  case_dir
                    .to_str()
                    .ok_or_else(|| LedgerError::Invalid("non-utf8 case dir".to_owned()))?,
                  // 同一个 case 目录名可能对应多类字节（页/压缩/manifest），
                  // oracle 依靠 kind+case 选解析器。
                  "--kind",
                  kind.as_ref(),
                  "--case",
                  case.as_str(),
                ],
              )?;
              if status.success() {
                report.pass += 1;
                report
                  .case_li
                  .push((label, "pass".to_owned(), "-".to_owned()));
              } else {
                report.fail += 1;
                report.case_li.push((
                  label,
                  "fail".to_owned(),
                  format!("exit {}", status.code().unwrap_or(-1)),
                ));
              }
            }
            None => {
              report.skipped += 1;
              report.case_li.push((
                label,
                "skipped".to_owned(),
                if oracle_supported(kind.as_ref()) {
                  "Rust 解码通过；C++ 交叉读需 oracle".to_owned()
                } else {
                  "未提供 oracle".to_owned()
                },
              ));
            }
          }
        }
      }
    }
  }
  Ok(report)
}

/// 读取仓库根的 `COMPAT_BASELINE`（缺省返回空串）。
pub fn baseline_text(path: &Path) -> String {
  // 基线文件可能多行；压成单行，保证 meta.txt 的 key=value 逐行格式不被破坏。
  fs::read_to_string(path)
    .map(|text| {
      text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
    })
    .unwrap_or_default()
}
