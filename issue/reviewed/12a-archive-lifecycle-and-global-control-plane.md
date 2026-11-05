# 12a/12c archive 生命周期与全局控制面：P0/P1 语义修正

来源：2026-11-05 复审确认的四条 P0/P1（archive 只能创建/列举、global_archive
空 tag 各 shard 自造、global_drop_table 无表名、global_create_branch 无 salt、
global_list_archive_tags 不去重）。本条记录已落地的代码与验证，剩余缺口见文末。

## 已实现

### 1. per-table archive 生命周期（`fkv_api` / `fkv_store` / `fkv_shard`）

- `fkv_api::Request` 新增：
  - `ReopenArchive { tbl, tag }`：按 tag 重开归档；空 tag = 重开活跃快照
    （对齐 C++ `ReopenRequest`：empty = latest available snapshot）。
  - `DeleteArchive { tbl, tag }`：删除该 branch 下该 tag 的归档（全部 term），
    空 tag 非法。
- `fkv_api::Response` 新增 `DeleteArchive(usize)`；`ReopenArchive` 复用
  `Response::Reopen`（C++ 无独立结果载荷）。
- `fkv_store`：
  - `reopen_archive(tag)`：扫当前 branch 的 `manifest_<branch>_<term>_<tag>`，
    多 term 取最高 term（C++ Cloud 用当前 process term 精确匹配；fkv 的归档跨
    term 保留，取最高 term 可在重启 / term 推进后按 tag 恢复），replay 后把
    快照写回活跃 manifest（main 用 `switch_manifest`，branch 用同名 manifest），
    后续读写继续基于该快照。
  - `delete_archive(tag)`：幂等删除全部 term 的同 tag 归档；与 `delete_branch`
    一致按 tag 扫盘，C++ 只按当前 process term 定位（有意扩展）。
- `fkv_shard`：新增 dispatch 分支、`Request::table`、`is_write_op`（两者都进
  per-partition 写队列，串行化归档创建/重开/删除）、`pri_of`（DeleteArchive
  低优先）、`op_of`（ReopenArchive → `Op::Reopen`，DeleteArchive →
  `Op::CreateArchive`，与 C++ `RequestType` 一致）。`prepare_table` 对两者跳过
  ensure：不应为了删归档 / 读归档给未打开分区建空表。
- 重试：`ReopenArchive` 空 tag 与 `Reopen` 一样进 `ReopenQueue`；非空 tag 缺失
  直接回 `ResourceMissing`（确定快照不存在，不能拿活跃 manifest 的可见性当前置）。

### 2. 全局控制面（`fkv_runtime`）

- `global_archive(tag, num_retained)`：
  - 空 tag 只在 runtime 生成一次（`fkv_api::archive_tag`）并广播给全部分区，
    一次全局归档 = 一份同名快照；
  - 通过 `GlobalReply::tag` / `GlobalReply::name` 回传实际 tag 与归档 manifest 名
    （对齐 C++ `GlobalArchiveRequest::ResultArchive`）。
- `global_reopen(tag)`：空 tag → `Request::Reopen`；非空 → `ReopenArchive`，
  同一 tag 广播全部分区（对齐 C++ `GlobalReopenRequest`）。
- `global_delete_archive(tag)`：广播 `DeleteArchive`（对齐 C++
  `GlobalArchiveRequest::Action::Delete`；空 tag `InvalidArgs`）。
- `global_create_branch(branch)` / `global_create_branch_with_salt(branch, salt_ts)`：
  归一化 branch 名后追加 `-<8 位十六进制 salt>`（`salt_ts == 0` 用当前微秒低
  32 位，非 0 用给定时间戳低 32 位），内部名通过 `GlobalReply::name` 返回
  （对齐 C++ `ResultBranch`）。
- `global_drop_table(tbl_name)`：按表名过滤目标分区后逐分区 `Drop`，不再删全部
  分区（对齐 C++ `DropTableRequest`）；空表名 `InvalidArgs`。
- `global_list_archive_tag_li` / `global_list_archive_tags`：`sort()` 后
  `dedup()`，按 `(tag, term)` 全等去重（C++ `sort` + `unique`）；新增
  `global_list_archive_tags` 同义方法。
- `GlobalReply` 新增 `tag` / `name` 字段（`Default` 为空，已有调用方只读
  `ok_num` / `failed_li` / `total()` 不受影响）。

### 3. metrics（`fkv_metrics`）

不新增 `Op`：C++ 的 `ArchiveRequest::Action::Delete` 与 Create 共用
`RequestType::Archive`，`ReopenRequest`（含 tag）与 Reopen 共用
`RequestType::Reopen`。`DeleteArchive` 记 `Op::CreateArchive`、`ReopenArchive`
记 `Op::Reopen`，保持 `from_cpp_request_type("archive"/"reopen")` 唯一。已在
`Op` 文档中注明。

## 测试

- `crate/engine/fkv_store/tests/branch_archive.rs::reopen_and_delete_archive`：
  归档 → 继续写 → 按 tag 重开 → 重启仍为归档快照 → 空 tag 重开活跃 →
  删除幂等 / 空 tag / 不存在 tag / 非法 tag。
- `crate/engine/fkv_runtime/tests/archive_lifecycle.rs`：空 tag 全局归档同名、
  全局列举去重、全局按 tag 重开、按 tag 删除（per-table + global）、
  salted 全局建分支、按表名全局删表（不波及其它表）。
- `crate/engine/fkv_runtime/tests/global_list_archive_tags.rs`：跨分区同
  `(tag, term)` 只回一条（修正原「不去重」错误期望）。
- `crate/engine/fkv_runtime/src/lib.rs` 单测：`global_drop_table` 新增表名过滤 /
  空表名断言；list 语料补跨分区全等条目。
- `crate/engine/fkv_runtime/tests/cloud_e2e.rs`：补充 `global_archive` 结果名、
  salted 分支名、`global_reopen("tag")`（需 `EKV_S3_ENDPOINT`）。

验证命令：

```
cargo test -p fkv_store --test branch_archive
cargo test -p fkv_runtime --lib
cargo test -p fkv_runtime --test archive_lifecycle \
  --test global_list_archive_tags --test archive_crond
cargo test -p fkv_shard
cargo clippy -p fkv_api -p fkv_store -p fkv_shard -p fkv_runtime -p fkv_metrics \
  --all-targets --no-deps -- -D warnings
```

## 破坏性变更（明确标注）

- `Runtime::global_drop_table()` → `global_drop_table(tbl_name: &str)`：原签名会
  删除**全部表**的全部分区，语义危险；新签名对齐 C++ `DropTableRequest`。
- `Runtime::global_create_branch(branch)` 现在生成 `branch-<salt>` 内部名；
  调用方必须使用 `GlobalReply::name` 继续操作分支（原行为直接用 branch 名）。
- `GlobalReply` 增加 `tag` / `name` 字段：外部用结构体字面量构造
  `GlobalReply` 的代码需补字段（仓库内无此用法）。

## 已知偏离 / 残余（未在本条修）

1. **生产入口仍缺**：`fkv_runtime` 没有 facade/cli 消费者，4+1 个 `global_*`
   仍未接线（`issue/todo/eng-84`）。
2. **重开旧归档后写入可能复用被回滚的页空间**：`reopen_archive` 把归档快照
   提升为活跃状态后，append 分配器从归档的 `max_fp_id` 继续；若同 branch 在
   该归档之后又写过数据 / 建过更新归档，新写页可能与这些更新的页重叠。C++
   `RefreshManifest(tag)` 同样是 rollback 语义；需要保留更新归档时应先建 branch。
3. **空目标集合的 result name**：C++ 即使 0 个分区也返回
   `BranchArchiveName(branch_, ShardTerm, tag)`；fkv runtime 不持有全局
   branch/term，0 个成功回执时 `GlobalReply::name` 为空（有回执时取成功分区
   的真实文件名）。需 runtime 记录 attach 身份后再补齐。
3. **Standby tag 精确同步**：C++ `RsyncPartition(tbl, tag)` 只拉指定归档；fkv
   `sync_standby_partition` 拉该分区全部文件（含归档），结果正确但可能多拉。
4. **per-table `ListArchiveTags` / `Prewarm` / `ListObject`** 仍是 `12a` 的
   未完成项，本条只覆盖 archive 生命周期与全局控制面。

## DONE（2026-11-05）

代码与测试全部落地，targeted tests/clippy 通过；工作树未提交（遵守无 commit
约束）。

## 复审（2026-11-05 r7）

结论：**通过**。逐项核对当前源码（均亲读，行号按当日工作区）：

- 请求族落地：`fkv_api/src/lib.rs:117` `ReopenArchive{tbl,tag}`、`:126`
  `DeleteArchive{tbl,tag}`、`:327` `Response::DeleteArchive(usize)`、`:204-205`
  `Request::table` 覆盖两者；`ArchiveTagInfo`（`:286`，另 `fkv_store/src/lib.rs:98`）
  由 per-table `list_archive_tags` 回传。
- store 实现：`fkv_store/src/lib.rs:2342` `reopen_archive`（空 tag → `reopen`，
  多 term 取最高 `:2368-2374`，replay 后写回活跃 manifest `:2400`）；
  `:2413` `delete_archive`（全 term 幂等删，空 tag `InvalidArgs`）。
- shard 接线：dispatch `:2422`（回 `Response::Reopen`）与 `:2523-2525`；
  `is_write_op` `:1375-1376`；`pri_of` DeleteArchive→Low `:92`；`op_of`
  ReopenArchive→`Op::Reopen` `:1093`、DeleteArchive→`Op::CreateArchive` `:1103`；
  `skips_table_open` 跳过 ensure `:1465-1474`；空 tag 重试进 park 队列、非空 tag
  直接 `ResourceMissing` `:1600-1606`。
- runtime 全局面：`global_archive` 空 tag 只生成一次并广播、回传
  tag/name（`fkv_runtime/src/lib.rs:1438-1457`）；`global_reopen` `:1462-1479`；
  `global_delete_archive` 空 tag 拒 `:1483-1486`；
  `global_create_branch_with_salt` 8 位十六进制 salt `:1512-1533`；
  `global_drop_table` 按表名过滤 `:1538-1554`；`global_list_archive_tag_li`
  `:1562` + 同义 `global_list_archive_tags` `:1568`；`GlobalReply.tag/name`
  `:278-283`。metrics 注释 `fkv_metrics/src/lib.rs:44,53-55`。
- 测试抽读：`fkv_store/tests/branch_archive.rs:326-439`
  `reopen_and_delete_archive`（归档→写→按 tag 重开→重启仍为快照→冷启动→空 tag
  →删除幂等/空 tag/缺失/非法 tag，含 `ArchiveTagInfo` 断言 `:352-358`）；
  `fkv_runtime/tests/archive_lifecycle.rs:72-205`（全局同名 tag、列举去重、按 tag
  重开、单分区+全局删除、salted 名 `snap-00000abc`、按表名删表不波及 other）；
  `tests/global_list_archive_tags.rs:51-142`；lib 单测 `:2439/:2494`；
  `tests/cloud_e2e.rs:201-217`。
- 残余（FileGc/LocalGc 合并、空目标 name、rollback 语义）已如实登记且在
  `issue/todo/12a-per-table-request-gaps.md` / `eng-84` 跟踪，不与本条冲突。
