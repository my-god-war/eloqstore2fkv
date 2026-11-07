//! oracle 语料读取辅助。
//!
//! 被各 oracle 测试以 `mod common;` 引入；每个测试只用到其中一部分函数。
//!
//! 默认本地缺语料时打印提示并跳过；CI 通过
//! `FKV_REQUIRE_ORACLE_FIXTURES=1` 把缺语料升级为 panic，避免「测试没跑」
//! 被当成「通过」（gap-08）。

#![allow(dead_code)]

use std::{
  env, fs,
  path::{Path, PathBuf},
};

/// `fixtures/` 根目录（相对本 crate 定位到仓库根）。
pub fn fixtures_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures")
}

/// 缺语料：强制模式下 panic，否则打印一行提示后由调用方跳过。
fn warn_or_panic(path: &Path, err: &std::io::Error) {
  let message = format!(
    "oracle fixture missing: {} ({err}); 设置 FKV_REQUIRE_ORACLE_FIXTURES=1 可在 CI 强制要求语料",
    path.display()
  );
  if env::var("FKV_REQUIRE_ORACLE_FIXTURES").as_deref() == Ok("1") {
    panic!("{message}");
  }
  eprintln!("skip: {message}");
}

/// 读取 `fixtures/<rel>`；缺文件时按 gap-08 的约定跳过或 panic。
pub fn read_fixture(rel: impl AsRef<Path>) -> Option<Vec<u8>> {
  let path = fixtures_root().join(rel);
  match fs::read(&path) {
    Ok(bytes) => Some(bytes),
    Err(err) => {
      warn_or_panic(&path, &err);
      None
    }
  }
}

/// 读取 `fixtures/cpp/<kind>` 下的 case 目录（按名称排序）。
///
/// 目录不存在或为空时：默认 `None`（调用方跳过），强制模式下 panic。
pub fn case_dirs(kind: &str) -> Option<Vec<PathBuf>> {
  let root = fixtures_root().join("cpp").join(kind);
  let entries = match fs::read_dir(&root) {
    Ok(entries) => entries,
    Err(err) => {
      warn_or_panic(&root, &err);
      return None;
    }
  };
  let mut case_li: Vec<PathBuf> = entries
    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
    .filter(|path| path.is_dir())
    .collect();
  case_li.sort();
  if case_li.is_empty() {
    let err = std::io::Error::new(std::io::ErrorKind::NotFound, "no case directories");
    warn_or_panic(&root, &err);
    return None;
  }
  Some(case_li)
}
