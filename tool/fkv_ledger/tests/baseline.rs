//! `baseline::write` 保留说明块的行为。

use std::{
  env::temp_dir,
  fs::{create_dir_all, read_to_string, remove_dir_all, write},
  path::{Path, PathBuf},
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

/// l2-04 口径登记：仓库根 `COMPAT_BASELINE` 必须显式写明「全局操作不计入 shard
/// 指标」。
///
/// C++ `RequestType` 的 `global_archive` / `global_reopen` /
/// `global_list_archive_tags` / `global_create_branch` 由 `EloqStore` 在 store 层
/// 处理，从不进入 `Shard::OnTaskFinished` 的指标埋点；fkv 据此不注册这些标签。
/// 该条目给出对账时的解释方式（两侧 `global_*` 都是零序列），删掉它会让跨实现
/// 按 `request_type` 对账重新出现无法解释的差异。
#[test]
fn baseline_registers_global_ops_not_in_shard_metrics() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let file = root.join("COMPAT_BASELINE");
  let text = read_to_string(&file).unwrap();
  assert!(
    text.contains("全局操作不计入 shard 指标"),
    "COMPAT_BASELINE 缺少全局操作指标口径登记: {}",
    file.display()
  );
}
