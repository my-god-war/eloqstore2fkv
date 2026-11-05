# R10 cloud 审查发现（rat，VERDICT: FAIL）

## 1. [P1] flush_segment_tbl 上传成功不写 segcomplete 标记

`:2300`。全仓写入点只有 2147（install）、2166（回填）。scheme 启用后本地新建的段文件永久缺
标记 → `segment_state` 判 `Incomplete` → 淘汰/重启后整文件重下，`restore_table_cache:2030`
不登记 → 磁盘占用穿透预算，`drop_incomplete_segment` 误删已上传完整文件。
修：flush 成功后写标记（该 fn 加 `ManifestIo` 约束，或迁到 ManifestIo impl）。

## 2. [P1] enable_segment_marker 回填不过滤在飞/脏 file_id

`:2162`。两个 install 交错启用 scheme 时，B 的回填会给 A 正在整写的文件按旧尾槽盖
「完整」标记，A 崩溃后留下新头 + 旧尾的半装文件被当完整读。
修：跳过 `inflight_download` 中的 `(tbl,true,fid)`，或加表级 enabling 守卫。

## 3. prewarm_segment_file 的 epoch 取太晚

`:3361`。epoch 在 `remote.get` 之后取，get 期间 `evict_table` 无法被 install 起点复核捕获，
已 detach 表被复活。修：epoch 提到 get 之前。

## 4. [P1] sidecar 默认空实现 + 包装层不转发

`fkv_io/src/lib.rs:181` 默认空实现；`fkv_metrics:914`、`roots.rs:203`、`backend.rs:389`、
`io_sim:887` 只转发 manifest 列举 → `clear_segment_marker_li:1983` 在这些组合上静默失效，
标记永久残留、分区目录永不可回收。
修：逐层转发，或改成必需方法杜绝静默。

## 5. [P1] drain_evictions 删段不删标记

`:1580`。按淘汰量线性泄漏小文件，空目录永不回收。
修：`FileIo` 加默认 no-op 的 `remove_sidecar_file_name`，不要在 cloud 层长期接受。

## 6. assemble_segment_file 每槽整块分配

`:1908`。每槽 `vec![0u8; segment_size]` 再 copy，slot_num 次分配 + 2x 峰值。
修：直接读进 `bytes[off..off+size]`，缺槽保持零。

## 7. S3 文档重复两行

`fkv_object_s3:539,552`。`HTTP 状态码 -> 引擎错误码`、`HTTP / 传输错误` 各出现两次，删重复。

## 已核对正确

S3 版本锁用例、drop_manifest 删到 own、scheme 仅缓存 true 无脏缓存、旧目录尾槽兼容。
Missing 优先命中、install 先摘再写可接受。

## 独立复核（dragon，2026-11-04T23:45Z，只读核对当前源码）

**7 条全部已在源码中落地**（`fkv_cloud/src/lib.rs` 等在此前数分钟内连续被编辑，逐条核对如下）：

1. **已修**。`flush_segment_tbl`（`:2365-2381`）在 `remote.put` 成功后立即
   `self.write_segment_marker(tbl, file_id).await?`，注释即指出「本实例本地新建的段文件
   不走 install 回填，缺标记会被 `segment_state` 判半装 → 淘汰/重启后整文件重下，
   `restore_table_cache` 不登记」。标记写入现集中在 `write_segment_marker` 一处
   （`write_manifest_file(..., segment_marker_name(file_id), SEGMENT_MARKER_MAGIC)`）。
2. **已修**。`enable_segment_marker`（`:2193-2219`）有 `segment_enabling_set` 表级守卫
   + `EnablingGuard`，回填循环先过 `segment_backfill_skip(tbl, file_id)`
   （文档：跳过在飞下载 / 有未上传脏数据的 file id），再 `legacy_segment_complete` 判定。
3. **已修**。`prewarm_segment_file`（`:3423-3445`）：`let epoch = self.evict_epoch.get();`
   已提到 `remote.get` **之前**，注释与意见同因（get 期间 `evict_table` 会让 install 复活已 detach 表）。
4. **已修（选了「改成必需方法」）**。`fkv_io` 的 `list_sidecar_file_name_li` /
   `remove_sidecar_file_name` 现在**无默认实现**，文档写明「空实现会让包装层漏转发时静默失效
   （标记永久残留、分区目录永不回收），每个后端与每个包装层都必须显式提供」；`roots.rs:208-213`、
   `backend.rs:400-412`、`io_sim`、`fkv_metrics` 侧均已显式转发。
5. **已修**。`drain_evictions`（`:1597-1650` 一带）在先删段文件、成功后再删完成标记，
   注释写明「只删文件不删标记会按淘汰量线性泄漏小文件，分区目录永远无法回收」。
6. **已修**。`assemble_segment_file`（`:1933-1952`）改为 `bytes.chunks_exact_mut(size)`
   直接读进目标切片（缺槽 `fill(0)`），注释说明「不再每槽分配一份临时缓冲
   （原实现 slot_num 次分配 + 2x 峰值内存）」。
7. **已修**。`fkv_object_s3` 的 `HTTP 状态码 -> 引擎错误码 …` 现全文件仅 **1 处**（`:548`），
   `HTTP / 传输错误` 亦无重复。

**证据（本机实测）**：

- `cargo nextest run -p fkv_cloud -p fkv_io -p fkv_object_s3 -p fkv_io_sim` → **132 passed**
- `./test.sh` → **944 passed / 1 skipped**；账本 `-p fkv_ledger` → 22 passed
- `cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings` → **EXIT=0**

**建议**：本批 7 条可整体移入 `reviewed/`（无残余、无 reject）。

**覆盖情况（更正上一段我先前的说法，逐条查了测试）**：

- #1、#5 **已有失败注入式用例**：`fkv_cloud/tests/segment_marker_gc.rs`
  `flush_writes_segment_marker`（断言 flush 后存在 `segcomplete_0`）、
  `drain_evictions_removes_segment_marker`（注释即写明「回退即失败：只删段文件时标记残留」）。
- #4 有 `list_sidecar_file_name_li` 层面的断言（同文件 :67-86 一带的 scheme 标记用例）。
- **#2 已有专测**：`fkv_cloud/src/lib.rs` 的 in-src `#[cfg(test)]`
  `scheme_backfill_skips_inflight_and_dirty_segment`（造「段 0 在飞 + 段 1 脏」后
  断言这两者**没有**回填标记、干净段 2 有标记），另有更强的跨实例用例
  `scheme_backfill_skips_segment_busy_of_other_instance`。该路径需要私有
  `enable_segment_marker` 与 `inflight_download` 内部状态，`tests/` 侧无法复刻。
- **#3 已在本轮补测**：新增
  `crate/engine/fkv_cloud/tests/segment_prewarm_epoch.rs::prewarm_epoch_taken_before_remote_get_prevents_revive`
  —— 手工 poll 让 `prewarm_segment_file` 停在 `remote.get`，期间 `evict_table`，
  再放行；断言返回 `TryAgain` 且 `resident_bytes() == 0`。
  **回退即失败已实测（2026-11-05T02:14Z）**：把 `let epoch = self.evict_epoch.get();`
  挪到 `remote.get` 之后 → 用例立即 FAIL（`Ready(Ok(true))`）；复原后
  `-p fkv_cloud` **104 passed** 全绿。
- #6 属分配模式，无测试；若要钉住可加「每次 assemble 的临时分配次数为 0（除目标缓冲）」断言。
- #7 纯文档。


## 处置（2026-11-05）

报告 7 条已全部在后续提交中落地（flush 写标记、回填跳过在飞、epoch 提前、sidecar 转发、
drain 删标记、assemble 直接读、S3 文档去重）；移入 `issue/reviewed/`。
