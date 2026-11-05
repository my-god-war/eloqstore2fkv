# eng-67 reopen / attach 全量丢弃 IndexCache：缺 C++ 按 file_page_id 不变复用缓存页（swizzle 复用）的路径

## 位置
- Rust: `crate/engine/fkv_store/src/lib.rs:1261-1263`（`install_replay_state` 无条件 `index_cache_map.insert(tbl, RefCell::new(IndexCache::default()))`）；`crate/core/fkv_btree/src/lib.rs:88-92`（`IndexNode` 只存 `leftmost + entries`，不记录 `file_page_id`，结构上不支持复用校验）
- C++: `src/storage/page_manager.cpp:605-625`（`InstallExternalSnapshot`："Reuse swizzling pointers when file_page_id is unchanged" —— 遍历 `meta.cached_pages_`，`DecodeId(val) == page->GetFilePageId()` 时 `AddSwizzling` 把旧缓存页重新挂回新 mapping）

## 现状
auto-reopen（`fkv_shard/src/lib.rs:285`）、表驱逐后再访问、standby/云 refresh manifest 全部整树冷启动。

## C++ 实现
物理页未变的缓存全部保活，只对真正变更的页失缓存。

## 差异与风险
standby/云模式周期性 refresh 后，Rust 点读从 0 IO 退化到 O(depth) IO；频繁 evict+reopen 的多表读负载命中雪崩。与 core-13（`fkv_cache::PageCache` 模块未接线）不重复 —— 本条是 **IndexCache 在 reopen 的保留策略**。

## 对齐建议
- `CacheEntry`/`IndexNode` 增加 `file_page_id`；
- `install_replay_state` 对旧缓存逐条用新 mapping 的 `file_page_id(page_id)` 校验：相等保留，不等丢弃（等价 C++ swizzle 复用）。

## 严重度
P2（standby/云 refresh 场景为 P1；本地 auto-reopen 频率低）

## 核验记录（2026-11-04）
C++ swizzle 复用循环实读确认（`page_manager.cpp:605-625`）；Rust 无条件 default() 实读确认。

## 审查结论（2026-11-04）

判定：**合理，已实现**。

现状复核：`install_replay_state` 无条件 `state.index_cache = Some(RefCell::new(new_index_cache()))`
（原 `fkv_store/src/lib.rs:1510/1528` 两次建缓存，后一次覆盖前一次），并注释「重放后页内容
可能被原地重写、或文件被 standby 同步 rename 替换：旧缓存条目指向的 page 布局已失效，必须
整表丢弃」。该保守做法确实把 standby / 云 refresh 的每次 reopen 都退化成整树冷启动。

为什么可以安全复用（判定依据，非猜测）：

- `STORE_ALLOC_MODE = AllocMode::Append`（`fkv_store/src/lib.rs:67` 编译期断言拒绝 Pooled）：
  `AppendAllocator::free` 为 no-op，物理页只分配一次、写一次，写后内容不变。
- 写路径更新逻辑页一律 `remap_page`（新 `FilePageId`），因此「逻辑 PageId → fpid 未变」等价
  「该 index 页字节未变」。index entry 里存的是逻辑 PageId，不是 fpid（compact 只换 fpid，
  正是 eng-68 保留缓存的依据）。
- 跨分支 / 跨 term 的 file id 空间不可比：standby 的 data / segment 文件只追加、manifest
  term 由 fencing 校验（`check_manifest_term`），同一 branch+term 下同名文件已写前缀不变。
  因此复用条件取 `(branch, term, file_page_id)` 三元组同一。

不复算页内容哈希：内容哈希需要 reopen 时 O(n) 读盘（`install_replay_state` 是同步函数，
且全量回读的代价高于按需重读整树），会直接抵消复用收益；append-only 不变量已能由
fpid 相等推出内容相等。故采用「版本（branch, term）+ fpid」校验，比哈希更便宜且同样可验证。

实现：

- `fkv_btree`：`IndexNode` 增 `file_page_id`（`IndexNode::new(file_page_id, page)`）；
  新增 `IndexCache::retain_mapped(resolve)`，逐条用新映射裁剪，丢弃换页 / 已释放条目，
  同时压实淘汰队列与 `bytes`。
- `fkv_task_read` / `fkv_task_scan`：4 处缓存回填点传入实际读取的 `file_page_id`。
- `fkv_store`：`TableState` 增 `cache_branch` / `cache_term`；新增 `ensure_index_cache`
  统一登记缓存身份；`install_replay_state` 在 `!bump` 且身份一致时保留旧缓存并
  `retain_mapped(|page_id| mapper.file_page_id(page_id).ok())`，否则整表重建。

验收与结果：

- 新增测试：`reopen_reuses_index_cache_when_file_page_ids_unchanged`（复用后读已 warm key
  仅 1 次 data 页 IO）、`reopen_drops_index_cache_when_term_changes`、
  `fkv_btree::retain_mapped_drops_remapped_and_missing_pages`。
- `cargo nextest run -p fkv_btree -p fkv_store -p fkv_runtime --no-fail-fast`：151 passed, 1 skipped。
- `cargo clippy -p fkv_btree -p fkv_store --no-deps --all-features --all-targets -- -D warnings`
  零警告（`fkv_task_read` / `fkv_task_scan` 同口径亦零警告）。

遗留：

- compact 搬迁后 `clear_changed_page_li` 丢弃 remap 记录，缓存条目里的 fpid 会变旧；
  下一次 reopen 时这些条目被保守丢弃（内容本身仍正确），属可接受的少复用。
- 分支切换仍整表丢弃（`cache_branch` 不一致），未做跨分支 fpid 比对；跨分支 file id
  空间不同，本就不该复用。

## DONE（2026-11-04）

已落地上述实现与测试，issue 归档至 `issue/done/`。

## 复审（2026-11-05 r6）

通过。逐项核验（行号随 27b 演进有漂移，语义完整保留）：

- `fkv_btree/src/lib.rs:82-86`：`IndexNode { leftmost, file_page_id, page }`；
  `:452`（`retain_mapped`）、`:432`（`retag`，27b 代际配套）。
- `fkv_store/src/lib.rs:169-171`（`TableState::cache_branch/cache_term`）、
  `:1790-1811`（`install_replay_state`：`same_identity = !bump && (branch, term) 一致`
  时保留旧缓存并 `retain_mapped(|page_id| mapper.file_page_id(page_id).ok())`，
  否则整表重建；bump = term 变化按新文件边界起步）、`:1830-1831`（登记缓存身份）。
- 回填传 fpid：`fkv_task_read/src/lib.rs:246`（`IndexNode::new(file_page_id, page)`）。
- 测试：`fkv_store/tests/index_cache.rs:266`（`reopen_reuses_index_cache_when_file_page_ids_unchanged`，
  断言 reopen 后缓存条目数不变且 warm key 只 1 次 data 页 IO）、`:301`
  （`reopen_drops_index_cache_when_term_changes`，断言缓存归零）。
