//! fixture 语料的 Rust 侧定义与双向断言。
//!
//! 语料与 `cpp/oracle/src/main.cpp` 一一对应，**两侧必须同步修改**：
//! - [`decode_case`]：用 `fkv_page` / `fkv_manifest` / `fkv_compress` 解码
//!   `payload.bin` 并与语料比对，失败给出首个差异 offset；
//! - [`build_payload`] / [`generate_reverse`]：用同一批 Rust builder 生成反向
//!   语料（Rust 写 → C++ 读），C++ `verify` 按同一语料逐条校验。

use std::{
  fs,
  iter::repeat_n,
  path::{Path, PathBuf},
  result,
};

use fkv_codec::{append_varint32, append_varint64, get_varint32, get_varint64};
use fkv_compress::{DictCompression, compress_raw, decompress_raw};
use fkv_manifest::{
  BranchManifestMetadata, HEADER_BYTES, ManifestBuilder, OFFSET_PAYLOAD_LEN, SnapshotArg,
  deserialize_branch_metadata, manifest_checksum, parse_header, validate_record,
};
use fkv_page::{
  CompressionType, DataPageBuilder, DataPageIter, EntrySpec, IndexPage, IndexPageBuilder,
  IndexPageIter, OverflowPage, PageType,
  overflow::{build as build_overflow_page, capacity as overflow_capacity},
  page_type, validate_checksum,
};
use fkv_types::{BranchFileRange, NULL_PAGE};

use crate::error::{LedgerError, Result};

/// 与 `KvOption` 默认值一致（cpp/oracle 语料口径）。
const PAGE_SIZE: usize = 4096;
const RESTART_INTERVAL: u16 = 16;
const OVERFLOW_POINTERS: u8 = 16;
/// 与 C++ `ZSTD_CLEVEL_DEFAULT` 一致。
const ZSTD_LEVEL: i32 = 3;

const OVERFLOW_KEY: &[u8] = b"of";
const OVERFLOW_PAGE_BASE: u32 = 7;
const OVERFLOW_TS: u64 = 100;
/// dict 样本数与样本长度（C++ `DictSamples` 同口径）。
const DICT_SAMPLE_COUNT: usize = 64;
const DICT_SAMPLE_LEN: usize = 1024;

/// 首个差异：payload 内 offset + 说明。
#[derive(Debug, Clone)]
pub struct CaseDiff {
  pub offset: usize,
  pub reason: String,
}

impl CaseDiff {
  fn at(offset: usize, reason: impl Into<String>) -> Self {
    Self {
      offset,
      reason: reason.into(),
    }
  }
}

fn repeat_to(pattern: &[u8], target: usize) -> Vec<u8> {
  let mut out = Vec::with_capacity(target + pattern.len());
  while out.len() < target {
    let remain = target - out.len();
    out.extend_from_slice(&pattern[..pattern.len().min(remain)]);
  }
  out
}

fn padded(prefix: &str, index: usize, width: usize) -> Vec<u8> {
  let digits = index.to_string();
  let mut out = prefix.as_bytes().to_vec();
  out.extend(repeat_n(b'0', width - digits.len()));
  out.extend_from_slice(digits.as_bytes());
  out
}

fn fixed32_le(bytes: &[u8], offset: usize) -> result::Result<u32, CaseDiff> {
  bytes
    .first_chunk::<4>()
    .map(|chunk| u32::from_le_bytes(*chunk))
    .ok_or_else(|| CaseDiff::at(offset, "4 字节长度段不足"))
}

// ---------------------------------------------------------------------------
// page 语料
// ---------------------------------------------------------------------------

struct PageEntry {
  key: Vec<u8>,
  value: Vec<u8>,
  ts: u64,
  expire_ts: u64,
  overflow: bool,
  large_value: bool,
}

fn plain_entry(key: Vec<u8>, value: Vec<u8>, ts: u64, expire_ts: u64) -> PageEntry {
  PageEntry {
    key,
    value,
    ts,
    expire_ts,
    overflow: false,
    large_value: false,
  }
}

/// data page 语料；`case` 必须是矩阵里的规范名（`large_values` 别名只在 C++ 侧）。
fn page_entry_li(case: &str) -> Option<Vec<PageEntry>> {
  let mut li = Vec::new();
  match case {
    "empty" => {}
    "single" => li.push(plain_entry(b"k0".to_vec(), b"v0".to_vec(), 100, 0)),
    "multi" => {
      for i in 0..8 {
        li.push(plain_entry(
          padded("k", i, 2),
          padded("v", i, 2),
          100 + i as u64,
          0,
        ));
      }
    }
    "restart" => {
      for i in 0..40 {
        li.push(plain_entry(
          padded("r", i, 3),
          vec![b'x'; 16],
          100 + i as u64,
          0,
        ));
      }
    }
    "ttl" => {
      for i in 0..4 {
        li.push(plain_entry(
          padded("t", i, 2),
          padded("v", i, 2),
          100 + i as u64,
          1000 + i as u64,
        ));
      }
    }
    "large" => {
      for i in 0..3 {
        li.push(PageEntry {
          key: padded("L", i, 2),
          value: vec![b'a' + i as u8; 1024],
          ts: 100 + i as u64,
          expire_ts: 0,
          overflow: false,
          large_value: true,
        });
      }
    }
    "overflow" => {
      li.push(plain_entry(b"o00".to_vec(), b"v00".to_vec(), 100, 0));
      // overflow entry 的 value 是页 id 指针数组（每个小端 4B）。
      let mut pointers = Vec::with_capacity(8);
      pointers.extend_from_slice(&OVERFLOW_PAGE_BASE.to_le_bytes());
      pointers.extend_from_slice(&(OVERFLOW_PAGE_BASE + 1).to_le_bytes());
      li.push(PageEntry {
        key: b"o01".to_vec(),
        value: pointers,
        ts: 101,
        expire_ts: 0,
        overflow: true,
        large_value: false,
      });
      li.push(plain_entry(b"o02".to_vec(), b"v02".to_vec(), 102, 0));
    }
    _ => return None,
  }
  Some(li)
}

// ---------------------------------------------------------------------------
// index 语料
// ---------------------------------------------------------------------------

struct IndexSpec {
  leaf: bool,
  leftmost: u32,
  entry_li: Vec<(Vec<u8>, u32)>,
}

fn index_spec(case: &str) -> Option<IndexSpec> {
  match case {
    "leaf" => Some(IndexSpec {
      leaf: true,
      leftmost: 1,
      entry_li: (0..4).map(|i| (padded("i", i, 2), 11 + i as u32)).collect(),
    }),
    "branch" => Some(IndexSpec {
      leaf: false,
      leftmost: 100,
      entry_li: (0..3)
        .map(|i| (padded("n", i, 2), 101 + i as u32))
        .collect(),
    }),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// overflow 语料（kind=overflow：data entry + 单组连号 overflow 页）
// ---------------------------------------------------------------------------

struct OverflowSpec {
  value: Vec<u8>,
  /// (page_id, chunk)，按链序。
  chunk_li: Vec<(u32, Vec<u8>)>,
  pointers: Vec<u8>,
}

fn overflow_spec() -> OverflowSpec {
  let value = repeat_to(b"eloqstore-overflow-", 8000);
  let page_cap = overflow_capacity(PAGE_SIZE, OVERFLOW_POINTERS, false).unwrap_or(0);
  let mut spec = OverflowSpec {
    value,
    chunk_li: Vec::new(),
    pointers: Vec::new(),
  };
  let mut page_id = OVERFLOW_PAGE_BASE;
  let mut off = 0;
  while off < spec.value.len() {
    let end = (off + page_cap).min(spec.value.len());
    spec.chunk_li.push((page_id, spec.value[off..end].to_vec()));
    spec.pointers.extend_from_slice(&page_id.to_le_bytes());
    off = end;
    page_id += 1;
  }
  spec
}

// ---------------------------------------------------------------------------
// manifest 语料
// ---------------------------------------------------------------------------

struct SegmentSpec {
  max_segment_fp_id: u64,
  /// segment 逻辑页 -> FilePageId（写入前按 `EncodeFilePageId` 编码）。
  fp_li: Vec<u64>,
}

struct ManifestSpec {
  root: u32,
  ttl_root: u32,
  /// `true`：`ManifestBuilder::Snapshot` 布局（branch / archive 真实快照）；
  /// `false`：log record 布局（empty / checksum，覆盖 delta 与 checksum）。
  snapshot: bool,
  max_fp_id: u64,
  dict_bytes: &'static [u8],
  /// (page_id, file_page_id)；写入前按 C++ `EncodeFilePageId` 编码。
  mapping_li: Vec<(u32, u64)>,
  /// `None` 表示无 branch 段（C++ 直接 Finalize 的空 record）。
  branch: Option<BranchManifestMetadata>,
  /// 可选 segment 段：C++ `Snapshot` 只在 segment_mapping != nullptr 时写。
  segment: Option<SegmentSpec>,
}

/// 对齐 C++ `MappingSnapshot::EncodeFilePageId`（fp<<3 | FilePageId 类型位）。
fn encode_file_page_id(fp: u64) -> u64 {
  (fp << 3) | 1
}

fn manifest_spec(case: &str) -> Option<ManifestSpec> {
  let spec = match case {
    // 空 log record：无 payload。
    "empty" => ManifestSpec {
      root: 0,
      ttl_root: NULL_PAGE,
      snapshot: false,
      max_fp_id: 0,
      dict_bytes: b"",
      mapping_li: Vec::new(),
      branch: None,
      segment: None,
    },
    "checksum" => ManifestSpec {
      root: 2,
      ttl_root: NULL_PAGE,
      snapshot: false,
      max_fp_id: 0,
      dict_bytes: b"",
      mapping_li: vec![(0, 1), (1, 2)],
      branch: Some(BranchManifestMetadata::new("", 0)),
      segment: None,
    },
    "branch" => {
      let mut branch = BranchManifestMetadata::new("feature-a3f7b2c1", 7);
      branch.file_range_li = vec![
        BranchFileRange {
          branch_name: "main".into(),
          term: 0,
          max_file_id: 42,
          max_segment_file_id: 43,
        },
        BranchFileRange {
          branch_name: "feature-a3f7b2c1".into(),
          term: 7,
          max_file_id: 57,
          max_segment_file_id: 58,
        },
      ];
      ManifestSpec {
        root: 3,
        ttl_root: NULL_PAGE,
        // C++ `BackgroundWrite::CreateBranch`：max_fp_id = (file_id+1)<<18，
        // 且不传 segment mapping，记录在 branch metadata 后结束。
        snapshot: true,
        max_fp_id: 58 << 18,
        dict_bytes: b"",
        mapping_li: vec![(0, 5), (1, 6), (2, 7)],
        branch: Some(branch),
        segment: None,
      }
    }
    "archive" => {
      let mut branch = BranchManifestMetadata::new("main", 0);
      branch.file_range_li = vec![BranchFileRange {
        branch_name: "main".into(),
        term: 0,
        max_file_id: 99,
        max_segment_file_id: 100,
      }];
      ManifestSpec {
        root: 9,
        ttl_root: 10,
        // C++ `BackgroundWrite::CreateArchive`：有 segment mapper 时同时写
        // max_segment_fp_id + segment mapping；dict 段也可非空。
        snapshot: true,
        max_fp_id: 100,
        dict_bytes: b"fkv-archive-dict",
        mapping_li: vec![(0, 21), (1, 22)],
        branch: Some(branch),
        segment: Some(SegmentSpec {
          max_segment_fp_id: 5,
          fp_li: vec![1, 2],
        }),
      }
    }
    _ => return None,
  };
  Some(spec)
}

// ---------------------------------------------------------------------------
// compress 语料
// ---------------------------------------------------------------------------

fn raw_input() -> Vec<u8> {
  repeat_to(b"fkv-raw-compress|0123456789abcdef|", 2048)
}

fn dict_input() -> Vec<u8> {
  repeat_to(b"fkv-dict-compress|branch=main|term=7|", 2048)
}

fn dict_sample_li() -> Vec<Vec<u8>> {
  (0..DICT_SAMPLE_COUNT)
    .map(|i| {
      repeat_to(
        format!("fkv-dict-sample-{i:02}|branch=main|term=7|").as_bytes(),
        DICT_SAMPLE_LEN,
      )
    })
    .collect()
}

// ---------------------------------------------------------------------------
// mapping 语料（MappingSnapshot::Serialize 字节 + page_count 前缀）
// ---------------------------------------------------------------------------

const STORE_MAGIC: &[u8; 4] = b"FST1";

/// 与 C++ `MappingSpecOf` 一一对应；值是 `MappingSnapshot` 的编码值
/// （低 3 位 ValType，高位 FilePageId / PageId）。
fn mapping_value_li(case: &str) -> Option<Vec<u64>> {
  let value_li = match case {
    "empty" => Vec::new(),
    "single" => vec![encode_file_page_id(42)],
    "multi" => vec![
      encode_file_page_id(1),
      fkv_types::INVALID_VALUE,
      encode_file_page_id(5),
      fkv_types::encode_page_id(0),
      encode_file_page_id(9),
      fkv_types::INVALID_VALUE,
      encode_file_page_id(257),
      encode_file_page_id(65536),
    ],
    _ => return None,
  };
  Some(value_li)
}

/// `page_count(varint32) | page_count × encoded_value(varint64)`；主体即
/// `MappingSnapshot::Serialize` 的输出。
fn build_mapping_bytes(case: &str) -> Result<Vec<u8>> {
  let value_li = mapping_value_li(case)
    .ok_or_else(|| LedgerError::Invalid(format!("unknown mapping case `{case}`")))?;
  let mut out = Vec::new();
  append_varint32(&mut out, value_li.len() as u32);
  for value in value_li {
    append_varint64(&mut out, value);
  }
  Ok(out)
}

// ---------------------------------------------------------------------------
// store 语料（有序 name/len/bytes 目录文件清单）
// ---------------------------------------------------------------------------

/// 与 C++ `StoreFilesOf` 一一对应；返回按 name 字节序升序的 (name, data)。
fn store_file_li(case: &str) -> Result<Vec<(String, Vec<u8>)>> {
  let mut file_li: Vec<(String, Vec<u8>)> = match case {
    "empty" => Vec::new(),
    "single" => vec![("data_0_0".to_owned(), build_page_bytes("single")?)],
    "multi" => vec![
      ("data_0_0".to_owned(), build_page_bytes("multi")?),
      ("data_1_0".to_owned(), build_page_bytes("single")?),
      ("manifest_0".to_owned(), build_manifest_bytes("branch")?),
      (
        "manifest_0_1730000000".to_owned(),
        build_manifest_bytes("archive")?,
      ),
      (
        "segment_0_main_0".to_owned(),
        repeat_to(b"fkv-segment|main|0|", 512),
      ),
    ],
    other => {
      return Err(LedgerError::Invalid(format!(
        "unknown store case `{other}`"
      )));
    }
  };
  file_li.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
  Ok(file_li)
}

/// `magic("FST1") | file_count(varint32) | file_count × [name_len | name | data_len | data]`。
fn build_store_bytes(case: &str) -> Result<Vec<u8>> {
  let file_li = store_file_li(case)?;
  let mut out = Vec::new();
  out.extend_from_slice(STORE_MAGIC);
  append_varint32(&mut out, file_li.len() as u32);
  for (name, data) in &file_li {
    append_varint32(&mut out, name.len() as u32);
    out.extend_from_slice(name.as_bytes());
    append_varint32(&mut out, data.len() as u32);
    out.extend_from_slice(data);
  }
  Ok(out)
}

// ---------------------------------------------------------------------------
// 期望字节重建（字节可跨实现复现的 kind）
// ---------------------------------------------------------------------------

fn build_page_bytes(case: &str) -> Result<Vec<u8>> {
  let entry_li = page_entry_li(case)
    .ok_or_else(|| LedgerError::Invalid(format!("unknown page case `{case}`")))?;
  let mut builder = DataPageBuilder::new(PAGE_SIZE, RESTART_INTERVAL)?;
  for entry in &entry_li {
    let added = builder.add(EntrySpec {
      key: &entry.key,
      value: &entry.value,
      ts: entry.ts,
      expire_ts: entry.expire_ts,
      overflow: entry.overflow,
      compression: CompressionType::None,
      large_value: entry.large_value,
    })?;
    if !added {
      return Err(LedgerError::Invalid(format!("page case `{case}` 页满")));
    }
  }
  Ok(builder.finish()?)
}

fn build_index_bytes(case: &str) -> Result<Vec<u8>> {
  let spec =
    index_spec(case).ok_or_else(|| LedgerError::Invalid(format!("unknown index case `{case}`")))?;
  let mut builder = IndexPageBuilder::new(PAGE_SIZE, RESTART_INTERVAL, spec.leaf)?;
  // 首条 add 写 leftmost 指针（空 key），对齐 C++ `Add({}, leftmost, leaf)`。
  if !builder.add(&[], spec.leftmost)? {
    return Err(LedgerError::Invalid("index leftmost 写入失败".to_owned()));
  }
  for (key, page_id) in &spec.entry_li {
    if !builder.add(key, *page_id)? {
      return Err(LedgerError::Invalid("index 页满".to_owned()));
    }
  }
  Ok(builder.finish()?)
}

fn build_overflow_bytes() -> Result<Vec<u8>> {
  let spec = overflow_spec();
  let mut builder = DataPageBuilder::new(PAGE_SIZE, RESTART_INTERVAL)?;
  let added = builder.add(EntrySpec {
    key: OVERFLOW_KEY,
    value: &spec.pointers,
    ts: OVERFLOW_TS,
    expire_ts: 0,
    overflow: true,
    compression: CompressionType::None,
    large_value: false,
  })?;
  if !added {
    return Err(LedgerError::Invalid("overflow data 页满".to_owned()));
  }
  let mut blob = builder.finish()?;
  for (_, chunk) in &spec.chunk_li {
    blob.extend_from_slice(&build_overflow_page(
      PAGE_SIZE,
      chunk,
      &[],
      OVERFLOW_POINTERS,
    )?);
  }
  Ok(blob)
}

fn fixed32(bytes: &[u8]) -> u32 {
  u32::from_le_bytes(bytes.try_into().expect("长度由调用方保证"))
}

/// manifest 语料；与 C++ 一致只保留 `header + payload`，不含对齐 padding
/// （padding 不参与 checksum，且 macOS / Linux 页大小不同，写进语料会不可复现）。
///
/// `branch` / `archive` 是真实 snapshot 布局（`ManifestBuilder::Snapshot`），
/// `empty` / `checksum` 仍是 log record 布局，用于覆盖 delta / checksum 路径。
fn build_manifest_bytes(case: &str) -> Result<Vec<u8>> {
  let spec = manifest_spec(case)
    .ok_or_else(|| LedgerError::Invalid(format!("unknown manifest case `{case}`")))?;
  let Some(branch) = &spec.branch else {
    // fkv_manifest 的 finalize 只覆盖带 mapping 段的 log；空 record（payload_len=0）
    // 手工拼 20B header，与 C++ `Finalize` 直接产出一致。
    let mut buf = vec![0u8; HEADER_BYTES];
    buf[8..12].copy_from_slice(&spec.root.to_le_bytes());
    buf[12..16].copy_from_slice(&spec.ttl_root.to_le_bytes());
    let sum = manifest_checksum(&buf[8..]);
    buf[..8].copy_from_slice(&sum.to_le_bytes());
    return Ok(buf);
  };
  if spec.snapshot {
    let mapping_value_li: Vec<u64> = spec
      .mapping_li
      .iter()
      .map(|(_, fp)| encode_file_page_id(*fp))
      .collect();
    let segment_value_li: Option<Vec<u64>> = spec.segment.as_ref().map(|segment| {
      segment
        .fp_li
        .iter()
        .map(|fp| encode_file_page_id(*fp))
        .collect()
    });
    let aligned = ManifestBuilder::new()
      .snapshot(
        SnapshotArg {
          root_id: spec.root,
          ttl_root: spec.ttl_root,
          max_fp_id: spec.max_fp_id,
          dict_bytes: spec.dict_bytes,
          branch_metadata: branch,
          max_segment_fp_id: spec
            .segment
            .as_ref()
            .map_or(0, |segment| segment.max_segment_fp_id),
        },
        &mapping_value_li,
        segment_value_li.as_deref(),
      )
      .map_err(|err| LedgerError::Invalid(format!("snapshot 编码失败：{err}")))?;
    let payload_len = fixed32(&aligned[OFFSET_PAYLOAD_LEN..OFFSET_PAYLOAD_LEN + 4]);
    return Ok(aligned[..HEADER_BYTES + payload_len as usize].to_vec());
  }
  let mut builder = ManifestBuilder::new();
  for (page_id, fp) in &spec.mapping_li {
    builder.update_mapping(*page_id, encode_file_page_id(*fp));
  }
  builder
    .append_branch_metadata(branch)
    .map_err(|err| LedgerError::Invalid(format!("branch metadata 追加失败：{err}")))?;
  let aligned = builder.finalize(spec.root, spec.ttl_root)?;
  let payload_len = fixed32(&aligned[OFFSET_PAYLOAD_LEN..OFFSET_PAYLOAD_LEN + 4]);
  Ok(aligned[..HEADER_BYTES + payload_len as usize].to_vec())
}

/// 可逐字节复现的 kind：重建期望字节；compress 不可跨实现复现，返回 `None`。
fn expected_bytes(kind: &str, case: &str) -> Result<Option<Vec<u8>>> {
  let bytes = match kind {
    "page" => build_page_bytes(case)?,
    "index" => build_index_bytes(case)?,
    "overflow" => build_overflow_bytes()?,
    "manifest" => build_manifest_bytes(case)?,
    "compress" => return Ok(None),
    "mapping" => build_mapping_bytes(case)?,
    "store" => build_store_bytes(case)?,
    other => {
      return Err(LedgerError::Invalid(format!(
        "unknown fixture kind `{other}`"
      )));
    }
  };
  Ok(Some(bytes))
}

/// 两个字节串的首个差异 offset；前缀相同则报长度差处。
fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
  a.iter()
    .zip(b.iter())
    .position(|(x, y)| x != y)
    .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
}

// ---------------------------------------------------------------------------
// 解码断言
// ---------------------------------------------------------------------------

type DiffResult = result::Result<(), CaseDiff>;

fn diff_page(case: &str, payload: &[u8]) -> DiffResult {
  let entry_li =
    page_entry_li(case).ok_or_else(|| CaseDiff::at(0, format!("unknown page case `{case}`")))?;
  let mut iter = DataPageIter::new(payload)
    .map_err(|err| CaseDiff::at(0, format!("data page 解析失败：{err}")))?;
  for (index, want) in entry_li.iter().enumerate() {
    let more = iter
      .advance()
      .map_err(|err| CaseDiff::at(0, format!("entry {index} 迭代失败：{err}")))?;
    if !more {
      return Err(CaseDiff::at(0, format!("entry {index} 缺失")));
    }
    let value_offset = iter.value_range().0;
    if iter.key() != want.key {
      return Err(CaseDiff::at(
        value_offset,
        format!("entry {index} key 不一致"),
      ));
    }
    if iter.value() != want.value {
      return Err(CaseDiff::at(
        value_offset,
        format!("entry {index} value 不一致"),
      ));
    }
    if iter.timestamp() != want.ts || iter.expire_ts() != want.expire_ts {
      return Err(CaseDiff::at(
        value_offset,
        format!("entry {index} 时间戳不一致"),
      ));
    }
    if iter.is_overflow() != want.overflow || iter.is_large_value() != want.large_value {
      return Err(CaseDiff::at(
        value_offset,
        format!("entry {index} 标志位不一致"),
      ));
    }
  }
  if iter.advance().unwrap_or(false) {
    return Err(CaseDiff::at(0, "条目数多于语料"));
  }
  Ok(())
}

fn diff_index(case: &str, payload: &[u8]) -> DiffResult {
  let spec =
    index_spec(case).ok_or_else(|| CaseDiff::at(0, format!("unknown index case `{case}`")))?;
  // Rust 迭代器不产出 leftmost 虚拟条目；leftmost 从页头字段独立断言。
  let leftmost = IndexPage::new(payload)
    .and_then(|page| page.leftmost_ptr())
    .map_err(|err| CaseDiff::at(11, format!("leftmost 解析失败：{err}")))?;
  if leftmost != spec.leftmost {
    return Err(CaseDiff::at(11, "leftmost 指针不一致"));
  }
  let mut iter = IndexPageIter::new(payload)
    .map_err(|err| CaseDiff::at(0, format!("index page 解析失败：{err}")))?;
  for (index, (key, page_id)) in spec.entry_li.iter().enumerate() {
    let more = iter
      .advance()
      .map_err(|err| CaseDiff::at(0, format!("index entry {index} 迭代失败：{err}")))?;
    if !more {
      return Err(CaseDiff::at(0, format!("index entry {index} 缺失")));
    }
    if iter.key() != key || iter.page_id() != *page_id {
      return Err(CaseDiff::at(0, format!("index entry {index} 不一致")));
    }
  }
  if iter.advance().unwrap_or(false) {
    return Err(CaseDiff::at(0, "index 条目数多于语料"));
  }
  Ok(())
}

fn diff_overflow(payload: &[u8]) -> DiffResult {
  let spec = overflow_spec();
  let pages = 1 + spec.chunk_li.len();
  if payload.len() != pages * PAGE_SIZE {
    return Err(CaseDiff::at(
      payload.len(),
      format!("payload {} 字节，期望 {pages}×{PAGE_SIZE}", payload.len()),
    ));
  }
  // data 页：页类型 + checksum + "of" overflow 条目语义。
  let data_slice = &payload[..PAGE_SIZE];
  if page_type(data_slice) != Ok(PageType::Data) {
    return Err(CaseDiff::at(8, "第一页不是 data 页"));
  }
  if !validate_checksum(data_slice).unwrap_or(false) {
    return Err(CaseDiff::at(0, "data 页 checksum 不一致"));
  }
  let mut iter = DataPageIter::new(data_slice)
    .map_err(|err| CaseDiff::at(0, format!("data 页解析失败：{err}")))?;
  let more = iter
    .advance()
    .map_err(|err| CaseDiff::at(0, format!("overflow data entry 迭代失败：{err}")))?;
  if !more
    || iter.key() != OVERFLOW_KEY
    || !iter.is_overflow()
    || iter.timestamp() != OVERFLOW_TS
    || iter.expire_ts() != 0
  {
    return Err(CaseDiff::at(0, "overflow data entry 不一致"));
  }
  if iter.value() != spec.pointers {
    return Err(CaseDiff::at(
      iter.value_range().0,
      "overflow 指针数组不一致",
    ));
  }
  if iter.advance().unwrap_or(false) {
    return Err(CaseDiff::at(0, "overflow data 页条目数多于语料"));
  }
  // overflow 页：checksum + 页类型 + 内容 + 尾指针为空，重组后等于原值。
  let mut rebuilt = Vec::with_capacity(spec.value.len());
  for (index, (_, chunk)) in spec.chunk_li.iter().enumerate() {
    let offset = (1 + index) * PAGE_SIZE;
    let slice = &payload[offset..offset + PAGE_SIZE];
    if !validate_checksum(slice).unwrap_or(false) {
      return Err(CaseDiff::at(offset, "overflow 页 checksum 不一致"));
    }
    let page = OverflowPage::new(slice)
      .map_err(|err| CaseDiff::at(offset, format!("overflow 页解析失败：{err}")))?;
    let value = page
      .value()
      .map_err(|err| CaseDiff::at(offset, format!("overflow 页取值失败：{err}")))?;
    if value != chunk.as_slice() {
      return Err(CaseDiff::at(offset, "overflow 页内容不一致"));
    }
    if page.num_pointers() != Ok(0) || !page.pointer_li().unwrap_or_default().is_empty() {
      return Err(CaseDiff::at(offset, "overflow 页尾指针应为空"));
    }
    rebuilt.extend_from_slice(value);
  }
  if rebuilt != spec.value {
    let offset = first_diff(&rebuilt, &spec.value).unwrap_or(0);
    return Err(CaseDiff::at(offset, "overflow 重组值与语料不一致"));
  }
  Ok(())
}

fn diff_manifest(case: &str, payload: &[u8]) -> DiffResult {
  let spec = manifest_spec(case)
    .ok_or_else(|| CaseDiff::at(0, format!("unknown manifest case `{case}`")))?;
  let header = parse_header(payload)
    .map_err(|err| CaseDiff::at(0, format!("manifest header 解析失败：{err}")))?;
  let record_len = HEADER_BYTES + header.payload_len as usize;
  if payload.len() != record_len {
    return Err(CaseDiff::at(
      record_len,
      format!(
        "manifest record 应为 {record_len} 字节，实际 {}",
        payload.len()
      ),
    ));
  }
  if !validate_record(payload).unwrap_or(false) {
    return Err(CaseDiff::at(0, "manifest checksum 不一致"));
  }
  if header.root != spec.root || header.ttl_root != spec.ttl_root {
    return Err(CaseDiff::at(8, "manifest root / ttl_root 不一致"));
  }
  let Some(want_branch) = &spec.branch else {
    if header.payload_len != 0 {
      return Err(CaseDiff::at(HEADER_BYTES, "空 manifest 不应有 payload"));
    }
    return Ok(());
  };
  if spec.snapshot {
    return diff_snapshot_manifest(&spec, payload);
  }
  // log record：mapping_len(4B) | mapping varints | branch_len(4B) | branch。
  let mut pos = HEADER_BYTES;
  let mapping_len = fixed32_le(&payload[pos..], pos)? as usize;
  pos += 4;
  let mapping_end = pos + mapping_len;
  let mut mapping_li = Vec::new();
  while pos < mapping_end {
    let (page_id, read) = get_varint32(&payload[pos..])
      .map_err(|err| CaseDiff::at(pos, format!("mapping page_id 解析失败：{err}")))?;
    pos += read;
    let (value, read) = fkv_codec::get_varint64(&payload[pos..])
      .map_err(|err| CaseDiff::at(pos, format!("mapping value 解析失败：{err}")))?;
    pos += read;
    mapping_li.push((page_id, value));
  }
  if pos != mapping_end {
    return Err(CaseDiff::at(mapping_end, "mapping 段长度与条目不一致"));
  }
  if mapping_li.len() != spec.mapping_li.len() {
    return Err(CaseDiff::at(HEADER_BYTES, "mapping 条数不一致"));
  }
  for (index, (got, want)) in mapping_li.iter().zip(spec.mapping_li.iter()).enumerate() {
    if *got != (want.0, encode_file_page_id(want.1)) {
      return Err(CaseDiff::at(
        HEADER_BYTES,
        format!("mapping {index} 不一致"),
      ));
    }
  }
  let branch_len = fixed32_le(&payload[pos..], pos)? as usize;
  pos += 4;
  let branch = deserialize_branch_metadata(&payload[pos..pos + branch_len])
    .map_err(|err| CaseDiff::at(pos, format!("branch metadata 解析失败：{err}")))?;
  if branch.branch_name != want_branch.branch_name || branch.term != want_branch.term {
    return Err(CaseDiff::at(pos, "branch 名称 / term 不一致"));
  }
  if branch.file_range_li != want_branch.file_range_li {
    return Err(CaseDiff::at(pos, "branch file_range_li 不一致"));
  }
  Ok(())
}

fn take_varint64(payload: &[u8], pos: &mut usize) -> result::Result<u64, CaseDiff> {
  let rest = payload
    .get(*pos..)
    .ok_or_else(|| CaseDiff::at(*pos, "varint64 越界"))?;
  let (value, read) =
    get_varint64(rest).map_err(|err| CaseDiff::at(*pos, format!("varint64 解析失败：{err}")))?;
  *pos += read;
  Ok(value)
}

fn take_varint32(payload: &[u8], pos: &mut usize) -> result::Result<u32, CaseDiff> {
  let rest = payload
    .get(*pos..)
    .ok_or_else(|| CaseDiff::at(*pos, "varint32 越界"))?;
  let (value, read) =
    get_varint32(rest).map_err(|err| CaseDiff::at(*pos, format!("varint32 解析失败：{err}")))?;
  *pos += read;
  Ok(value)
}

fn take_fixed32(payload: &[u8], pos: &mut usize) -> result::Result<u32, CaseDiff> {
  let value = fixed32_le(
    payload
      .get(*pos..)
      .ok_or_else(|| CaseDiff::at(*pos, "fixed32 越界"))?,
    *pos,
  )?;
  *pos += 4;
  Ok(value)
}

/// 解析一段 `varint64...`，返回每个值；`base` 仅用于错误 offset。
fn decode_varint64_section(section: &[u8], base: usize) -> result::Result<Vec<u64>, CaseDiff> {
  let mut pos = 0usize;
  let mut value_li = Vec::new();
  while pos < section.len() {
    let (value, read) = get_varint64(&section[pos..])
      .map_err(|err| CaseDiff::at(base + pos, format!("varint64 解析失败：{err}")))?;
    pos += read;
    value_li.push(value);
  }
  Ok(value_li)
}

/// 真实 snapshot：max_fp_id | dict | mapping | branch metadata | [segment]。
fn diff_snapshot_manifest(spec: &ManifestSpec, payload: &[u8]) -> DiffResult {
  let want_branch = spec
    .branch
    .as_ref()
    .ok_or_else(|| CaseDiff::at(HEADER_BYTES, "snapshot 缺少 branch metadata 语料"))?;
  let mut pos = HEADER_BYTES;
  let max_fp_id = take_varint64(payload, &mut pos)?;
  if max_fp_id != spec.max_fp_id {
    return Err(CaseDiff::at(
      HEADER_BYTES,
      format!(
        "snapshot max_fp_id 应为 {}，实际 {max_fp_id}",
        spec.max_fp_id
      ),
    ));
  }
  let dict_len = take_varint32(payload, &mut pos)? as usize;
  let dict_end = pos
    .checked_add(dict_len)
    .ok_or_else(|| CaseDiff::at(pos, "dict 段长度溢出"))?;
  let got_dict = payload
    .get(pos..dict_end)
    .ok_or_else(|| CaseDiff::at(pos, "dict 段越界"))?;
  if got_dict != spec.dict_bytes {
    return Err(CaseDiff::at(pos, "snapshot dict_bytes 不一致"));
  }
  pos = dict_end;

  let mapping_len = take_fixed32(payload, &mut pos)? as usize;
  let mapping_end = pos
    .checked_add(mapping_len)
    .ok_or_else(|| CaseDiff::at(pos, "mapping 段长度溢出"))?;
  let mapping_view = payload
    .get(pos..mapping_end)
    .ok_or_else(|| CaseDiff::at(pos, "mapping 段越界"))?;
  let mapping_value_li = decode_varint64_section(mapping_view, pos)?;
  let want_mapping_li: Vec<u64> = spec
    .mapping_li
    .iter()
    .map(|(_, fp)| encode_file_page_id(*fp))
    .collect();
  if mapping_value_li != want_mapping_li {
    return Err(CaseDiff::at(pos, "snapshot mapping 值不一致"));
  }
  pos = mapping_end;

  let branch_len = take_fixed32(payload, &mut pos)? as usize;
  let branch_end = pos
    .checked_add(branch_len)
    .ok_or_else(|| CaseDiff::at(pos, "branch metadata 段长度溢出"))?;
  let branch_bytes = payload
    .get(pos..branch_end)
    .ok_or_else(|| CaseDiff::at(pos, "branch metadata 段越界"))?;
  let branch = deserialize_branch_metadata(branch_bytes)
    .map_err(|err| CaseDiff::at(pos, format!("branch metadata 解析失败：{err}")))?;
  if branch != *want_branch {
    return Err(CaseDiff::at(pos, "snapshot branch metadata 不一致"));
  }
  pos = branch_end;

  match &spec.segment {
    Some(segment) => {
      let max_segment_fp_id = take_varint64(payload, &mut pos)?;
      if max_segment_fp_id != segment.max_segment_fp_id {
        return Err(CaseDiff::at(
          pos,
          format!(
            "snapshot max_segment_fp_id 应为 {}，实际 {max_segment_fp_id}",
            segment.max_segment_fp_id
          ),
        ));
      }
      let seg_len = take_fixed32(payload, &mut pos)? as usize;
      let seg_end = pos
        .checked_add(seg_len)
        .ok_or_else(|| CaseDiff::at(pos, "segment 段长度溢出"))?;
      let seg_view = payload
        .get(pos..seg_end)
        .ok_or_else(|| CaseDiff::at(pos, "segment 段越界"))?;
      let segment_value_li = decode_varint64_section(seg_view, pos)?;
      let want_segment_li: Vec<u64> = segment
        .fp_li
        .iter()
        .map(|fp| encode_file_page_id(*fp))
        .collect();
      if segment_value_li != want_segment_li {
        return Err(CaseDiff::at(pos, "snapshot segment mapping 不一致"));
      }
      pos = seg_end;
    }
    None => {
      if pos != payload.len() {
        return Err(CaseDiff::at(
          pos,
          "无 segment mapping 的 snapshot 不应有 segment 段",
        ));
      }
    }
  }
  if pos != payload.len() {
    return Err(CaseDiff::at(pos, "snapshot 尾部有多余字节"));
  }
  Ok(())
}

fn diff_compress(case: &str, payload: &[u8], dict: Option<&[u8]>) -> DiffResult {
  let want: Vec<u8> = match case {
    "raw" => raw_input(),
    "dict" => dict_input(),
    other => return Err(CaseDiff::at(0, format!("unknown compress case `{other}`"))),
  };
  let decompressed = match case {
    "raw" => {
      decompress_raw(payload).map_err(|err| CaseDiff::at(0, format!("raw 解压失败：{err}")))?
    }
    "dict" => {
      let dict = dict.ok_or_else(|| CaseDiff::at(0, "缺少 dict.bin"))?;
      let mut compression = DictCompression::new(ZSTD_LEVEL);
      compression
        .load_dictionary(dict.to_vec())
        .map_err(|err| CaseDiff::at(0, format!("字典加载失败：{err}")))?;
      compression
        .decompress(payload)
        .map_err(|err| CaseDiff::at(0, format!("dict 解压失败：{err}")))?
    }
    _ => return Err(CaseDiff::at(0, "unreachable")),
  };
  if decompressed != want {
    let offset = first_diff(&decompressed, &want).unwrap_or(0);
    return Err(CaseDiff::at(offset, "解压结果与语料不一致"));
  }
  Ok(())
}

fn diff_mapping(case: &str, payload: &[u8]) -> DiffResult {
  let want_li = mapping_value_li(case)
    .ok_or_else(|| CaseDiff::at(0, format!("unknown mapping case `{case}`")))?;
  let (count, used) = get_varint32(payload)
    .map_err(|err| CaseDiff::at(0, format!("mapping page_count 解析失败：{err}")))?;
  if count as usize != want_li.len() {
    return Err(CaseDiff::at(
      0,
      format!("mapping page_count {count}，期望 {}", want_li.len()),
    ));
  }
  let mut pos = used;
  for (index, want) in want_li.iter().enumerate() {
    let value = take_varint64(payload, &mut pos)?;
    if value != *want {
      return Err(CaseDiff::at(
        pos,
        format!("mapping value {index} 不一致（实际 {value:#x}）"),
      ));
    }
  }
  if pos != payload.len() {
    return Err(CaseDiff::at(pos, "mapping 尾部有多余字节"));
  }
  Ok(())
}

fn diff_store(case: &str, payload: &[u8]) -> DiffResult {
  let want_li = store_file_li(case).map_err(|err| CaseDiff::at(0, err.to_string()))?;
  if payload.len() < STORE_MAGIC.len() || &payload[..STORE_MAGIC.len()] != STORE_MAGIC {
    return Err(CaseDiff::at(0, "store magic 不是 FST1"));
  }
  let mut pos = STORE_MAGIC.len();
  let (count, used) = get_varint32(&payload[pos..])
    .map_err(|err| CaseDiff::at(pos, format!("store file_count 解析失败：{err}")))?;
  pos += used;
  if count as usize != want_li.len() {
    return Err(CaseDiff::at(
      pos,
      format!("store file_count {count}，期望 {}", want_li.len()),
    ));
  }
  for (index, (want_name, want_data)) in want_li.iter().enumerate() {
    let (name_len, used) = get_varint32(&payload[pos..])
      .map_err(|err| CaseDiff::at(pos, format!("store name_len 解析失败：{err}")))?;
    pos += used;
    let name_end = pos
      .checked_add(name_len as usize)
      .ok_or_else(|| CaseDiff::at(pos, "store name 长度溢出"))?;
    let name = payload
      .get(pos..name_end)
      .ok_or_else(|| CaseDiff::at(pos, "store name 越界"))?;
    pos = name_end;
    let (data_len, used) = get_varint32(&payload[pos..])
      .map_err(|err| CaseDiff::at(pos, format!("store data_len 解析失败：{err}")))?;
    pos += used;
    let data_end = pos
      .checked_add(data_len as usize)
      .ok_or_else(|| CaseDiff::at(pos, "store data 长度溢出"))?;
    let data = payload
      .get(pos..data_end)
      .ok_or_else(|| CaseDiff::at(pos, "store data 越界"))?;
    if name != want_name.as_bytes() {
      return Err(CaseDiff::at(pos, format!("store file {index} name 不一致")));
    }
    if data != want_data.as_slice() {
      let offset = first_diff(data, want_data).unwrap_or(0);
      return Err(CaseDiff::at(
        pos.saturating_add(offset),
        format!("store file {index} ({want_name}) 内容不一致"),
      ));
    }
    // 清单里的字节必须仍能被真实 parser 读回。
    if want_name.starts_with("data_") {
      if page_type(data) != Ok(PageType::Data) || !validate_checksum(data).unwrap_or(false) {
        return Err(CaseDiff::at(
          pos,
          format!("store file {index} 不是合法 data 页"),
        ));
      }
    } else if want_name.starts_with("manifest_") && !validate_record(data).unwrap_or(false) {
      return Err(CaseDiff::at(
        pos,
        format!("store file {index} manifest checksum 不一致"),
      ));
    } else if want_name.starts_with("segment_") && data.is_empty() {
      return Err(CaseDiff::at(
        pos,
        format!("store file {index} segment 为空"),
      ));
    }
    pos = data_end;
  }
  if pos != payload.len() {
    return Err(CaseDiff::at(pos, "store 尾部有多余字节"));
  }
  Ok(())
}

/// 解码一个 fixture case 并与语料断言；失败返回首个差异。
///
/// 语义断言先行（错误可定位到条目），字节级重建比对兜底（能覆盖 checksum 区
/// 等语义层看不出的差异）。`dict` 仅 `compress/dict` 需要（`dict.bin` 内容）。
pub fn decode_case(
  kind: &str,
  case: &str,
  payload: &[u8],
  dict: Option<&[u8]>,
) -> result::Result<(), CaseDiff> {
  match kind {
    "page" => diff_page(case, payload)?,
    "index" => diff_index(case, payload)?,
    "overflow" => diff_overflow(payload)?,
    "manifest" => diff_manifest(case, payload)?,
    "compress" => return diff_compress(case, payload, dict),
    "mapping" => diff_mapping(case, payload)?,
    "store" => diff_store(case, payload)?,
    other => return Err(CaseDiff::at(0, format!("unknown fixture kind `{other}`"))),
  }
  let expected = expected_bytes(kind, case)
    .map_err(|err| CaseDiff::at(0, format!("重建期望字节失败：{err}")))?;
  if let Some(offset) = expected
    .as_deref()
    .and_then(|bytes| first_diff(bytes, payload))
  {
    return Err(CaseDiff::at(offset, "字节与 Rust 重建的语料不一致"));
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// 反向 fixture（Rust 写 → C++ verify 读）
// ---------------------------------------------------------------------------

/// 用 Rust builder 生成一个 case 的 payload（与 C++ oracle 同一语料）。
///
/// compress 字节不可跨实现复现，由 Rust 侧 zstd 产出（C++ 只解压比对原值）。
pub fn build_payload(kind: &str, case: &str) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
  match kind {
    "page" => Ok((build_page_bytes(case)?, None)),
    "index" => Ok((build_index_bytes(case)?, None)),
    "overflow" => Ok((build_overflow_bytes()?, None)),
    "manifest" => Ok((build_manifest_bytes(case)?, None)),
    "mapping" => Ok((build_mapping_bytes(case)?, None)),
    "store" => Ok((build_store_bytes(case)?, None)),
    "compress" => match case {
      "raw" => {
        let payload = compress_raw(&raw_input(), ZSTD_LEVEL)?
          .ok_or_else(|| LedgerError::Invalid("raw 压缩未变小".to_owned()))?;
        Ok((payload, None))
      }
      "dict" => {
        let mut compression = DictCompression::new(ZSTD_LEVEL);
        for sample in dict_sample_li() {
          compression.add_sample(&sample);
        }
        compression
          .build_dictionary()
          .map_err(|err| LedgerError::Invalid(format!("字典训练失败：{err}")))?;
        let dict = compression.dictionary_bytes().to_vec();
        let payload = compression
          .compress(&dict_input())?
          .ok_or_else(|| LedgerError::Invalid("dict 压缩未变小".to_owned()))?;
        Ok((payload, Some(dict)))
      }
      other => Err(LedgerError::Invalid(format!(
        "unknown compress case `{other}`"
      ))),
    },
    other => Err(LedgerError::Invalid(format!(
      "unknown fixture kind `{other}`"
    ))),
  }
}

/// 生成反向 fixture 目录 `<out>/<kind>/<case>/`（payload + dict + meta）。
///
/// `status=pass` 表示本侧生成完成；交叉校验由 `verify`（C++ oracle 读回）给出。
pub fn generate_reverse(
  out: &Path,
  kind: &str,
  case: &str,
  baseline: &str,
  schema_version: u32,
) -> Result<PathBuf> {
  let (payload, dict) = build_payload(kind, case)?;
  let dir = out.join(kind).join(case);
  fs::create_dir_all(&dir)?;
  fs::write(dir.join("payload.bin"), &payload)?;
  if let Some(dict) = &dict {
    fs::write(dir.join("dict.bin"), dict)?;
  }
  fs::write(
    dir.join("meta.txt"),
    format!(
      "schema={schema_version}\nkind={kind}\ncase={case}\nbaseline={baseline}\nstatus=pass\noracle=-\npayload=payload.bin\n",
    ),
  )?;
  Ok(dir)
}
