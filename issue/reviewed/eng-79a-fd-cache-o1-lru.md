# eng-79a FdCache 换成索引式侵入链表 LRU（O(1)）

## 位置

- `crate/io/fkv_io_compio/src/lib.rs:52` `FD_CACHE_LIMIT`、`:128-138` `FdCache::default`、
  `:175-215` `get_read` / `get_write` / `insert`、`:249-280` `evict_oldest`。

## 方案

- 用 `Vec<Option<FdNode>>` 槽位数组 + 空闲槽位栈，`prev` / `next` 为 `Option<usize>` 索引，
  不引入 unsafe、不引入 dyn。
- `FdNode { path, access, file, identity, prev, next }`；read / write 共用一条 LRU 链，
  淘汰只看全局 `len()`，因此 `limit` 语义与现在一致（read+write 合计）。
- 两个 `HashMap<PathBuf, usize>` 分别索引 read / write 句柄，命中即 `unlink + push_front`。
- `insert` 同 `(path, access)` 已存在时原位替换 `file` / `identity` 并移到队首，
  不增长 len（保持现有覆盖语义）。
- `evict_oldest` 改为 `pop_tail`：O(1)，删除 `min_by_key` 与二次 `get`。
- `remove` / `forget_under` / `clear` 同步维护链表与索引。
- `FD_CACHE_LIMIT` 从 256 改为 10_000，对齐 `fkv_config::KvOption::fd_limit` 默认值。

## 验收

- `cargo check -p fkv_io_compio` 通过，`cargo clippy -p fkv_io_compio ... -D warnings` 零警告。
- `cargo nextest run -p fkv_io_compio` 全绿，含 `fd_cache_limit_is_configurable`。
- `fd_cache_len()` 恒 `<= limit`；重复访问已缓存路径不增加 `fd_open_num()`。

## DONE（2026-11-04）

实现要点（`crate/io/fkv_io_compio/src/lib.rs`）：

- `CachedFd` 换为 `FdNode { path, access, file, identity, prev, next }`；
  `FdCache` 改为槽位数组 `nodes: Vec<Option<FdNode>>` + `free_li` 空闲索引栈 +
  read / write 两张 `HashMap<PathBuf, usize>` + `head` / `tail`，单条全局 LRU 链。
- 命中 `touch`：索引查表 -> `unlink` -> `push_front`，O(1)。
- 插入：同 `(路径, 模式)` 原位替换并移到队首；新节点头插；超限循环 `pop_tail`
  （`unlink + release`），每次 O(1)。
- `remove` / `forget_under` / `clear` 同步维护链表、索引与空闲栈，无残留槽位。
- 删除 `clock` / `used` / `min_by_key` / 二次 `get` 防御；`FD_CACHE_LIMIT` 由 256
  改为 10_000（对齐 `KvOption::fd_limit` 默认值）。

验证：

- `cargo check -p fkv_io_compio`：通过。
- `cargo nextest run -p fkv_io_compio`：全绿，含 `fd_cache_limit_is_configurable`、
  `fd_cache_is_bounded`（显式 `with_fd_cache_limit(256)` 保持原断言）。
- clippy：0 warning。

## 复审（2026-11-05 r6）

通过。

- `fkv_io_compio/src/lib.rs:186 FdNode`（prev/next 双向链）、`:199 FdCache`
  （`nodes: Vec<Option<FdNode>>` + `free_li: Vec<usize>` 空闲栈 + 双 HashMap
  索引）、`:283 push_front`、`:324 touch`（查表→unlink→push_front O(1)）、
  `:302/:320` 槽位分配/回收；全文无 `min_by_key`/`clock` 线性扫描残留。
- `FD_CACHE_LIMIT = 10_000`（`:54-56`）；`tests/hardening.rs:226
  fd_cache_limit_is_configurable`（≤2 断言）、`:259
  fd_cache_evicts_and_reuses_under_limit`（limit=4 + 重复命中 open 数不变）。
