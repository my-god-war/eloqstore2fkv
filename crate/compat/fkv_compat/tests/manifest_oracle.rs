//! C++ oracle 生成的 manifest record → Rust 解析断言。
//!
//! 语料由 `cpp/oracle` 的 `ManifestBuilder` 产出，落在
//! `fixtures/cpp/manifest/<case>/payload.bin`。
//!
//! 两类布局：
//! - snapshot（`branch` / `archive`，对齐 eloqstore `ManifestBuilder::Snapshot`）：
//!   `max_fp_id(varint64) | dict_len(varint32) | dict | mapping_len(4B) |
//!    mapping values(varint64...) | branch_len(4B) | branch metadata |
//!    [max_segment_fp_id(varint64) | seg_len(4B) | segment values(varint64...)]`；
//!   最后一段是可选段：C++ 只在 `segment_mapping != nullptr` 时写。
//! - log record（`empty` / `checksum`）：
//!   `mapping_len(4B) | mapping entries | branch_len(4B) | branch metadata`。

mod common;

use fkv_codec::{get_fixed32, get_varint32, get_varint64};
use fkv_manifest::{
  BranchManifestMetadata, deserialize_branch_metadata, parse_header, payload, validate_record,
};
use fkv_types::encode_file_page_id;

fn read_case(case: &str) -> Option<Vec<u8>> {
  common::read_fixture(format!("cpp/manifest/{case}/payload.bin"))
}

fn take_varint64(body: &[u8], pos: &mut usize) -> u64 {
  let (value, read) = get_varint64(&body[*pos..]).unwrap();
  *pos += read;
  value
}

fn take_varint32(body: &[u8], pos: &mut usize) -> u32 {
  let (value, read) = get_varint32(&body[*pos..]).unwrap();
  *pos += read;
  value
}

fn take_fixed32(body: &[u8], pos: &mut usize) -> u32 {
  let value = get_fixed32(&body[*pos..]).unwrap();
  *pos += 4;
  value
}

fn decode_values(section: &[u8]) -> Vec<u64> {
  let mut pos = 0usize;
  let mut value_li = Vec::new();
  while pos < section.len() {
    value_li.push(take_varint64(section, &mut pos));
  }
  value_li
}

#[derive(Debug)]
struct SnapshotSections {
  max_fp_id: u64,
  dict_bytes: Vec<u8>,
  mapping_value_li: Vec<u64>,
  branch_metadata: BranchManifestMetadata,
  segment: Option<(u64, Vec<u64>)>,
}

/// 按 eloqstore `ManifestBuilder::Snapshot` 的布局逐段解码；必须恰好耗尽 payload。
fn parse_snapshot(record: &[u8]) -> SnapshotSections {
  let body = payload(record).unwrap();
  let mut pos = 0usize;
  let max_fp_id = take_varint64(body, &mut pos);
  let dict_len = take_varint32(body, &mut pos) as usize;
  let dict_bytes = body[pos..pos + dict_len].to_vec();
  pos += dict_len;
  let mapping_len = take_fixed32(body, &mut pos) as usize;
  let mapping_value_li = decode_values(&body[pos..pos + mapping_len]);
  pos += mapping_len;
  let branch_len = take_fixed32(body, &mut pos) as usize;
  let branch_metadata = deserialize_branch_metadata(&body[pos..pos + branch_len]).unwrap();
  pos += branch_len;
  // 可选 segment 段：没有时 payload 在 branch metadata 之后结束。
  let segment = if pos == body.len() {
    None
  } else {
    let max_segment_fp_id = take_varint64(body, &mut pos);
    let seg_len = take_fixed32(body, &mut pos) as usize;
    let value_li = decode_values(&body[pos..pos + seg_len]);
    pos += seg_len;
    Some((max_segment_fp_id, value_li))
  };
  assert_eq!(pos, body.len(), "snapshot payload 不应有尾部多余字节");
  SnapshotSections {
    max_fp_id,
    dict_bytes,
    mapping_value_li,
    branch_metadata,
    segment,
  }
}

#[test]
fn cpp_oracle_empty_manifest_parses_with_rust() {
  let Some(bytes) = read_case("empty") else {
    return;
  };
  assert!(validate_record(&bytes).unwrap());
  let header = parse_header(&bytes).unwrap();
  assert_eq!(header.root, 0);
  assert_eq!(header.ttl_root, u32::MAX);
  assert_eq!(header.payload_len, 0);
  assert!(payload(&bytes).unwrap().is_empty());
}

#[test]
fn cpp_oracle_manifest_log_checksum_parses_with_rust() {
  let Some(bytes) = read_case("checksum") else {
    return;
  };
  assert!(validate_record(&bytes).unwrap());
  let header = parse_header(&bytes).unwrap();
  let body = payload(&bytes).unwrap();
  assert_eq!(body.len(), header.payload_len as usize);
  assert_eq!(header.root, 2);
  assert_eq!(header.ttl_root, u32::MAX);

  // log record 头部是 mapping 段长度。
  let mapping_len = u32::from_le_bytes(body[..4].try_into().unwrap()) as usize;
  let branch_len_at = 4 + mapping_len;
  let branch_len =
    u32::from_le_bytes(body[branch_len_at..branch_len_at + 4].try_into().unwrap()) as usize;
  let branch = &body[branch_len_at + 4..branch_len_at + 4 + branch_len];
  let meta = deserialize_branch_metadata(branch).unwrap();
  // 空 branch metadata：只覆盖 mapping 段 + checksum。
  assert!(meta.branch_name.is_empty());
  assert_eq!(meta.term, 0);
  assert!(meta.file_range_li.is_empty());
  // 2 条 mapping：page 0 -> fp 1、page 1 -> fp 2，各 2 字节 varint。
  assert_eq!(mapping_len, 4);
}

/// `branch` 是真实 branch snapshot：`CreateBranch` 不传 segment mapping，
/// 因此段布局在 branch metadata 后结束（无 segment 段）。
#[test]
fn cpp_oracle_manifest_branch_snapshot_parses_with_rust() {
  let Some(bytes) = read_case("branch") else {
    return;
  };
  assert!(validate_record(&bytes).unwrap());
  let sections = parse_snapshot(&bytes);
  assert_eq!(sections.max_fp_id, 58 << 18);
  assert!(sections.dict_bytes.is_empty());
  assert_eq!(
    sections.mapping_value_li,
    vec![
      encode_file_page_id(5),
      encode_file_page_id(6),
      encode_file_page_id(7),
    ]
  );
  assert_eq!(
    sections.branch_metadata.branch_name.as_ref(),
    "feature-a3f7b2c1"
  );
  assert_eq!(sections.branch_metadata.term, 7);
  assert_eq!(sections.branch_metadata.file_range_li.len(), 2);
  assert_eq!(
    sections.branch_metadata.file_range_li[0]
      .branch_name
      .as_ref(),
    "main"
  );
  assert_eq!(sections.branch_metadata.file_range_li[0].max_file_id, 42);
  assert_eq!(
    sections.branch_metadata.file_range_li[1]
      .branch_name
      .as_ref(),
    "feature-a3f7b2c1"
  );
  assert_eq!(sections.branch_metadata.file_range_li[1].term, 7);
  assert_eq!(
    sections.branch_metadata.file_range_li[1].max_segment_file_id,
    58
  );
  assert_eq!(
    sections.segment, None,
    "branch snapshot 不应带可选 segment 段"
  );
  let header = parse_header(&bytes).unwrap();
  assert_eq!(header.root, 3);
  assert_eq!(header.ttl_root, u32::MAX);
}

/// `archive` 是真实 archive snapshot：`CreateArchive` 在有 segment mapper 时
/// 同时写 max_segment_fp_id 与 segment mapping；这里还带非空 dict_bytes。
#[test]
fn cpp_oracle_manifest_archive_snapshot_parses_with_rust() {
  let Some(bytes) = read_case("archive") else {
    return;
  };
  assert!(validate_record(&bytes).unwrap());
  let sections = parse_snapshot(&bytes);
  assert_eq!(sections.max_fp_id, 100);
  assert_eq!(sections.dict_bytes, b"fkv-archive-dict".to_vec());
  assert_eq!(
    sections.mapping_value_li,
    vec![encode_file_page_id(21), encode_file_page_id(22)]
  );
  assert_eq!(sections.branch_metadata.branch_name.as_ref(), "main");
  assert_eq!(sections.branch_metadata.term, 0);
  assert_eq!(sections.branch_metadata.file_range_li.len(), 1);
  assert_eq!(sections.branch_metadata.file_range_li[0].max_file_id, 99);
  assert_eq!(
    sections.segment,
    Some((5, vec![encode_file_page_id(1), encode_file_page_id(2)])),
    "archive snapshot 应带 segment 段"
  );
  let header = parse_header(&bytes).unwrap();
  assert_eq!(header.root, 9);
  assert_eq!(header.ttl_root, 10);
}
