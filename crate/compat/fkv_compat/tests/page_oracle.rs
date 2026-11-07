//! C++ oracle 生成的 page / index 字节 → Rust 解码断言（L1 字节兼容 fixture）。
//!
//! 语料由 `cpp/oracle`（eloqstore 真实 `DataPageBuilder` / `IndexPageBuilder` +
//! `SetChecksum`）产出，落在 `fixtures/cpp/<kind>/<case>/payload.bin`，
//! 契约定在 `fixtures/README.md`。本测试只解码断言，不需要 C++ 工具链。

mod common;

use std::{fs, path::PathBuf};

use fkv_page::{
  CompressionType, DataPage, DataPageIter, IndexPage, IndexPageIter, PageType, decode_pointers,
  page_type, validate_checksum,
};

#[derive(Debug, PartialEq, Eq)]
struct DecodedEntry {
  key: Vec<u8>,
  value: Vec<u8>,
  ts: u64,
  expire_ts: u64,
  overflow: bool,
  large_value: bool,
}

fn decode(blob: &[u8]) -> Vec<DecodedEntry> {
  let mut iter = DataPageIter::new(blob).unwrap();
  let mut entry_li = Vec::new();
  while iter.advance().unwrap() {
    assert_eq!(iter.compression(), CompressionType::None);
    assert!(iter.expire_ts() == 0 || iter.expire_ts() > iter.timestamp());
    entry_li.push(DecodedEntry {
      key: iter.key().to_vec(),
      value: iter.value().to_vec(),
      ts: iter.timestamp(),
      expire_ts: iter.expire_ts(),
      overflow: iter.is_overflow(),
      large_value: iter.is_large_value(),
    });
  }
  entry_li
}

fn want(
  key: &str,
  value: Vec<u8>,
  ts: u64,
  expire_ts: u64,
  overflow: bool,
  large_value: bool,
) -> DecodedEntry {
  DecodedEntry {
    key: key.as_bytes().to_vec(),
    value,
    ts,
    expire_ts,
    overflow,
    large_value,
  }
}

/// `fixtures/cpp/page/<case>/payload.bin` 逐 case 期望值（与 `cpp/oracle` 语料一致）。
fn expected_page(case: &str) -> Vec<DecodedEntry> {
  let mut out = Vec::new();
  match case {
    "empty" => {}
    "single" => out.push(want("k0", b"v0".to_vec(), 100, 0, false, false)),
    "multi" => {
      for i in 0..8u64 {
        out.push(want(
          &format!("k{i:02}"),
          format!("v{i:02}").into_bytes(),
          100 + i,
          0,
          false,
          false,
        ));
      }
    }
    "restart" => {
      for i in 0..40u64 {
        out.push(want(
          &format!("r{i:03}"),
          vec![b'x'; 16],
          100 + i,
          0,
          false,
          false,
        ));
      }
    }
    "ttl" => {
      for i in 0..4u64 {
        out.push(want(
          &format!("t{i:02}"),
          format!("v{i:02}").into_bytes(),
          100 + i,
          1000 + i,
          false,
          false,
        ));
      }
    }
    // `large` 是矩阵里的规范名；`large_values` 是 oracle 早期别名。
    "large" | "large_values" => {
      for i in 0..3u64 {
        out.push(want(
          &format!("L{i:02}"),
          vec![b'a' + i as u8; 1024],
          100 + i,
          0,
          false,
          true,
        ));
      }
    }
    "overflow" => {
      out.push(want("o00", b"v00".to_vec(), 100, 0, false, false));
      out.push(want(
        "o01",
        vec![7, 0, 0, 0, 8, 0, 0, 0],
        101,
        0,
        true,
        false,
      ));
      out.push(want("o02", b"v02".to_vec(), 102, 0, false, false));
    }
    other => panic!("unexpected cpp fixture case `{other}`"),
  }
  out
}

fn case_dir_li(kind: &str) -> Option<Vec<PathBuf>> {
  // 语料由 C++ oracle 生成（`fixtures/README.md` + `.github/workflows/oracle.yml`）。
  // 缺语料时默认跳过；CI 用 `FKV_REQUIRE_ORACLE_FIXTURES=1` 强制失败。
  common::case_dirs(kind)
}

#[test]
fn cpp_oracle_data_pages_decode_with_rust_reader() {
  let Some(case_li) = case_dir_li("page") else {
    return;
  };
  for dir in case_li {
    let case = dir.file_name().unwrap().to_string_lossy().into_owned();
    let bytes = fs::read(dir.join("payload.bin")).unwrap();
    // 页头 + checksum 必须能被 Rust 侧读回。
    assert_eq!(page_type(&bytes).unwrap(), PageType::Data, "case {case}");
    assert!(validate_checksum(&bytes).unwrap(), "case {case}");
    let page = DataPage::new(&bytes).unwrap();
    assert!(usize::from(page.content_len().unwrap()) <= bytes.len());

    let got_li = decode(&bytes);
    let want_li = expected_page(&case);
    assert_eq!(got_li, want_li, "case {case}");
    // 条目按 key 升序（page 内不变量）。
    for pair in got_li.windows(2) {
      assert!(pair[0].key < pair[1].key, "case {case} keys not ascending");
    }
    if case == "overflow" {
      // overflow 条目的 value 是指针数组，Rust 侧可独立解出。
      assert_eq!(
        decode_pointers(&got_li[1].value).unwrap(),
        vec![7, 8],
        "case {case}"
      );
    }
  }
}

struct WantIndexEntry {
  key: &'static [u8],
  page_id: u32,
}

fn expected_index(case: &str) -> (bool, u32, Vec<WantIndexEntry>) {
  match case {
    "leaf" => (
      true,
      1,
      vec![
        WantIndexEntry {
          key: b"i00",
          page_id: 11,
        },
        WantIndexEntry {
          key: b"i01",
          page_id: 12,
        },
        WantIndexEntry {
          key: b"i02",
          page_id: 13,
        },
        WantIndexEntry {
          key: b"i03",
          page_id: 14,
        },
      ],
    ),
    "branch" => (
      false,
      100,
      vec![
        WantIndexEntry {
          key: b"n00",
          page_id: 101,
        },
        WantIndexEntry {
          key: b"n01",
          page_id: 102,
        },
        WantIndexEntry {
          key: b"n02",
          page_id: 103,
        },
      ],
    ),
    other => panic!("unexpected cpp index case `{other}`"),
  }
}

#[test]
fn cpp_oracle_index_pages_decode_with_rust_reader() {
  let Some(case_li) = case_dir_li("index") else {
    return;
  };
  for dir in case_li {
    let case = dir.file_name().unwrap().to_string_lossy().into_owned();
    let bytes = fs::read(dir.join("payload.bin")).unwrap();
    assert!(
      matches!(
        page_type(&bytes).unwrap(),
        PageType::LeafIndex | PageType::NonLeafIndex
      ),
      "case {case}"
    );
    assert!(validate_checksum(&bytes).unwrap(), "case {case}");

    let (want_leaf, want_leftmost, want_li) = expected_index(&case);
    let page = IndexPage::new(&bytes).unwrap();
    assert_eq!(page.is_leaf().unwrap(), want_leaf, "case {case}");
    assert_eq!(page.leftmost_ptr().unwrap(), want_leftmost, "case {case}");
    assert!(
      usize::from(page.content_len().unwrap()) <= bytes.len(),
      "case {case}"
    );

    let mut iter = IndexPageIter::new(&bytes).unwrap();
    let mut got_li = Vec::new();
    while iter.advance().unwrap() {
      got_li.push((iter.key().to_vec(), iter.page_id()));
    }
    let want_li: Vec<(Vec<u8>, u32)> = want_li
      .iter()
      .map(|entry| (entry.key.to_vec(), entry.page_id))
      .collect();
    assert_eq!(got_li, want_li, "case {case}");
  }
}
