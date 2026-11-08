//! corpus 语料 / 解码断言 / 反向 fixture 测试。
//!
//! 已入库语料 `fixtures/cpp/<kind>/<case>/` 由 C++ oracle 生成；这里用 Rust
//! 解码器断言同一语料（C++ 写 → Rust 读），并验证 Rust builder 重建字节与
//! C++ 产物逐字节一致。

use std::{
  env::temp_dir,
  fs,
  fs::{create_dir_all, remove_dir_all},
  path::PathBuf,
  process::id,
};

use fkv_ledger::{corpus, fixture};

/// 已入库 C++ 语料根目录。
fn cpp_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cpp")
}

/// 全部 oracle 支持的 kind/case（带真实 payload 的目录）。
fn committed_case_li() -> Vec<(String, String, PathBuf)> {
  let root = cpp_root();
  let mut case_li = Vec::new();
  let Ok(kind_li) = fs::read_dir(&root) else {
    eprintln!("skip corpus: 语料未生成（{}）", root.display());
    return case_li;
  };
  let mut kind_path_li: Vec<PathBuf> = kind_li
    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
    .filter(|path| {
      path.is_dir()
        && fixture::oracle_supported(path.file_name().unwrap().to_string_lossy().as_ref())
    })
    .collect();
  kind_path_li.sort();
  for kind_dir in kind_path_li {
    let kind_name = kind_dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut case_path_li: Vec<PathBuf> = fs::read_dir(&kind_dir)
      .expect("kind 目录可读")
      .filter_map(|entry| entry.ok().map(|entry| entry.path()))
      .filter(|path| path.join("payload.bin").is_file())
      .collect();
    case_path_li.sort();
    for case_dir in case_path_li {
      let case = case_dir.file_name().unwrap().to_string_lossy().into_owned();
      case_li.push((kind_name.clone(), case, case_dir));
    }
  }
  case_li
}

/// C++ 写的字节，Rust 解码器逐 case 断言（六类格式交叉读）。
#[test]
fn cpp_bytes_decode_with_rust_corpus() {
  let case_li = committed_case_li();
  assert!(!case_li.is_empty(), "fixtures/cpp 语料为空");
  for (kind, case, dir) in case_li {
    let payload = fs::read(dir.join("payload.bin")).unwrap();
    let dict = fs::read(dir.join("dict.bin")).ok();
    corpus::decode_case(&kind, &case, &payload, dict.as_deref())
      .unwrap_or_else(|diff| panic!("{kind}/{case} offset {}: {}", diff.offset, diff.reason));
  }
}

/// 篡改 payload 一个字节：解码失败且给出首个差异 offset。
#[test]
fn tampered_payload_reports_first_diff_offset() {
  let (kind, case, dir) = ("page", "single", cpp_root().join("page/single"));
  let mut payload = fs::read(dir.join("payload.bin")).unwrap();
  // 300 落在条目区之后的 padding：语义断言看不到，字节比对必须指出 offset。
  payload[300] ^= 0xff;
  let diff = corpus::decode_case(kind, case, &payload, None).expect_err("篡改应失败");
  assert_eq!(diff.offset, 300, "reason {}", diff.reason);
}

/// Rust 写 → 语料自检：反向 fixture 生成后能被同一解码器读回。
#[test]
fn rust_written_payload_round_trips() {
  let temp = tempdir("corpus_roundtrip");
  for (kind, case_li) in fixture::MATRIX {
    if !fixture::oracle_supported(kind) {
      continue;
    }
    for case in case_li {
      let dir =
        corpus::generate_reverse(&temp, kind, case, "baseline-test", fixture::SCHEMA_VERSION)
          .unwrap_or_else(|err| panic!("{kind}/{case} 生成失败：{err}"));
      let payload = fs::read(dir.join("payload.bin")).unwrap();
      let dict = fs::read(dir.join("dict.bin")).ok();
      corpus::decode_case(kind, case, &payload, dict.as_deref())
        .unwrap_or_else(|diff| panic!("{kind}/{case} offset {}: {}", diff.offset, diff.reason));
    }
  }
  cleanup(&temp);
}

/// generate_reverse 的 meta / 目录布局与 fixture 契约一致。
#[test]
fn reverse_fixture_writes_meta_and_dict() {
  let temp = tempdir("corpus_meta");
  let dir = corpus::generate_reverse(
    &temp,
    "compress",
    "dict",
    "baseline-test",
    fixture::SCHEMA_VERSION,
  )
  .unwrap();
  assert!(dir.join("payload.bin").is_file());
  assert!(dir.join("dict.bin").is_file());
  let meta = fs::read_to_string(dir.join("meta.txt")).unwrap();
  assert!(meta.contains(&format!("schema={}", fixture::SCHEMA_VERSION)));
  assert!(meta.contains("kind=compress"));
  assert!(meta.contains("case=dict"));
  assert!(meta.contains("status=pass"));
  let meta_dir = corpus::generate_reverse(
    &temp,
    "page",
    "single",
    "baseline-test",
    fixture::SCHEMA_VERSION,
  )
  .unwrap();
  assert!(
    !meta_dir.join("dict.bin").exists(),
    "非 dict case 不应有 dict.bin"
  );
  cleanup(&temp);
}

fn tempdir(tag: &str) -> PathBuf {
  let dir = temp_dir().join(format!("fkv_corpus_{tag}_{}", id()));
  let _ = remove_dir_all(&dir);
  create_dir_all(&dir).unwrap();
  dir
}

fn cleanup(dir: &PathBuf) {
  let _ = remove_dir_all(dir);
}
