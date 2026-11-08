# sem-73 cloud 启动「非空本地根 + allow_reuse_local_caches=false」：C++ `InvalidArgs` 拒启，fkv 启动并懒清本地 cache

- 领域：语义 / 启动接受面（cloud 本地 cache 复用）
- 优先级：P3（cloud-only；只在「本地根非空 + 复用关闭（默认）」出现；fkv 更宽松，不改盘面格式与可见数据）
- 置信度：高（两侧启动路径逐行核对 + C++ 全部 cloud 重启用例均显式置 `allow_reuse_local_caches=true` 反证）

## 现象

C++ 在 `Start` 时对每个 `store_path` 根执行 `EloqStore::InitStoreSpace()`；**cloud
模式下若 `allow_reuse_local_caches=false`（默认）且该根非空，直接回 `InvalidArgs`
并 `fail_start`**：

```
src/eloq_store.cpp:781        KvError err = InitStoreSpace();
src/eloq_store.cpp:1178-1198  const bool cloud_store = !options_.cloud_store_path.empty();
                              ...
                              if (cloud_store && !options_.allow_reuse_local_caches &&
                                  !std::filesystem::is_empty(store_path)) {
                                  LOG(ERROR) << store_path
                                             << " is not empty in cloud store mode, clear "
                                                "the directory";
                                  return KvError::InvalidArgs;
                              }
```

即：C++ 云存储用默认配置（复用关闭）时，只要本地 cache 目录里有上次运行留下的
分区目录，`Start` 就**拒绝启动**，要求运维先清目录或显式打开
`allow_reuse_local_caches`。

fkv 没有这一启动检查：`Runtime::start` → `backend_of` 在 cloud 分支只把
`allow_reuse_local_caches` 投影进后端，不检查本地根是否为空，直接启动成功；随后在
**首次打开某表时按表懒清**该表的本地 data / segment：

```
crate/engine/fkv_runtime/src/startup.rs:334   pub fn start(config) -> Result<Self>   // 无根非空检查
crate/engine/fkv_runtime/src/backend.rs:91    allow_reuse_local_caches: option.allow_reuse_local_caches,
crate/engine/fkv_cloud/src/prewarm.rs:19-28   restore_table_cache(): if !self.reuse_local_caches {
                                                self.invalidate_local_file_li(tbl).await?; ... }
crate/engine/fkv_cloud/src/prewarm.rs:226-245 invalidate_local_file_li(): 删本地 data / segment + clear 标记
```

行为锁定测试：`crate/engine/fkv_cloud/tests/cloud/evict.rs:249-266`
（`allow_reuse_local_caches=false` → 本地 data / segment 被清空、`resident_bytes()==0`）。

## 证据

- 反证：C++ 测试套件里**每个** cloud 重启 / 复用用例都显式
  `options.allow_reuse_local_caches = true;`
  （`tests/cloud.cpp:439,517,572,609,1436,1584,1699`、`tests/cloud_term.cpp:24`、
  `tests/ds_shards.cpp:37`、`tests/standby.cpp:230,474,480,900,906,1097`）——
  正因为默认 `false` + 非空根会 `InvalidArgs` 拒启，测试必须显式打开复用才能重启。
- `COMPAT_BASELINE` 第 462 行只把该旋钮列为「已接线」
  （`max_cloud_concurrency` / `cloud_verify_ssl` / `allow_reuse_local_caches` /
  `cloud_auto_credentials`），未描述关闭态的启动接受面。
- `COMPAT_BASELINE` 第 758-769 行「多根 store_path 全局枚举的缺失 / 不可读根语义」
  仅把 C++ `InitStoreSpace` 的已知偏离限定为「根不存在」这一条
  （「Rust 启动不建根……已知偏离仅限『根不存在』这一条」）——**未覆盖
  「非空根 + 复用关闭 ⇒ 拒启」这条**，与本条冲突，该「仅限」为过强断言。

## 次要同面差异（并入本条，均为本地 cache 面，不逐字节对拍）

在 `allow_reuse_local_caches=true` 时两侧「复用哪些文件 / 何时复用」也不同：

- **时机**：C++ 在 `CloudStoreMgr::RestoreStartupState()`（`Init` 期，
  `src/async_io_manager.cpp:3628-3635`）**急切扫全部 shard 自有分区目录**；
  fkv 是 **per-table 懒恢复**（首次 `read_manifest` / `read_manifest_file` 触发，
  `fkv_cloud/src/manifest.rs:138,274,378`）。fkv 从未访问过的陈旧分区目录会一直留在
  盘上（C++ 直接拒启，不给这个状态）。
- **文件集**：C++ `RestoreFilesForTable`（`async_io_manager.cpp:4003-4178`）
  会删除本地**全部 manifest**、删除**最大 data file id 的全部 term 文件**、
  删 `.tmp`，非法条目（非常规文件 / 未知前缀 / 坏 data 名）返回 `InvalidArgs`
  且 shard 侧 `LOG(FATAL)+exit(1)`（`src/storage/shard.cpp:104-110`）；
  fkv 全部登记（最大 id 文件照留），非法残留不予理会。
- 结论：这些属既有「本地 cache 模型不做 1:1」（rev-41 + `COMPAT_BASELINE`
  单池条目）范围，主路径等价（都用 manifest 的 per-file 归属 + 校验决定命中），
  不产生可见数据 / 远端对象差异；本条只要求把**启动接受面**（拒启 vs 启动）
  登记清楚。

## 影响

- 差分 / 迁移口径：同一「cloud + 默认配置（复用关闭）+ 非空本地根」输入，
  C++ 侧 `Start` 回 `InvalidArgs`、fkv 侧启动成功并清本地 cache。差分器若把
  两侧启动结果对拍，会把该差异报成失败——需按已知差异跳过，或统一在同一侧
  先清目录 / 打开复用。
- 运维口径：C++ 默认要求「云存储干净启动或显式打开本地复用」；fkv 默认自动
  清本地 cache 继续启动。迁移时若依赖 C++ 的 fail-fast 兜底「盘没清干净」，
  fkv 不会报错。
- 不改任何盘面格式、对象命名与可见数据；仅启动判定 / 本地 cache 起点不同。

## 建议

（登记型，二选一）

1. 把本条登记进 `COMPAT_BASELINE`：修正第 758-769 行「已知偏离仅限『根不存在』」
   的过强措辞，补一条「cloud + `allow_reuse_local_caches=false` + 非空本地根：
   C++ `InvalidArgs` 拒启（`eloq_store.cpp:1190-1196`），fkv 启动并按表懒清
   （`fkv_cloud/src/prewarm.rs:19-28`）」；差分口径「C++ 拒启不得判 fkv 错，
   fkv 启动成功不得当作 C++ 侧可复现」。
2. 若希望对齐 C++ fail-fast：`backend_of` 的 cloud 分支在
   `!allow_reuse_local_caches` 时检查 `RuntimeConfig::root`（及 `store_path` 各根）
   是否为空，非空回 `InvalidArgs`。倾向方案 1（fkv 自动清更稳，对齐 C++ 会引入
   默认配置无法重启的运维坑）。

## 验收方式

```bash
cd /tmp/r29/fkv
cargo nextest run -p fkv_cloud                     # 含 evict.rs 的复用/清空用例
cargo nextest run -p fkv_runtime store_path        # 现有多根/缺失根用例仍绿
```

（本轮已跑：`fkv_cloud` 136/136、`fkv_runtime::store_path` 2/2、`fkv_config` 46/46
全绿。）

## 实现记录（2026-11-08，登记型）

- COMPAT_BASELINE 增补：
  - 新条目「Cloud 启动对非空本地根的接受面（sem-73，Rust 更宽松）」——C++
    `InitStoreSpace` 在 cloud + `allow_reuse_local_caches=false`（默认）且本地根
    非空时 `InvalidArgs` 拒启（`eloq_store.cpp:1178-1196`，上游 cloud 重启测试
    均显式置 true）⇔ fkv 启动成功并按表懒清（`fkv_cloud/src/prewarm.rs:19-28`
    → `invalidate_local_file_li`；锁定测试 `fkv_cloud/tests/cloud/evict.rs`）；
  - 差异总表行（新差异仅在拒绝启动 / 清理时机，盘面与数据不变）；
  - **修正原过窄断言**：多根 `store_path` 条目的「已知偏离仅限『根不存在』这一条」
    → 改为两条（根不存在 + 本条的 cloud 非空根）。
- 验证：`check_baseline_sync --check` 44/44（`--fix` 刷新压行 84889 chars）；
  `check_issue_links --check` 1393/0/0；`fkv_config`/`fkv_cloud` 相关套件在
  语义域抽跑已绿（136/136、46/46、2/2）。

## 复审记录（2026-11-08，第 39 轮结构域）

- 判定：**达标**，移入 reviewed/。COMPAT_BASELINE 三处（新条目 `:768-780`、
  差异总表行 `:1354`、过强断言修正 `:765-767`）逐条核对；C++ 侧引用
  （`eloq_store.cpp:1178,1190-1196`、`kv_options.h:190` 默认 false、
  `tests/cloud.cpp` 7 处显式置 true）与 fkv 侧（`startup.rs:334`、`backend.rs:91`、
  `fkv_cloud/src/prewarm.rs:19→226`、锁定测试 `cloud/evict.rs:249-266`）全部对得上。
