//! C++ oracle 生成的 `store` 目录 / 文件清单 → Rust 解码断言。
//!
//! payload 布局：
//! `magic("FST1") | file_count(varint32) |
//!  file_count × [name_len(varint32) | name | data_len(varint32) | data]`
//! 文件按 name 字节序升序。清单里的 data 页 / manifest 必须仍能被 Rust
//! format 层 parser 读回，保证目录布局不是「只有名字对」。

mod common;

use fkv_codec::get_varint32;
use fkv_manifest::{parse_header, validate_record};
use fkv_page::{PageType, page_type, validate_checksum};

const MAGIC: &[u8; 4] = b"FST1";

fn repeat_to(pattern: &[u8], target: usize) -> Vec<u8> {
  let mut out = Vec::with_capacity(target);
  while out.len() < target {
    out.extend_from_slice(pattern);
  }
  out.truncate(target);
  out
}

fn expected_names(case: &str) -> Vec<&'static str> {
  match case {
    "empty" => Vec::new(),
    "single" => vec!["data_0_0"],
    "multi" => vec![
      "data_0_0",
      "data_1_0",
      "manifest_0",
      "manifest_0_1730000000",
      "segment_0_main_0",
    ],
    other => panic!("unexpected cpp store case `{other}`"),
  }
}

fn parse_files(payload: &[u8]) -> Vec<(String, Vec<u8>)> {
  assert!(payload.len() >= MAGIC.len());
  assert_eq!(&payload[..MAGIC.len()], MAGIC, "store magic");
  let mut pos = MAGIC.len();
  let (count, used) = get_varint32(&payload[pos..]).unwrap();
  pos += used;
  let mut file_li = Vec::with_capacity(count as usize);
  for _ in 0..count {
    let (name_len, used) = get_varint32(&payload[pos..]).unwrap();
    pos += used;
    let name = std::str::from_utf8(&payload[pos..pos + name_len as usize])
      .unwrap()
      .to_owned();
    pos += name_len as usize;
    let (data_len, used) = get_varint32(&payload[pos..]).unwrap();
    pos += used;
    let data = payload[pos..pos + data_len as usize].to_vec();
    pos += data_len as usize;
    file_li.push((name, data));
  }
  assert_eq!(pos, payload.len(), "store 不应有尾部多余字节");
  file_li
}

#[test]
fn cpp_oracle_store_listing_decodes_with_rust() {
  let Some(dir_li) = common::case_dirs("store") else {
    return;
  };
  assert_eq!(dir_li.len(), 3, "store 矩阵应为 empty/single/multi");
  let segment = repeat_to(b"fkv-segment|main|0|", 512);
  for dir in dir_li {
    let case = dir.file_name().unwrap().to_string_lossy().into_owned();
    let Some(bytes) = common::read_fixture(format!("cpp/store/{case}/payload.bin")) else {
      return;
    };
    assert!(!bytes.is_empty(), "case {case}");
    let file_li = parse_files(&bytes);

    let want_names = expected_names(&case);
    let got_names: Vec<&str> = file_li.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(got_names, want_names, "case {case} 文件清单");
    // 文件名按字节序升序（oracle 的目录列举口径）。
    for pair in got_names.windows(2) {
      assert!(pair[0] < pair[1], "case {case} 文件名未按序");
    }

    for (name, data) in &file_li {
      if name.starts_with("data_") {
        assert_eq!(
          page_type(data).unwrap(),
          PageType::Data,
          "case {case} {name}"
        );
        assert!(validate_checksum(data).unwrap(), "case {case} {name}");
        assert_eq!(data.len(), 4096, "case {case} {name}");
      } else if name.starts_with("manifest_") {
        let header = parse_header(data).unwrap();
        assert!(
          validate_record(data).unwrap(),
          "case {case} {name} manifest checksum"
        );
        assert_eq!(
          data.len(),
          fkv_manifest::HEADER_BYTES + header.payload_len as usize,
          "case {case} {name} record 长度"
        );
      } else if name.starts_with("segment_") {
        assert_eq!(data, &segment, "case {case} {name}");
      } else {
        panic!("case {case} 未知文件 {name}");
      }
    }

    if case == "multi" {
      // 两个真实 manifest 布局：branch（无 segment 段）与 archive（有 segment 段）。
      assert!(!file_li[2].1.is_empty() && !file_li[3].1.is_empty());
      assert_ne!(file_li[2].1, file_li[3].1);
    }
  }
}
