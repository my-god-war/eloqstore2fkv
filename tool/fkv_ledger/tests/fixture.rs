//! fixture / verify 脚手架测试。

use std::{
  env::temp_dir,
  fs::{create_dir_all, metadata, read, read_to_string, remove_dir_all, set_permissions, write},
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::id,
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};

use fkv_ledger::{error, fixture};

fn temp_root(tag: &str) -> PathBuf {
  static NEXT_ID: AtomicU64 = AtomicU64::new(0);
  let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
  let root = temp_dir().join(format!(
    "fkv_fixture_{tag}_{}_{}_{}",
    id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos(),
    seq,
  ));
  create_dir_all(&root).unwrap();
  root
}

#[test]
fn generate_without_oracle_writes_skipped_meta() {
  let root = temp_root("skipped");
  let dir = fixture::generate(&root, "page", "smoke", None, "baseline-x").unwrap();
  assert!(dir.join("meta.txt").exists());
  assert!(dir.join("payload.bin").exists());

  let meta = fixture::read_meta(&dir).unwrap();
  let get = |key: &str| meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
  assert_eq!(get("status").as_deref(), Some("skipped"));
  assert_eq!(get("kind").as_deref(), Some("page"));
  assert_eq!(get("baseline").as_deref(), Some("baseline-x"));
  let _ = remove_dir_all(&root);
}

#[test]
fn generate_rejects_unknown_kind() {
  let root = temp_root("badkind");
  let err = fixture::generate(&root, "nope", "smoke", None, "").unwrap_err();
  assert!(matches!(err, error::LedgerError::Invalid(_)));
  let _ = remove_dir_all(&root);
}

#[test]
fn verify_counts_skipped_cases_without_oracle() {
  let root = temp_root("verify");
  fixture::generate(&root, "page", "a", None, "b").unwrap();
  fixture::generate(&root, "manifest", "b", None, "b").unwrap();
  let report = fixture::verify(&root, None).unwrap();
  assert_eq!(report.total(), 2);
  assert_eq!(report.skipped, 2);
  assert_eq!(report.pass, 0);
  assert!(!report.failed());
  assert!(report.to_markdown().contains("skipped 2"));
  let _ = remove_dir_all(&root);
}

#[test]
fn verify_errors_on_missing_dir() {
  let missing = temp_dir().join("fkv_fixture_does_not_exist");
  assert!(fixture::verify(&missing, None).is_err());
}

#[test]
fn baseline_text_flattens_multiple_lines() {
  let root = temp_root("baseline");
  let file = root.join("COMPAT_BASELINE");
  write(&file, "line1\nline2\n\n").unwrap();
  assert_eq!(fixture::baseline_text(&file), "line1 | line2");
  let _ = remove_dir_all(&root);
}

#[test]
fn generate_matrix_covers_every_case_with_schema() {
  let root = temp_root("matrix");
  let dir_li = fixture::generate_matrix(&root, None, "baseline-x").unwrap();
  let expect: usize = fixture::MATRIX
    .iter()
    .map(|(_, case_li)| case_li.len())
    .sum();
  assert_eq!(dir_li.len(), expect);
  for dir in &dir_li {
    let meta = fixture::read_meta(dir).unwrap();
    let get = |key: &str| meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("schema"), Some(fixture::SCHEMA_VERSION.to_string()));
    assert_eq!(get("status").as_deref(), Some("skipped"));
  }
  let report = fixture::verify(&root, None).unwrap();
  assert_eq!(report.skipped, expect);
  assert!(!report.failed());
  let _ = remove_dir_all(&root);
}

#[test]
fn verify_fails_on_schema_mismatch() {
  let root = temp_root("schema");
  let dir = fixture::generate(&root, "page", "a", None, "b").unwrap();
  let meta_path = dir.join("meta.txt");
  let text = read_to_string(&meta_path)
    .unwrap()
    .replace(&format!("schema={}", fixture::SCHEMA_VERSION), "schema=0");
  write(&meta_path, text).unwrap();
  let report = fixture::verify(&root, None).unwrap();
  assert_eq!(report.fail, 1);
  assert!(report.failed());
  assert!(report.to_markdown().contains("schema=0"));
  let _ = remove_dir_all(&root);
}

#[test]
fn verify_counts_generated_failure_as_fail() {
  let root = temp_root("genfail");
  let dir = fixture::generate(&root, "page", "a", None, "b").unwrap();
  let meta_path = dir.join("meta.txt");
  let text = read_to_string(&meta_path)
    .unwrap()
    .replace("status=skipped", "status=fail");
  write(&meta_path, text).unwrap();
  let report = fixture::verify(&root, None).unwrap();
  assert_eq!(report.fail, 1);
  assert!(report.failed());
  let _ = remove_dir_all(&root);
}

#[test]
fn oracle_supported_matches_implemented_kinds() {
  for kind in fixture::ORACLE_KINDS {
    assert!(fixture::oracle_supported(kind), "kind {kind}");
  }
  // 矩阵里全部 7 类都有字节实现（含 mapping / store）。
  assert_eq!(fixture::ORACLE_KINDS.len(), fixture::MATRIX.len());
  for (kind, _) in fixture::MATRIX {
    assert!(fixture::oracle_supported(kind), "kind {kind}");
  }
  assert!(!fixture::oracle_supported("nope"));
}

/// 假 oracle：把固定 3 字节写进 `--out`，用于验证调用与 meta 状态，不校验字节。
fn fake_oracle(root: &Path) -> PathBuf {
  let path = root.join("fake_oracle.sh");
  write(
    &path,
    "#!/bin/sh\nout=\"\"\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    --out) out=$2; shift 2;;\n    *) shift;;\n  esac\ndone\nprintf abc > \"$out/payload.bin\"\n",
  )
  .unwrap();
  let mut mode = metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  set_permissions(&path, mode).unwrap();
  path
}

#[test]
fn generate_with_oracle_marks_pass_for_all_kinds() {
  let root = temp_root("oracle");
  let oracle = fake_oracle(&root);
  let dir = fixture::generate(&root, "page", "single", Some(&oracle), "b").unwrap();
  let meta = fixture::read_meta(&dir).unwrap();
  let get = |key: &str| meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
  assert_eq!(get("status").as_deref(), Some("pass"));
  assert_eq!(get("oracle").as_deref(), oracle.to_str());
  assert_eq!(read(dir.join("payload.bin")).unwrap(), b"abc");

  // mapping / store 现在也有 oracle 字节实现：走同一 pass 路径。
  for kind in ["mapping", "store"] {
    let dir = fixture::generate(&root, kind, "single", Some(&oracle), "b").unwrap();
    let meta = fixture::read_meta(&dir).unwrap();
    let get = |key: &str| meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("status").as_deref(), Some("pass"), "kind {kind}");
    assert_eq!(get("oracle").as_deref(), oracle.to_str(), "kind {kind}");
    assert_eq!(
      read(dir.join("payload.bin")).unwrap(),
      b"abc",
      "kind {kind}"
    );
  }
  let _ = remove_dir_all(&root);
}

#[test]
fn oracle_with_empty_payload_is_failure() {
  let root = temp_root("empty_payload");
  let path = root.join("empty_oracle.sh");
  write(
    &path,
    "#!/bin/sh\nout=\"\"\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    --out) out=$2; shift 2;;\n    *) shift;;\n  esac\ndone\n: > \"$out/payload.bin\"\n",
  )
  .unwrap();
  let mut mode = metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  set_permissions(&path, mode).unwrap();
  let dir = fixture::generate(&root, "mapping", "empty", Some(&path), "b").unwrap();
  let meta = fixture::read_meta(&dir).unwrap();
  let status = meta
    .iter()
    .find(|(k, _)| k == "status")
    .map(|(_, v)| v.as_str());
  assert_eq!(status, Some("fail"));
  let _ = remove_dir_all(&root);
}
