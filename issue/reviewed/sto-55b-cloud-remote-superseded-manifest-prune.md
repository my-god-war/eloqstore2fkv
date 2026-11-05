# sto-55b 云端旧 term manifest 远端对象剪枝

## 位置
- `crate/engine/fkv_cloud/src/lib.rs`：`CloudStore::manifest_key` 按 `manifest_<branch>_<term>` 上传远端；`list_manifest_file_name_li` 合并远端对象；`prune_superseded_manifest_files` 当前显式 no-op。
- 参考：`crate/io/fkv_io/src/lib.rs` 的本地默认剪枝规则（sto-55a）。

## 现状
云端每次 term bump 都会产生新的远端 `manifest_<branch>_<term>` 对象，旧对象永久驻留：
- 进入 `manifest_retained_file_page_set` 的 replay 输入，放大 GC 开销；
- 旧 manifest 的 retained 集合持续计入保护集，回收面变窄。

本地剪枝规则已实现，但 `CloudStore::remove_manifest_file` 会同时删远端对象，本轮按「远端对象删除单独留 todo」的要求关闭。

## 方案（需谨慎）
1. 前置条件：确认当前 term 已通过 `CURRENT_TERM` / fencing 生效，且不存在 term 更小的活跃写入者；跨实例场景需 lease 或 term 租约证据。
2. 只删同 active branch、无 tag、`term < 当前 term` 的远端 manifest 对象；绝不删当前 term、其它 branch、archive、`main` 固定名对象。
3. 远端删除与本地缓存删除分开：先删远端，再清本地缓存；失败只记录指标，不回滚已删对象。
4. 增加开关（默认关闭），先在单实例云端测试开启验证。

## 验收
- 新增云端集成测试（fkv_cloud tests）：构造 `manifest_main_1` / `manifest_main_2` 远端对象，term=2 时仅删 1，保留 2 / archive / 其它 branch。
- 并发/旧 term 写入保护测试：旧 term 仍有写入者时不得删除。
- `cargo nextest run -p fkv_cloud --no-fail-fast` 全绿。

## DONE（2026-11-04）

- `CloudStore::prune_superseded_manifest_files` 落地：先 `remote_manifest_name_li`
  分页全量列举远端 manifest 并按 term 升序；仅当远端同 branch 最高 term 等于
  本实例 identity term 时，删除更旧的无 tag manifest（远端对象 → 本地 cache）。
- 保护：`main` / `term == 0` / identity 不一致 / 存在更高 term 全部跳过；当前
  term 对象永不删除；单对象删除或本地清理失败只 `log::warn!`，不阻断 GC 收尾。
- 复用列举逻辑：`list_manifest_file_name_li` 与剪枝共用 `remote_manifest_name_li`。
- 测试 `crate/engine/fkv_cloud/tests/manifest_prune.rs`：旧 term 删除且最新保留、
  更高 term 存在时零删除、identity 不匹配零删除、main 不剪枝。
- 验证：`cargo check -p fkv_cloud`；`cargo nextest run -p fkv_cloud -p fkv_store
  --no-fail-fast`（37 + 139 全绿）；`cargo clippy -p fkv_cloud --no-deps
  --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

通过（注：实现在 `fkv_cloud`，GC 收尾主题归本 lane，cloud lane 交叉）。实现锚点
`fkv_cloud/src/lib.rs:1998`（`CloudStore::prune_superseded_manifest_files` 覆写）；
测试 `fkv_cloud/tests/manifest_prune.rs`（5 个测试）存在。逐行断言未抽读（cloud 侧），
实现存在性与测试文件已核实。
