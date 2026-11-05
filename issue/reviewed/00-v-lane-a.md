# 00-v-lane-a：验证轮 lane A（core/format/codec/compress 与杂项）

- 轮次标记：2026-11-05 v
- 输入：`issue/done/` 中 lane A 8 条
- 约束：只读源码（亲读行号），未改 `crate/` 代码/文档，未运行 cargo；仅写 `issue/new/`、`issue/reviewed/` 并从 `issue/done/` mv
- 基线：eloqstore `82b774c`；仓库工作区（并行会话在途）

## 逐条去向

| 条目 | 去向 | 判定 | 关键证据（file:line，亲读） |
|---|---|---|---|
| r8a-01-dead-pub-consts-and-magic-128 | `issue/reviewed/` | 完善（残余转 v-01） | `fkv_types/src/lib.rs:132` 常量在；`fkv_config/src/lib.rs:50` 导入、`:397` 比较消魔法 128；`fkv_buf/src/lib.rs` 无 `DirectIoBuf`（仅 C++ 描述串） |
| r8a-02-fkv-compress-doc-dangling-issue-new-08 | `issue/reviewed/` | 完善 | `fkv_compress/src/lib.rs:309-310`、`tests/compress.rs:78` 去 `issue/new/08`；`fkv_store/src/lib.rs:1708` 注释改「首次尝试后不重试」 |
| r8b-01-branch-reserve-1024-baseline-pointer-broken | `issue/reviewed/` | 完善 | `COMPAT_BASELINE:57-60` 有条目；38 份 `fixtures/**/meta.txt` baseline 同步；`doc/architecture/06-gaps.md:95` 指针有效 |
| r8b-02-ttl-read-filter-default-divergence-unregistered | `issue/reviewed/` | 完善 | `COMPAT_BASELINE:61-63` TTL 读过滤条目；`doc/progress.md:6` 镜像含 `ttl_read_filter` |
| r9a-02-fkv-codec-put-fixed64-zero-production-consumer | `issue/reviewed/` | 完善 | `put_fixed64` 被 `fkv_page/src/header.rs:82`（set_checksum）、`fkv_manifest/src/lib.rs:307`（finalize）消费 |
| r9a-03-phantom-issue-ids-in-scope-comments-... | `issue/done/`（留） | 不完善 | 6 个悬空编号全部原样保留（见下） |
| r9b-01-task-compact-single-file-wrappers-zero-production-consumer | `issue/reviewed/` | 完善 | `fkv_task_compact/src/lib.rs` 仅剩 `compact_file_li`:25 / `compact_segment_file_li`:88；`tests/compact.rs:7/:55/:98` 改调 `_li`；`fkv_store:2976/:3891` 消费 `_li`；无 re-export/doc 引用 |
| r9b-03-with-branch-silently-keeps-unnormalized-name | `issue/reviewed/` | 完善 | `fkv_store/src/lib.rs:949-968` 归一化失败改 `debug_assert!`+`log::warn!`+回退 `MAIN_BRANCH_NAME`；`tests/naming.rs:109` 双 profile 断言 |

## 判定明细

### 通过（7，已归档 reviewed）

- **r8a-01**：DONE 两项声明属实；`grep -rn MAX_OVERFLOW_POINTERS crate tool web cpp fixtures` = 3 处（定义+导入+比较），`grep DirectIoBuf crate` 于 `fkv_buf` 定义处 0 命中。零死代码纪律（SKILL §12）满足。**残余**：验收第 3 条边界测试未补，拆出 v-01。
- **r8a-02**：源码/doc 中 `issue/new/08` 零命中（仅本条 issue 正文保留该串）。`grep -rn "issue/new/08" crate doc` 空。
- **r8b-01**：`grep -n 预留 COMPAT_BASELINE` 命中 branch file id 1024 条目；`git diff` 确认 38 份 `fixtures/**/meta.txt` 的 `baseline=` 在刷并含该条（例 `fixtures/rust/page/empty/meta.txt`）。
- **r8b-02**：`COMPAT_BASELINE:61-63` 与 `fkv_store/src/lib.rs:342/:496-502` 表述一致；`doc/progress.md:6` 命中 `ttl_read_filter`。
- **r9a-02**：`grep -rn -w put_fixed64 crate tool web` = 定义 + 2 生产消费 + tests；消费点与 `get_fixed64` 读侧对称，替换原内联 `to_le_bytes` 写出，语义等价。
- **r9b-01**：`grep -rn "fkv_task_compact::compact_file\|fkv_task_compact::compact_segment_file" crate` 0 命中；删除连带清 re-export（`grep "pub use"` 于 fkv_buf/fkv_task_compact 无）与 doc 引用。
- **r9b-03**：方案 2 落地并带测试；合法名路径逐字节不变。

### 不完善（1，留 done）

- **r9a-03**：DONE 自述「接受部分保留」，实际 0 清理——6 个编号在 scope 内全部原样保留、且无对应文件：
  - `fkv_btree/src/lib.rs:244`（core-04）
  - `fkv_mapping/src/lib.rs:629`（core-04）、`:806`（core-01）
  - `fkv_codec/src/lib.rs:85`（fmt-13）
  - `fkv_compress/src/lib.rs:253`（fmt-12）、`:162`（fmt-22）
  - `fkv_compress/tests/compress.rs:133`（fmt-22）
  - `fkv_large/src/managed.rs:151`（io-06）
  `find issue -name "<id>*.md"` 六个编号全部 0 命中。验收要求「≥1 命中或编号已从注释移除」，两者皆不满足。verdict：不完善，留 `done/` 待下轮（本条自身即残余载体，未另立 issue，遵守「宁缺毋滥」）。

## 新立项

- [`v-01`](../new/v-01-fkv-config-overflow-pointers-validate-boundary-untested.md)：fkv_config `overflow_pointers` validate 的 0/128/129 边界无任何测试（r8a-01 验收残余）。已查重：`issue/` 无承接条目。

## 在途 / 停止条件

- lane A 8 条均仍在 `issue/done/`，无被并行会话移走，无跳过。
- 在途观察（未阻断判定）：`fkv_store/src/lib.rs` mtime 07:49（近 5 分钟），`git diff` 只动 `:2847` archive 分支，未触 r9b-03 的 `with_branch` 锚点；`fkv_btree/src/lib.rs` 在途 diff 为 `options→option` 重命名，未触 r9a-03 的 `:244`。
- 旁路：`issue/new/r10-cloud-findings.md` 已被并行会话移至 `issue/reviewed/`（非 lane A，未干涉）。

## 结论

lane A：完善 7 / 不完善 1（r9a-03）。新立项 1（v-01）。未改动任何 `crate/` 代码或文档，未运行 cargo。
