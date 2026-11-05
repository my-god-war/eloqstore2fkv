# 00-v-lane-b：验证轮 lane B（io/compio/数据面/耐久性）

- 轮次标记：2026-11-05 v
- 输入：`issue/done/` 中 lane B 8 条（r8e-01/02/03、r9e-01/02、rev-01/02/03）
- 约束：只读源码（亲读行号），未改 `crate/` 代码/文档，未运行 cargo；仅写
  `issue/new/`、`issue/reviewed/` 并从 `issue/done/` mv
- 基线：eloqstore `82b774c`；本地 compio 源码 `0.20.0-beta.1`；fkv 依赖 compio `0.19.2`

## 逐条去向

| 条目 | 去向 | 判定 | 关键证据（file:line，亲读） |
|---|---|---|---|
| r8e-01-sync-data-dirty-dir-cleared-before-dir-fsync | `issue/reviewed/` | 通过 | `lib.rs:2524-2531` dir fsync 成功后才 `dirty_dir.remove`；测试见 r9e-01 |
| r8e-02-bench-key-interval-stride-diverges-from-cpp | `issue/reviewed/` | 通过（登记） | `COMPAT_BASELINE:64-67`；`doc/metrics.md:169-173`；`main.rs:609-612`/`:318` 措辞已如实 |
| r8e-03-open-segment-file-collapse-non-notfound-errors | `issue/reviewed/` | 通过（代码），残余→v-05 | `lib.rs:1990`（NotFound 耗尽）/`:1993`/`:1996`（map_io_err）；EISDIR 测试缺失 |
| r9e-01-sync-data-dirty-dir-retry-window-untested | `issue/reviewed/` | 通过 | `tests/durability_dir.rs:78-96` 真实注入 `SyncDir@1!err=IoFail` |
| r9e-02-open-data-file-still-folds-open-errors | `issue/reviewed/` | 通过（目标1 代码），残余→v-05 | `lib.rs:2015`/`:2016`/`:2019`；目标2 测试、目标3 同类折叠点未做 |
| rev-01-write-file-missing-tmp-fsync-before-rename | `issue/reviewed/` | 通过（代码），残余→v-06 | `lib.rs:3092-3112` write→sync_data(:3101)→rename(:3107)→dirty_dir(:3112) |
| rev-02-enobufs-raw-branch-dead-resourcebusy-unmapped | `issue/reviewed/` | 通过（代码），残余→v-05 | `lib.rs:725-727` ResourceBusy→TryAgain；raw ENOBUFS 分支 :730 保留 |
| rev-03-fixed-buffer-drop-hook-missing-token-ring-guard | `issue/reviewed/` | 通过 | `lib.rs:2854-2859` owns_current_ring 重建；`:2864-2876` token ptr_eq；`fixed.rs:349-353` |

## 耐久性语义专项核对（任务点 4）

- **tmp fsync 顺序（写入→fsync(tmp)→rename→fsync(dir)）**：`write_file`
  `lib.rs:3092-3112` 顺序正确（`:3101` sync_data 在 `:3107` rename 之前；
  目录 fsync 由下一次 `sync_data` 的 `:2527-2530` 承接）。`atomic_write_file`
  `:1941-1968` 同序。**唯一缺口**：`write_file` 无 failpoint，无法注入 sync 失败
  （→ v-06）。`sync_data` 本体顺序正确（见下）。
- **dirty dir 清理时机（必须在 dir fsync 成功后）**：`lib.rs:2527-2530`
  `sync_dir(&dir).await?` 成功后才 `dirty_dir.remove(tbl)`；失败条目保留、下次
  重试补同步；与文件路径 `:2518-2523`「先 fsync 成功再清脏」一致。真实注入
  护栏 `tests/durability_dir.rs:78-96` 有效（回退修复第二轮断言必失败）。
- **ENOBUFS / ResourceBusy 错误映射**：`lib.rs:725-727` 覆盖 compio completion
  丢失 raw errno 的 `ErrorKind::ResourceBusy → TryAgain`；`:730` raw
  `EINTR/EAGAIN/ENOBUFS → TryAgain`；`:733` `EMFILE/ENFILE → OpenFileLimit`。
  分类正确，但 `pub(crate)` 无单测（→ v-05）。
- **fixed buffer drop 钩子与 token ring 守卫**：`lib.rs:2864-2876` 钩子捕获
  registry token 并 `Arc::ptr_eq` 校验后才 `release`；`:2854-2859` 复用前
  `owns_current_ring()` 否则置空重建；`fixed.rs:351-353` `token()` 为
  `Arc::clone`（ptr_eq 有效）。防护闭环。

## 新立项

- [`v-05`](../new/v-05-open-error-errno-classification-tests-missing.md)（P3）：
  打开路径 errno 分类（segment `read_segment` / data `read_file`）与
  `map_io_err` 的 ResourceBusy 映射无任何测试钉住——承载 r8e-03 / r9e-02 /
  rev-02 三者残余；另含 r9e-02 目标 3 未裁定的同类折叠点。
- [`v-06`](../new/v-06-write-file-tmp-fsync-no-failpoint-no-durability-test.md)
  （P3）：rev-01 修复的 `write_file` tmp→fdatasync→rename 无 failpoint、无耐久
  护栏，回退 `sync_data()` 现有测试全绿。

已查重：`grep -rn "ResourceBusy\|OpenFileLimit\|NoPermission\|EISDIR\|WriteFile\|write_file" issue/`
无其它承接条目；与 lane A 的 v-01 不同轴。

## 在途 / 停止条件

- lane B 8 条均仍在 `issue/done/`，无被并行会话移走，无跳过。
- 在途观察（未阻断判定）：`crate/io/fkv_io_compio/src/lib.rs` mtime 07:38
  （> 5 分钟），`git diff` 只动 `:1898-1912` 的 `AppendManifest:SyncFail`
  failpoint（并行会话），**未触及任何 lane B 锚点**（2518-2531 / 1970-2021 /
  719-738 / 2844-2876 / 3083-3114 行号在读时一致）。`tests/durability_dir.rs`
  mtime 06:28、`fixed.rs` 07:24，均未再变。
- 旁路：lane A 报告 `issue/reviewed/00-v-lane-a.md`（08:00）、`issue/new/v-01-*`
  已存在（非本 lane，未干涉）。

## 结论

lane B：通过 8 / 不完善 0（全部代码核验属实；测试残余 3 条拆出 v-05/v-06）。
新立项 2（v-05、v-06）。未改动任何 `crate/` 代码或文档，未运行 cargo。
