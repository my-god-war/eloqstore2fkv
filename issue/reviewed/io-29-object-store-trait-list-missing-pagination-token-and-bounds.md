# io-29 fkv_object ObjectStore::list 无 continuation token / max_keys / recursive 语义，大 bucket 一次全量物化

## 位置
- Rust: `crate/io/fkv_object/src/lib.rs:34`（`fn list(prefix) -> Vec<ObjectMeta>`，无分页游标、无数量上限、无目录递归语义）、`:37-115`（`MemObjectStore::list` 全表过滤 + 排序返回）、`crate/io/fkv_object_s3/src/lib.rs:172-176`（S3 backend 用 `object_store` crate 的 `store.list()` 流内部自动翻页，但 trait 层仍等全部完成后一次性返回 `Vec`）、`:233-235`
- C++: `eloqstore/include/storage/object_store.h:191-222`（`ListTask` 带 `continuation_token_` + `recurse_`）、`:55-60`（`ParseListObjectsResponse(..., std::string *next_continuation_token)`）；`include/async_io_manager.h:1561-1567`（prewarm 队列 `kMaxPrewarmPendingFiles = 1000`，消费一页再拉下一页）、`src/async_io_manager.cpp`（`PopPrewarmFile` / `MarkPrewarmListingComplete` 按页驱动）

## 现状
`ObjectStore::list` 把整个 prefix 下的全部对象一次性收集进 `Vec<ObjectMeta>` 才返回。`fkv_task_prewarm` 与 cloud GC/restore 的远端 list 都走它。`ObjectMeta` 含 `key/etag/size`，百万对象级 bucket（多分区 × data/segment/manifest/term 对象）下单次 list 的内存与首字节延迟都不可控。conditional put（`if_match/if_none_match`）trait 层已具备，`MemObjectStore` 语义与 S3 基本对齐，本条不涉。

## C++ 实现
C++ 的 List 是**游标式异步任务**：`ListTask` 携带 `continuation_token_`，`ParseListObjectsResponse` 回填 `next_continuation_token`；prewarm 维护上限 1000 的 pending 队列，消费一批再提交下一页 List 任务，`ListTask::SetRecursive` 控制是否递归。列举过程与消费并行，内存 O(队列) 而非 O(bucket)。

## 差异与风险
- **大 bucket OOM / 超时**：list 中途不返回，prewarm 必须等全量列举完才开始下载，恢复时长与对象数线性爆炸；`§7.7` cloud 验收在百万对象场景不可用。
- trait 无法表达"取 N 个 + 游标"，`fkv_object_s3` 即便底层 `object_store` 是流式，也被 trait 逼成全量 `collect`（`lib.rs:174` 的 `while let Some` 循环全收集）。
- 缺 `recursive` 开关：当前 `list(prefix)` 总是递归语义（S3 无目录），与 C++ 非递归 List 行为不一致，cloud 侧 key 布局若引入层级会误匹配。
- `MemObjectStore.list` 返回顺序按字典序排序，与 S3 一致，但 trait 未写明契约，backend 可随意实现。

## 对齐建议
- trait 增加分页形态：`fn list_page<'a>(&'a self, prefix: &'a str, token: Option<&'a str>, max_keys: usize) -> impl Future<Output = Result<ListPage>>`，`ListPage { meta_li, next_token: Option<String> }`；保留 `list()` 作为自动翻页便捷方法并标注大 bucket 慎用。
- `fkv_object_s3` 把 `object_store::list` 流改为按 `max_keys` 取一截返回游标（`object_store` 的 `ListOption::with_limit` + 流截断即可），不再全量 collect。
- `fkv_task_prewarm` 改为按页消费（对齐 C++ `kMaxPrewarmPendingFiles` 的有界队列）。
- trait doc 写明 key 排序与 prefix 递归契约；若需要非递归语义，增加 `delimiter` 参数。

## 性能优化建议
分页游标使 prewarm 边列举边下载，首对象可用时间从 O(对象数) 降到 O(页大小)。

## 严重度
P2（功能缺失 / 规模化风险）

## 审查结论（2026-11-04）

**采纳（P2 规模化）**。

- 成立：`ObjectStore::list` 一次性物化全部对象，无 continuation token /
  max_keys / recursive；prewarm 必须等全量列举完成。
- 修复：trait 增加 `list_page(prefix, token, max_keys) -> ListPage`，
  保留 `list()` 作为自动翻页便捷方法；S3 后端用 `ListOption::with_limit`
  截断；prewarm 按页消费。
- 需同时改 `fkv_object` / `fkv_object_s3` / `fkv_task_prewarm` / cloud GC。
- 无 crate 锁冲突，可独立实施（改动面较大）。

## 进展（2026-11-04，部分完成）

- `fkv_object` 新增 `ListPage { meta_li, next_token }` 与
  `ObjectStore::list_page(prefix, token, max_keys)` 默认实现
  （基于 `list` 过滤 + 截断，`max_keys=0` 不限）。
- `fkv_object_s3` 覆盖 `list_page`：actor 增加 `ListPage` 命令，
  直接消费 `object_store` 的流，按 token 跳过、攒满 `max_keys` 即停止，
  不再把整个 prefix 物化。
- 新增 `fkv_object::object::list_page_walks_without_duplicates`：
  5 个 key、每页 2 条，断言顺序、无重复、无遗漏。
- 验证：`cargo nextest run -p fkv_object -p fkv_object_s3` 18 passed。

追加：`fkv_cloud` 的 `list_manifest_file_name_li` 与
`remote_file_id_li_of` 已改为 `list_page(..., 1000)` 循环，按页解析后只保留
需要的 manifest 名 / file id，不再一次性物化整个 prefix 的 `ObjectMeta`。
验证：`cargo nextest run -p fkv_cloud -p fkv_task_prewarm` 20 passed。

未做：`fkv_task_prewarm` 仍是「先收集全部 file id 再逐个预热」；要真正降低
首对象延迟需改成逐页「列举 → 下载」流水线，属后续项。

## 核验（2026-11-04，本会话）

逐条核对源码后的结论：

- **已落地且属实**：`fkv_object` 的 `ListPage` / `list_page` 默认实现、
  `fkv_object_s3` actor 的 `ListPage` 命令、`fkv_cloud` 的
  `list_manifest_file_name_li` / `remote_file_id_li_of` 分页循环均存在，
  测试 `list_page_walks_without_duplicates` 也在。
- **行号漂移**：`ObjectStore::list` 实际在 `crate/io/fkv_object/src/lib.rs:82`
  （非 `:34`）；`MemObjectStore::list` 在 `:211`；S3 的 `list_page` 在
  `crate/io/fkv_object_s3/src/lib.rs:532`（非 `:233`）。
- **C++ 参考不可核验**：`cpp/eloqstore` 只有 `build` / `shim`，没有
  `include/`、`src/`；issue 里 C++ 文件与行号无法在仓内确认，只能作为语义依据。
- **补充缺陷（issue 未指出）**：`fkv_object_s3` 的 `list_page` 每次都从
  prefix 起点 `store.list()` 拉流再本地跳过 `<= token`，整桶遍历退化为 O(n²)，
  且丢掉 S3 `start-after` 下推。见 `io-29b`。
- **不加 `delimiter`**：当前布局
  `<prefix>/<table>.<partition>/<data|segment|manifest>_...` 是平铺单层，
  递归与非递归等价，加参数只会让所有后端多一条无用分支。
- 剩余可执行项拆为 `io-29a` ~ `io-29d` 四个 todo 文件。


## DONE（2026-11-04）

io-29a ~ io-29d 全部落地：契约文档、S3 `start-after` 下推、
CloudStore 分页 file id API、prewarm 逐页列举→下载流水线。
验证：`cargo nextest run -p fkv_object -p fkv_object_s3 -p fkv_cloud -p fkv_task_prewarm`
49 passed；下游 `fkv_task_reopen` 9 passed；四个 crate `clippy --no-deps -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过。

- `fkv_object/src/lib.rs:23 ListPage`、`:107 list_page` 默认实现；契约文档
  （字节序升序 / token 严格大于 / max_keys==0 不限）见 `:88-127`（io-29a 落地）。
- `fkv_object_s3/src/lib.rs:398` token 非空走 `store.list_with_offset` 下推
  start-after（io-29b）；actor `CommandKind::ListPage`（`:387`）、
  `list_page`（`:553-562`）。
- `fkv_cloud/src/lib.rs:325 CLOUD_LIST_PAGE_KEYS=1000`、`remote_data_file_page`
  （`:2039`）/ `remote_segment_file_page`（`:2050`）、名字过滤收进
  `remote_file_entry_page`（`:2018-2032`）（io-29c，`FileIdPage` 符号已演进为
  `FilePage { entry_li: Vec<RemoteFileEntry>, next_token }`，`:331-339`，
  `remote_file_id_page_paginates` 断言跨页不重不漏 + size，`tests/cloud.rs:567-602`）。
- `fkv_task_prewarm/src/lib.rs:70-96 stream::unfold + VecDeque` 按页拉取、
  `prewarm_paged` `buffer_unordered(PREWARM_FILE_CONCURRENCY=8)`（`:112-140`）、
  `ListingError`（`:155`）；`tests/prewarm.rs:118
  prewarm_streams_across_list_pages`（io-29d）。
