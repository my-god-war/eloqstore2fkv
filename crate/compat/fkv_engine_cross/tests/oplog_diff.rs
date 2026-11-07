//! 同一份文本 op log 在 Rust 侧的执行 + 规范化 dump，用于和 C++ driver 的
//! `dump` 做逐字节 diff。
//!
//! 环境变量（由 `engine.yml` 设置）：
//!   FKV_OPLOG       输入 op log（`put key value ts` / `del key ts`）
//!   FKV_RUST_STORE  新建的 Rust store 目录
//!   FKV_RUST_DUMP   输出 dump 文件
//!
//! 用例带 `#[ignore]`，常规 `cargo test --workspace` 报 ignored 而不是假绿；
//! 证据运行（CI `engine.yml`）显式 `--ignored`，此时缺 env 必须失败。

use std::{
  env,
  fs::{read_to_string, remove_dir_all, write},
  path::PathBuf,
};

use fkv_btree::BatchEntry;
use fkv_io_compio::CompioStore;
use fkv_store::{AllocMode, Store};
use fkv_types::{DEFAULT_PAGES_PER_FILE_SHIFT, TableIdent};

const PAGE_SIZE: usize = 4096;
const RESTART_INTERVAL: u16 = 16;

fn hex(bytes: &[u8]) -> String {
  let mut out = String::with_capacity(bytes.len() * 2);
  for byte in bytes {
    out.push_str(&format!("{byte:02x}"));
  }
  out
}

/// 用例是 `#[ignore]` 的：只有显式 `--ignored`（证据运行）才会进来。此时
/// env 缺失意味着证据链断了，必须失败而不是静默通过。
fn required_env(name: &str) -> String {
  env::var(name).unwrap_or_else(|_| {
    panic!("op-log diff 需要 {name}（由 engine.yml 的 op-log 步骤设置）；常规测试应看到 ignored")
  })
}

#[compio::test]
#[ignore = "需要 FKV_OPLOG / FKV_RUST_STORE / FKV_RUST_DUMP；由 engine.yml 以 --ignored 运行"]
async fn rust_oplog_dump() {
  let oplog = required_env("FKV_OPLOG");
  let store_root = required_env("FKV_RUST_STORE");
  let dump_path = required_env("FKV_RUST_DUMP");
  let store_root = PathBuf::from(store_root);
  let _ = remove_dir_all(&store_root);
  std::fs::create_dir_all(&store_root).unwrap();

  let tbl = TableIdent::new("demo", 1).unwrap();
  let io = CompioStore::new(store_root, PAGE_SIZE, DEFAULT_PAGES_PER_FILE_SHIFT);
  let alloc_mode = match env::var("FKV_ENGINE_APPEND_MODE").as_deref() {
    Ok("") | Ok("0") | Err(_) => AllocMode::Pooled,
    Ok(_) => AllocMode::Append,
  };
  let mut store = Store::new(io, PAGE_SIZE, RESTART_INTERVAL).with_alloc_mode(alloc_mode);

  for (line_no, line) in read_to_string(&oplog).unwrap().lines().enumerate() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    let parts = line.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
      ["put", key, value, ts] => {
        let ts = ts
          .parse::<u64>()
          .unwrap_or_else(|err| panic!("{oplog}:{} put ts 非法: {err}", line_no + 1));
        store
          .put(
            &tbl,
            &[BatchEntry::upsert(
              key.as_bytes().to_vec(),
              value.as_bytes().to_vec(),
              ts,
            )],
          )
          .await
          .unwrap_or_else(|err| panic!("{oplog}:{} put 失败: {err}", line_no + 1));
      }
      ["del", key, ts] | ["delete", key, ts] => {
        let ts = ts
          .parse::<u64>()
          .unwrap_or_else(|err| panic!("{oplog}:{} del ts 非法: {err}", line_no + 1));
        store
          .delete(&tbl, key.as_bytes(), ts)
          .await
          .unwrap_or_else(|err| panic!("{oplog}:{} del 失败: {err}", line_no + 1));
      }
      _ => panic!("{oplog}:{} 无法解析: {line}", line_no + 1),
    }
  }

  let mut dump = String::new();
  for (key, value) in store.scan(&tbl, b"", b"", 0).await.unwrap() {
    dump.push_str(&hex(&key));
    dump.push('\t');
    dump.push_str(&hex(&value));
    dump.push('\n');
  }
  write(dump_path, dump).unwrap();
}
