
## DONE（2026-11-04）

**结论：三项验收全部落地并实测；守卫补上「owner 已归档」这个此前未覆盖的漏洞。**

1. CI 接入（核对，无需改动）：`.github/workflows/ci.yml:28-29` 已有
   `cargo clippy --workspace --all-targets -- -D warnings` 紧跟 `./tool/orphan.sh`，
   `sh/clippy.sh` 保持只做 clippy（本地开发不被依赖图检查打断），与 33 的落地一致。
2. 白名单 owner 清理（本轮实际开发项）：
   - 事实修正：33 原文称「守卫的 stale 检查会强制同步删除白名单行」，实测**不成立**。
     32 条 owner 校验用的是 `find issue -name "*$owner*"`，包含 `issue/done/`、
     `issue/reject/`，所以「归口 issue 已归档、crate 仍是孤儿」会被白名单永久掩盖。
     本次就命中了：`12-runtime-modes-api` 已移入 `issue/done/`，但 5 行白名单仍指向它。
   - `tool/orphan.sh`：owner 必须解析到 **活** 文件（`issue/new`、`issue/todo`、
     `issue/` 根）；只存在于 `issue/done` / `issue/reject` / `issue/reviewed` 时报错
     并给出处理指引；完全不存在仍走原有「无对应文件」报错，两类不再互相混淆。
   - `tool/orphan_allow.tsv`：5 行 owner 指向 12 的活子项，并更新原因：
     `fkv_standby` → `12b-standby-runtime-wiring`（Runtime 无 standby 同步线程）、
     `fkv_task_prewarm` → `12a-per-table-request-gaps`（Runtime 未接 Prewarm 请求）、
     `fkv_cloud` / `fkv_runtime` / `fkv_object_s3` →
     `12c-global-fanout-and-cloud-followups`。
     另：`fkv_root` 行已在上一条 `33a` 删除（白名单 8 → 7 项）。

验证（含正负样例，临时改动均已回滚）：

- 正样例：`./tool/orphan.sh` → 退出 0，「孤儿 crate 守卫通过（白名单 7 项）」；
  传递孤儿提示 `fkv_api` / `fkv_metrics` / `fkv_object` / `fkv_shard`（属 12 未接线的运行时栈）。
- 负样例 1（未登记孤儿）：临时建 `crate/core/fkv_probe_orphan`（workspace glob 自动入图）
  → 退出 2 并列出 `fkv_probe_orphan`；已 `rm -rf`，并用 `cargo check -p fkv_ledger`
  让 `Cargo.lock` 剪掉残条目（校验 `grep -c fkv_probe_orphan Cargo.lock` = 0）。
- 负样例 2（白名单过期）：临时给已有生产消费者的 `fkv_store` 加一行 → 退出 2
  并报「白名单已过期」。
- 负样例 3（owner 不存在）：临时写 `probe-owner` → 退出 2 报「无对应文件」。
- 负样例 4（owner 已归档）：临时把 `fkv_cloud` 的 owner 改回 `12-runtime-modes-api`
  → 退出 2 报「owner 已归档」+ 处理指引（本轮新增能力）。
- `cargo test -p fkv_ledger` → 7 + 1 + 8 passed（含他人正在改的 `baseline.rs`）。
- `bash -n tool/orphan.sh` 通过。

留给上游的说明：

- `issue/todo/33-orphan-crates.md`（父条目）的验收「守卫退出 0 + 白名单每项都有归口」
  现已满足；其两个子项 33a（done：删除 `fkv_root`）、33b（本条 done）均已闭环，
  父条目可由其 owner 收口（本条队列只到 33b，未动父文件）。
- `fkv_runtime` 的最终接线（CLI/入口经 Runtime 起 shard）目前没有专门 issue，
  暂挂在 `12c` 下；若 12c 收尾时仍未接线，建议新开一条入口接线 issue。

## 复审（2026-11-05 r6）

通过。

- `.github/workflows/ci.yml:28-29`：`cargo clippy --workspace --all-targets
  -D warnings` 紧跟 `./tool/orphan.sh`，与 md 记录一致。
- `tool/orphan.sh:104 ARCHIVE_DIR_LI=(issue/done issue/reject issue/reviewed)`、
  `:7-9` 注释写明 owner 必须是活文件、归档需重新归口——「owner 已归档」检查
  落地。
- `tool/orphan_allow.tsv` 现为 6 项（fkv_cloud/fkv_task_prewarm/fkv_runtime/
  fkv_object_s3 指向 12* 活项 + fkv_io_mem/fkv_io_sim test-only），比 md 的
  7 项少一行 `fkv_standby`——12b standby 接线完成后被后续清理移除，属正常
  演进，白名单每项仍有归口。
