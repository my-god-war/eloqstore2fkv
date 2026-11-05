//! format 层逐字节金标：Rust builder 输出必须与固定 oracle 字节一致。
//!
//! 金标由 `cpp/oracle` 的 `DataPageBuilder::Finish()` 产出（NDEBUG），
//! 基线见仓库根 `COMPAT_BASELINE`。

use std::str::from_utf8;

use fkv_page::{CompressionType, DataPageBuilder, EntrySpec, IndexPageBuilder, PageType};

/// data page：5 条 entry，restart interval = 2。
#[test]
fn data_page_bytes_match_oracle() {
  const GOLDEN: &str = "0000000000000000023f0000000000000000000001106131c8010001106232280001126333e707dc010001116434b40100011c6535d8041300200030000300";
  const PAGE_SIZE: usize = 512;
  const RESTART_INTERVAL: u16 = 2;
  let items = [
    (b"a".as_slice(), b"1".as_slice(), 100u64, 0u64, false, false),
    (b"b".as_slice(), b"2".as_slice(), 120, 0, false, false),
    (b"c".as_slice(), b"3".as_slice(), 110, 999, false, false),
    (b"d".as_slice(), b"4".as_slice(), 200, 0, true, false),
    (b"e".as_slice(), b"5".as_slice(), 300, 0, false, true),
  ];
  let mut builder = DataPageBuilder::new(PAGE_SIZE, RESTART_INTERVAL).unwrap();
  for (key, value, ts, expire, overflow, large) in items {
    assert!(
      builder
        .add(EntrySpec {
          key,
          value,
          ts,
          expire_ts: expire,
          overflow,
          compression: CompressionType::None,
          large_value: large,
        })
        .unwrap()
    );
  }
  let page = builder.finish_without_checksum().unwrap();
  assert_eq!(page.len(), PAGE_SIZE);

  let golden = hex(GOLDEN);
  assert_eq!(
    usize::from(fkv_page::content_len(&page).unwrap()),
    golden.len()
  );
  assert_eq!(
    &page[..golden.len()],
    golden.as_slice(),
    "data page 与 oracle 逐字节不一致"
  );
  assert!(
    page[golden.len()..].iter().all(|&b| b == 0),
    "padding 必须为 0"
  );
}

/// index leaf：`Add("",10,true)` 起 5 条。
#[test]
fn index_page_bytes_match_oracle() {
  const GOLDEN: &str = "00000000000000000125000a000000000162280001631400016450000165140f0017000200";
  let mut builder = IndexPageBuilder::new(512, 2, true).unwrap();
  assert!(builder.add(b"", 10).unwrap());
  for (key, page_id) in [(b"b".as_slice(), 20u32), (b"c", 30), (b"d", 40), (b"e", 50)] {
    assert!(builder.add(key, page_id).unwrap());
  }
  let page = builder.finish_without_checksum().unwrap();
  assert_eq!(page.len(), 512);
  assert_eq!(fkv_page::page_type(&page).unwrap(), PageType::LeafIndex);

  let golden = hex(GOLDEN);
  assert_eq!(
    usize::from(fkv_page::content_len(&page).unwrap()),
    golden.len()
  );
  assert_eq!(
    &page[..golden.len()],
    golden.as_slice(),
    "index page 与 oracle 逐字节不一致"
  );
  assert!(
    page[golden.len()..].iter().all(|&b| b == 0),
    "padding 必须为 0"
  );
}

fn hex(text: &str) -> Vec<u8> {
  text
    .as_bytes()
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| u8::from_str_radix(from_utf8(pair).unwrap(), 16).unwrap())
    .collect()
}
