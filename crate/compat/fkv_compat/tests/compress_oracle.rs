//! C++ oracle 生成的压缩字节 → Rust 解压断言。
//!
//! 语料由 `cpp/oracle` 的 `compression::CompressRaw` / `DictCompression` 产出：
//! `fixtures/cpp/compress/raw/payload.bin` 与
//! `fixtures/cpp/compress/dict/{payload.bin,dict.bin}`。

mod common;

use fkv_compress::{DictCompression, decompress_raw};

/// 与 `cpp/oracle` 的 `RepeatTo` 同规则：按 pattern 重复到 target 字节后截断。
fn repeat_to(pattern: &[u8], target: usize) -> Vec<u8> {
  let mut out = Vec::with_capacity(target);
  while out.len() < target {
    out.extend_from_slice(pattern);
  }
  out.truncate(target);
  out
}

fn raw_input() -> Vec<u8> {
  repeat_to(b"fkv-raw-compress|0123456789abcdef|", 2048)
}

fn dict_input() -> Vec<u8> {
  repeat_to(b"fkv-dict-compress|branch=main|term=7|", 2048)
}

#[test]
fn cpp_oracle_raw_compression_decompresses_with_rust() {
  let Some(payload) = common::read_fixture("cpp/compress/raw/payload.bin") else {
    return;
  };
  assert!(!payload.is_empty());
  let out = decompress_raw(&payload).unwrap();
  assert_eq!(out, raw_input());
}

#[test]
fn cpp_oracle_dict_compression_decompresses_with_rust() {
  let Some(payload) = common::read_fixture("cpp/compress/dict/payload.bin") else {
    return;
  };
  let Some(dict) = common::read_fixture("cpp/compress/dict/dict.bin") else {
    return;
  };
  assert!(!dict.is_empty());
  // C++ 默认 zstd_compression_level = ZSTD_CLEVEL_DEFAULT(3)；解压只依赖字典字节。
  let mut compression = DictCompression::new(3);
  compression.load_dictionary(dict).unwrap();
  let out = compression.decompress(&payload).unwrap();
  assert_eq!(out, dict_input());
}
