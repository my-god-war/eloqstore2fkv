//! C++ oracle 生成的 `mapping` 字节 → Rust `MappingTbl` 解码断言。
//!
//! payload 布局：`page_count(varint32) | page_count × encoded_value(varint64)`。
//! 主体由 eloqstore `MappingSnapshot::Serialize` 产出（MappingTbl 按 PageId
//! 升序写 varint64），Rust 侧用 `fkv_mapping::MappingTbl::deserialize` 读回。

mod common;

use fkv_codec::get_varint32;
use fkv_mapping::MappingTbl;
use fkv_types::{INVALID_VALUE, encode_file_page_id, encode_page_id};

/// 与 `cpp/oracle` 的 `MappingSpecOf` 一一对应。
fn expected(case: &str) -> Vec<u64> {
  match case {
    "empty" => Vec::new(),
    "single" => vec![encode_file_page_id(42)],
    "multi" => vec![
      encode_file_page_id(1),
      INVALID_VALUE,
      encode_file_page_id(5),
      encode_page_id(0),
      encode_file_page_id(9),
      INVALID_VALUE,
      encode_file_page_id(257),
      encode_file_page_id(65536),
    ],
    other => panic!("unexpected cpp mapping case `{other}`"),
  }
}

#[test]
fn cpp_oracle_mapping_tables_decode_with_rust() {
  let Some(dir_li) = common::case_dirs("mapping") else {
    return;
  };
  assert_eq!(dir_li.len(), 3, "mapping 矩阵应为 empty/single/multi");
  for dir in dir_li {
    let case = dir.file_name().unwrap().to_string_lossy().into_owned();
    let Some(bytes) = common::read_fixture(format!("cpp/mapping/{case}/payload.bin")) else {
      return;
    };
    // 空映射也有 page_count 前缀，因此非空。
    assert!(!bytes.is_empty(), "case {case}");
    let (count, used) = get_varint32(&bytes).unwrap();
    assert_eq!(count as usize, expected(&case).len(), "case {case}");

    let tbl = MappingTbl::deserialize(&bytes[used..]).unwrap();
    assert_eq!(tbl.len(), count as usize, "case {case}");
    let got: Vec<u64> = tbl.iter_value_li().collect();
    assert_eq!(got, expected(&case), "case {case}");
  }
}
