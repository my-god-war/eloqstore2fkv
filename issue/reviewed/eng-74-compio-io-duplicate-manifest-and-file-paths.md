# eng-74 compio IO 层四对函数逐行重复：manifest append/atomic write、read_file/file_size、data/segment list/remove、direct/buffered 读写

## 位置

`crate/io/fkv_io_compio/src/lib.rs` 内存着四组近乎逐行相同的实现：

| 组 | 两份实现 | 差异 |
|---|---|---|
| manifest 追加 | `append_manifest` `:1427-1460` / `append_manifest_file` `:1516-1549` | 仅路径来源（`manifest_path(tbl)` vs `dir.join(name)`） |
| manifest 原子写 | `switch_manifest` `:1462-1479` / `write_manifest_file` `:1497-1514` | 仅路径与 tmp 名来源 |
| 整文件读 / 尺寸 | `read_file` `:1830-1850` / `file_size` `:1864-1892` | 仅最后一步（`read_whole` vs `metadata`） |
| data / segment 列举与删除 | `list_file_id_li` `:1894-1913` / `list_segment_file_id_li` `:1932-1950`；`remove_file` `:1915-1930` / `remove_segment_file` `:1952-1967` | 仅选 `index.data_*` 还是 `index.segment_*`、调 `remove_file` 还是 `remove_segment_file` |
| 页读 / 页写 | `read_page` `:1159-1245` 内部 direct / buffered 两套分支（`:1190-1217` vs `:1218-1243`）；`write_page` `:1247-1285` 同样两套（`:1259-1274` vs `:1275-1285`） | 仅缓冲类型与 `write_at` 入参不同 |

## 现状

- 每组两份的公共部分占 80%~95%：
  - append：`ensure_dir` → `open_write_file` → 查 `manifest_offset` → 取写缓冲 → `write_at` → 更新偏移 → failpoint → `sync_data` → `sync_manifest_dir_once`。
  - 原子写：`File::create(tmp)` → `write_whole` → `sync_data` → failpoint → `rename` → `forget_fd` → `sync_dir`。
  - 读文件：`cached_data_path` → `open_read_file` → `NotFound` 时 `forget_path` + 重查一次 → 读/取 metadata。
- 维护成本已经可见：任何一处 failpoint 名、fsync 顺序、偏移缓存更新逻辑修改，都要在两份里同步；`eng-20`（manifest 撕裂尾自愈）新增的 `reset_manifest` 也走同一偏移缓存，后续若加 manifest 重试/校验，重复面继续扩大。
- `read_page` / `write_page` 的 direct/buffered 分支把 checksum、短读重试、池归还各写一遍；`WriteChunk` 枚举（`:307-310`）已经抽象了“两种缓冲”，但只用于 `write_page_li`，读路径与单页写路径没有复用。
- 这些函数在页 IO 热路径上，重复代码本身不直接降性能，但让“改一处漏一处”的概率随 IO 策略演进（O_DIRECT、provided buffer、超时）线性上升。

## C++ 实现

C++ 把路径差异参数化，而不是复制函数体：
- `IouringMgr::AppendManifest(tbl_id, log, offset)`（`src/async_io_manager.cpp:2791-2870`）一份实现，活跃 manifest 与 branch manifest 只由调用方传入不同的 `(branch, term)`/offset。
- `OpenFD` / `OpenOrCreateFD`（`src/async_io_manager.cpp:1584-1600` 一带）统一处理读/写/创建三种打开方式。
- `WriteFile` / `ReadFile` 以 `flags` 与 fd 封装差异，页读写共用同一提交路径。

## 建议

1. 抽出三个私有 helper：
   ```rust
   async fn append_file_at(&self, path: &Path, record: &[u8]) -> Result<()>
   async fn atomic_write_file(&self, dir: &Path, path: &Path, record: &[u8]) -> Result<()>
   async fn open_existing_or_retry(&self, tbl, file_id, segment: bool) -> Result<Option<File>>
   ```
   `append_manifest` / `append_manifest_file`、`switch_manifest` / `write_manifest_file`、
   `read_file` / `file_size` 都变成一行路径选择 + 调用。
2. 用 `enum FileKind { Data, Segment }`（已有 `Access` 的风格）参数化目录索引选择与删除方法，
   把 `list_*` / `remove_*` 各收敛成一个泛型 helper；`FileKind` 是编译期枚举，不引入 dyn。
3. 页读写：把 direct / buffered 的差异收敛到 `WriteChunk` 式的 `enum PageBuf { Plain(Vec<u8>), Direct(DirectBuf) }`，
   并让 `read_page` 用同一个 `PageBuf`；checksum、短读/短写校验、池归还只在枚举外层写一次。
4. failpoint 名称保留在公共 helper 内，避免两份实现各写一套字符串。

## 严重度

P2（可维护性；四组重复覆盖 manifest 持久化与页 IO 主路径，策略演进时易漏改）

## 核验记录（2026-11-04）

按当前源码（并行会话合入 `reset_manifest` 后）重新取行号：`append_manifest` `:1427-1460`、
`append_manifest_file` `:1516-1549`、`switch_manifest` `:1462-1479`、`write_manifest_file` `:1497-1514`、
`read_file` `:1830-1850`、`file_size` `:1864-1892`、`list_file_id_li` `:1894-1913`、
`list_segment_file_id_li` `:1932-1950`、`remove_file` `:1915-1930`、`remove_segment_file` `:1952-1967`。
逐段对照确认公共行一致，仅路径/读取动作为差异。

## 审查结论（2026-11-04）

核验通过，五组重复全部成立，行号与源码一致：

- `append_manifest` / `append_manifest_file`：仅 `dir` / `path` 来源不同，failpoint、偏移缓存、fsync 顺序逐行相同。
- `switch_manifest` / `write_manifest_file`：tmp 名分别为 `manifest_main_0.tmp` 与 `{name}.tmp`，实为「路径追加 `.tmp`」，其余完全一致。
- `read_file` / `file_size`：`cached_data_path` → `open_read_file` → `NotFound` 时 `forget_path` + 重查一次的整段逻辑重复。
- `list_file_id_li` / `list_segment_file_id_li`、`remove_file` / `remove_segment_file`：仅 `bool segment` 选择的索引 / 删除入口不同。
- `read_page` / `write_page` 的 direct / buffered 分支：重试匹配、缓冲归还、checksum 校验各写两遍。

一处需修正 issue 的描述：`write_page_li` 的 Plain 分支用 `vec![0u8; bytes]` 且不归还写池，与 `write_page` 的 `take_write_buf` 不同，不是「仅缓冲类型与 write_at 入参不同」。合并时统一走 `WriteChunk` 并归还写池（仅分配策略变化，error / 重试 / fsync / 对齐语义不变）。

拆分：74a manifest 追加与原子替换、74b 读文件/尺寸与 FileKind 列举删除、74c 页读写缓冲收敛。

## DONE（2026-11-04）

- 74a：manifest 追加 / 原子写收敛（`append_file_at`、`atomic_write_file`、`tmp_path`）。
- 74b：`read_file` / `file_size` 走 `open_data_file`；`FileKind` 参数化目录索引与 `list` / `remove`。
- 74c：页读 `ReadChunk` + `read_page_chunk` / `put_read_chunk`，页写 `write_chunk` 统一 direct / buffered。
- 验证：`cargo nextest run -p fkv_io_compio` 29 passed；`cargo clippy -p fkv_io_compio --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（拆分总纲，三个子项 74a/74b/74c 均已独立复核通过，见各自 r6 小节）。

- `append_file_at`/`atomic_write_file`/`tmp_path`（74a）、`FileKind`/
  `open_data_file`/`list_file_id_by_kind`/`remove_file_by_kind`（74b）、
  `ReadChunk`/`read_page_chunk`/`put_read_chunk`/`write_chunk`（74c）
  全部在 `fkv_io_compio/src/lib.rs` 落地，行号见子项小节。
