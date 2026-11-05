//! `baseline::write` 保留说明块的行为。

use std::{
  env::temp_dir,
  fs::{create_dir_all, read_to_string, remove_dir_all, write},
  path::PathBuf,
  process::id,
};

use fkv_ledger::baseline;

#[test]
fn write_preserves_note_block_and_level() {
  let dir: PathBuf = temp_dir().join(format!("fkv_baseline_write_{}", id()));
  create_dir_all(&dir).unwrap();
  let file = dir.join("COMPAT_BASELINE");
  write(
    &file,
    "eloqstore main old 2026-01-01\n兼容等级：L1\n\n范围与排除项：\n- note a\n- note b\n",
  )
  .unwrap();

  baseline::write(&file, "main", "abc", "2026-11-04", "L1-partial").unwrap();

  let text = read_to_string(&file).unwrap();
  assert!(text.contains("兼容等级：L1-partial"));
  assert!(text.contains("范围与排除项：\n- note a\n- note b"));
  assert!(!text.contains("兼容等级：L1\n"));

  let parsed = baseline::read(&file).unwrap().unwrap();
  assert_eq!(parsed.commit, "abc");
  assert_eq!(parsed.level, "L1-partial");
  let _ = remove_dir_all(&dir);
}
