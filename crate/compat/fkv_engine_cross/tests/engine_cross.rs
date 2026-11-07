//! 引擎级交叉：C++ EloqStore 写 → Rust 打开/读 → Rust 续写 → C++ 读回。
//!
//! 需要 `FKV_CPP_STORE` 指向 `cpp/engine_driver write` 生成的真实 store 目录。
//! 用例带 `#[ignore]`，常规 `cargo test --workspace` 报 ignored 而不是假绿；
//! 证据运行（CI `engine.yml`）显式 `--ignored`，此时缺 env 必须失败。

use std::{env, path::PathBuf, sync::Mutex};

use fkv_btree::BatchEntry;
use fkv_io::PageIo;
use fkv_io_compio::CompioStore;
use fkv_store::{AllocMode, Store};
use fkv_types::{DEFAULT_PAGES_PER_FILE_SHIFT, TableIdent};

const PAGE_SIZE: usize = 4096;
const RESTART_INTERVAL: u16 = 16;
const CPP_COUNT: usize = 64;
const RUST_COUNT: usize = 16;
/// 第二张表 `other.1`（多表交叉）：C++ `write ... other.1 o 16 other`，
/// Rust 续写 `p0000..p0007`。
const OTHER_CPP_COUNT: usize = 16;
const OTHER_RUST_COUNT: usize = 8;
/// 4 KiB 页放不下的 value：由引擎写成 overflow 页链（约 26 页 / 条）。
const OVERFLOW_SIZE: usize = 102_400;
const OVERFLOW_COUNT: usize = 4;

/// 两个用例共用 `FKV_CPP_STORE` 指向的同一目录（C++ write → Rust → C++ verify
/// 是一条链），必须串行执行，否则并发 reopen / 续写会互相打断。
static STORE_LOCK: Mutex<()> = Mutex::new(());

fn key(prefix: char, index: usize) -> Vec<u8> {
  format!("{prefix}{index:04}").into_bytes()
}

/// 用例是 `#[ignore]` 的：只有显式 `--ignored`（证据运行）才会进来。此时
/// `FKV_CPP_STORE` 缺失意味着证据链断了，必须失败而不是静默通过。
fn cpp_store() -> PathBuf {
  match env::var_os("FKV_CPP_STORE") {
    Some(path) => PathBuf::from(path),
    None => panic!(
      "引擎级交叉需要 FKV_CPP_STORE（engine_driver write 生成的 store 目录）；\
       常规测试应看到 ignored，证据运行请按 engine.yml 设置环境变量"
    ),
  }
}

fn tbl() -> TableIdent {
  TableIdent::new("demo", 1).unwrap()
}

/// 与 `demo.1` 同一个 store root 上的第二张表，覆盖多表场景。
fn tbl_other() -> TableIdent {
  TableIdent::new("other", 1).unwrap()
}

/// 分配模式与 C++ driver 的 `ENGINE_APPEND_MODE` 对齐：默认 pooled，设置
/// `FKV_ENGINE_APPEND_MODE=1` 走 append。
fn alloc_mode() -> AllocMode {
  match env::var("FKV_ENGINE_APPEND_MODE").as_deref() {
    Ok("") | Ok("0") | Err(_) => AllocMode::Pooled,
    Ok(_) => AllocMode::Append,
  }
}

#[compio::test]
#[ignore = "需要 FKV_CPP_STORE（engine_driver 生成的真实 store）；由 engine.yml 以 --ignored 运行"]
#[allow(clippy::await_holding_lock)]
async fn cpp_write_rust_read_then_rust_write() {
  let root = cpp_store();
  let _guard = STORE_LOCK.lock().expect("store 锁被 poison");
  let tbl = tbl();
  let io = CompioStore::new(root, PAGE_SIZE, DEFAULT_PAGES_PER_FILE_SHIFT);
  io.set_file_identity("main", 0);
  io.preload_path_li(&tbl);
  let mut store = Store::new(io, PAGE_SIZE, RESTART_INTERVAL).with_alloc_mode(alloc_mode());
  store.reopen(&tbl).await.expect("Rust 打开 C++ store 失败");

  for index in 0..CPP_COUNT {
    let key = key('c', index);
    let value = store
      .get(&tbl, &key)
      .await
      .unwrap_or_else(|err| panic!("Rust 读 C++ key {key:?} 失败: {err}"));
    assert_eq!(
      value.as_deref(),
      Some(b"cpp".as_slice()),
      "Rust 读 C++ key {key:?} 值不一致"
    );
  }

  // Rust 续写：C++ 侧随后用 verify 读回。
  let batch = (0..RUST_COUNT)
    .map(|index| BatchEntry::upsert(key('r', index), b"rust".to_vec(), 2))
    .collect::<Vec<_>>();
  store.put(&tbl, &batch).await.expect("Rust 续写失败");
  for index in 0..RUST_COUNT {
    assert_eq!(
      store.get(&tbl, &key('r', index)).await.unwrap().as_deref(),
      Some(b"rust".as_slice())
    );
  }
}

/// 第二张表 `other.1` 的引擎级交叉：C++ 写 → Rust 打开/读 → Rust 续写 → C++ 读回。
///
/// 其余用例只覆盖 `demo.1`；本用例在**同一个 `FKV_CPP_STORE` root** 上再开一张
/// 表，验证多表场景下 C++ / Rust 的表目录与 manifest 布局互操作：C++ 先
/// `write "$STORE" other.1 o 16 other`，Rust 续写 `p0000..p0007 = "rust2"`，
/// 随后 C++ 侧 `verify ... other.1 o 16 other` / `other.1 p 8 rust2` 读回。
#[compio::test]
#[ignore = "需要 FKV_CPP_STORE（engine_driver 生成的真实 store）；由 engine.yml 以 --ignored 运行"]
#[allow(clippy::await_holding_lock)]
async fn cpp_write_rust_read_second_table() {
  let root = cpp_store();
  let _guard = STORE_LOCK.lock().expect("store 锁被 poison");
  // engine_driver 每个进程只开一张表；本用例为第二张表单独建一套
  // CompioStore / Store，复用现有用例的构造顺序（branch main、term 0、
  // preload_path_li 后再 reopen），不依赖单 Store 多表共存。
  let tbl = tbl_other();
  let io = CompioStore::new(root, PAGE_SIZE, DEFAULT_PAGES_PER_FILE_SHIFT);
  io.set_file_identity("main", 0);
  io.preload_path_li(&tbl);
  let mut store = Store::new(io, PAGE_SIZE, RESTART_INTERVAL).with_alloc_mode(alloc_mode());
  store
    .reopen(&tbl)
    .await
    .expect("Rust 打开 C++ store 的 other.1 失败");

  // C++ `write "$STORE" other.1 o 16 other` 写入的语料。
  for index in 0..OTHER_CPP_COUNT {
    let key = key('o', index);
    let value = store
      .get(&tbl, &key)
      .await
      .unwrap_or_else(|err| panic!("Rust 读 C++ other.1 key {key:?} 失败: {err}"));
    assert_eq!(
      value.as_deref(),
      Some(b"other".as_slice()),
      "Rust 读 C++ other.1 key {key:?} 值不一致"
    );
  }

  // Rust 续写第二张表：C++ 侧随后用 `verify "$FKV_STORE" other.1 p 8 rust2` 读回。
  let batch = (0..OTHER_RUST_COUNT)
    .map(|index| BatchEntry::upsert(key('p', index), b"rust2".to_vec(), 2))
    .collect::<Vec<_>>();
  store
    .put(&tbl, &batch)
    .await
    .expect("Rust 续写 other.1 失败");
  for index in 0..OTHER_RUST_COUNT {
    assert_eq!(
      store.get(&tbl, &key('p', index)).await.unwrap().as_deref(),
      Some(b"rust2".as_slice())
    );
  }
}

/// overflow / 大 value 双向：C++ 写 4×102400 B 'x'（4 KiB 页装不下，落 overflow
/// 页链）→ Rust 逐条读并校验 → Rust 续写 4×102400 B 'y' → C++ `verify_repeat` 读回。
#[compio::test]
#[ignore = "需要 FKV_CPP_STORE（engine_driver 生成的真实 store）；由 engine.yml 以 --ignored 运行"]
#[allow(clippy::await_holding_lock)]
async fn cpp_overflow_rust_read_then_rust_write() {
  let root = cpp_store();
  let _guard = STORE_LOCK.lock().expect("store 锁被 poison");
  let tbl = tbl();
  let io = CompioStore::new(root, PAGE_SIZE, DEFAULT_PAGES_PER_FILE_SHIFT);
  io.set_file_identity("main", 0);
  io.preload_path_li(&tbl);
  let mut store = Store::new(io, PAGE_SIZE, RESTART_INTERVAL).with_alloc_mode(alloc_mode());
  store
    .reopen(&tbl)
    .await
    .expect("Rust 打开 C++ overflow store 失败");

  // 本地只跑 `engine_driver write` 基本语料时没有 overflow 语料；没有 L0000
  // 时明确跳过。CI（`engine.yml`）设置 `FKV_REQUIRE_OVERFLOW=1`，缺语料失败。
  if store
    .get(&tbl, &key('L', 0))
    .await
    .expect("Rust 探测 overflow 语料失败")
    .is_none()
  {
    assert!(
      env::var("FKV_REQUIRE_OVERFLOW").as_deref() != Ok("1"),
      "overflow 交叉缺少 L0000..L0003（需 engine_driver write_repeat）；\
       FKV_REQUIRE_OVERFLOW=1 时不允许跳过"
    );
    eprintln!("跳过 overflow 交叉：store 中没有 L0000（先用 write_repeat 生成）");
    return;
  }

  // C++ 侧由 write_repeat 写入 L0000..L0003。
  for index in 0..OVERFLOW_COUNT {
    let key = key('L', index);
    let value = store
      .get(&tbl, &key)
      .await
      .unwrap_or_else(|err| panic!("Rust 读 C++ overflow key {key:?} 失败: {err}"))
      .unwrap_or_else(|| panic!("Rust 读 C++ overflow key {key:?} 返回 None"));
    assert_eq!(
      value.len(),
      OVERFLOW_SIZE,
      "Rust 读 C++ overflow key {key:?} 长度不一致"
    );
    assert!(
      value.iter().all(|byte| *byte == b'x'),
      "Rust 读 C++ overflow key {key:?} 内容不是全 'x'"
    );
  }

  // Rust 续写 overflow：C++ 侧随后用 `verify_repeat ... M 4 102400 y` 读回。
  let batch = (0..OVERFLOW_COUNT)
    .map(|index| BatchEntry::upsert(key('M', index), vec![b'y'; OVERFLOW_SIZE], 2))
    .collect::<Vec<_>>();
  store
    .put(&tbl, &batch)
    .await
    .expect("Rust 续写 overflow 失败");
  for index in 0..OVERFLOW_COUNT {
    let key = key('M', index);
    let value = store
      .get(&tbl, &key)
      .await
      .unwrap()
      .unwrap_or_else(|| panic!("Rust 读回自己写的 overflow key {key:?} 返回 None"));
    assert_eq!(value.len(), OVERFLOW_SIZE);
    assert!(value.iter().all(|byte| *byte == b'y'));
  }
}

#[test]
#[ignore = "需要 FKV_CPP_STORE（engine_driver 生成的真实 store）；由 engine.yml 以 --ignored 运行"]
fn cpp_store_root_is_not_empty() {
  let root = cpp_store();
  assert!(root.is_dir(), "FKV_CPP_STORE 不是目录: {}", root.display());
}
