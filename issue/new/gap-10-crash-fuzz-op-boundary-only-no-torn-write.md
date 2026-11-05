# gap-10 崩溃 fuzz 只在「op 边界」crash；SimStore 无 mid-op 窗口，故 intra-op fuzz 目前是空加覆盖（P1）

## 来源

对 gap-03/04/07 实现的复审（2026-11-05，fkv HEAD `b5073e1`）。先声明：这批实现质量高
——`op_log_replay.rs` 有 6 条 op log × 每个 op 索引 crash → restart → reopen →
与干净前缀逐字节比对，外加子进程真 `!kill` 用例。本条**不是说它没做**，
而是说 gap-04 验收里的两个点在当前模型下**结构上无法达成**，且**不能靠「给 SimStore
再加 mid-op 窗口 + 一层 intra-op fuzz」补上**。

## 现象（决定性证据）

`fkv_io_sim` 里 15 处注入点**全部位于所属函数的顶部、任何状态变更之前**：

```
$ rg -n "fkv_failpoint::(fail_point|kill_point)" crate/io/fkv_io_sim/src/lib.rs
643/644   WritePage            (write_page：在 log_op 与 volatile 写入之前)
664/665   SyncData             (sync_data：在 log_op 与 flush 循环之前)
774/775   AppendManifest:Write (append_manifest：在 log_op 之前)
776       AppendManifest:Sync  (同上，与 Write 同一位置)
794/795   AppendManifest:Write (append_manifest_file，同上)
796       AppendManifest:Sync  (同上)
820/821   AtomicWriteFile:Rename / :Sync  (switch_manifest，在 log_op 之前)
862/863   AtomicWriteFile:Rename / :Sync  (write_manifest_file，同上)
1069/1070 WriteSegments        (write_segment，在 log_op 之前)
1201/1202 WriteFile:SyncFail / WriteFile:Rename (write_file，在 log_op 之前)
```

关键：`sync_data` 的 **flush 循环内部没有任何注入点**（`fkv_io_sim/src/lib.rs:663-712`
整段只有函数顶部那一对；segment 刷写循环同样没有）。

于是：

1. 在任一注入点 abort（`!kill`）≡ **「这个 op 完全没被应用」**。
2. 而「op k 未应用」这一状态，**op 边界 fuzz 已经全覆盖**——
   `crash_reopen(prefix_len=k)` 就是「应用 k 条后 crash」，与「第 k 条 op 起始处 kill」
   产出同一份 durable 状态。
3. 且 `crash()` 只清 `volatile`（`fkv_io_sim/src/lib.rs:399-403`）、manifest 直接写
   `durable`，场景又全为小 value（`rg -n put_large op_log_replay.rs` → 无命中），
   所以每个 op 结束时 `volatile` 整体为空。

⇒ **在 sim 上再叠一层按 failpoint 枚举的 intra-op fuzz，新增覆盖为零。**

## 修正后的方向：闭环打到真后端

真正的两阶段窗口在 compio 侧，且已由现有 kill point 可达：

| 窗口 | 注入点 | 位置 |
|---|---|---|
| 追加 record 已 `write_at`、未 `fdatasync`（撕裂尾） | `AppendManifest:Write` | `fkv_io_compio/src/lib.rs:1900`（`:1887` 之后、`:1904` 之前） |
| record 内容已 durable、目录项未 fsync | `AppendManifest:Sync` | `:1941` |
| tmp 已 fdatasync、未 rename | `AtomicWriteFile:Sync` / `:Rename` | `:1962` / `:1964`（`:1961` 之后、`:1966` 之前） |

读侧的撕裂恢复分支此前**没有任何测试经过真实 store 走到**：SimStore 永远产生不出撕裂字节，
`fkv_task_reopen/src/lib.rs` 中「torn body」与「body 完好但 padding 未落盘」两处 `break`
长期只被单元级用例覆盖。**反向验证**：把这两处去掉，测试必须变红。

SimStore 侧要补的是**它自己的模型**（tmp→rename 两段式、字节级部分 record、
目录项耐久性），那是另一件事；在模型补齐之前，不要在 sim 上做 intra-op fuzz ——
它会给出「已覆盖撕裂写」的假象。该部分已移交给 gap-12 记录 compio 侧的位置缺口。

## 验收

- 存在在**真实后端**的 failpoint 处中止进程、并在重开后校验不变量的用例；
- 不变量是「状态等于某个 op 前缀」而非「进程 abort 了」；
- 撕裂尾有物化用例（盘上截断 / 垃圾尾 / 中间损坏），且去掉读侧 `break` 会变红；
- 若只做了 sim 侧 intra-op fuzz 而未补 sim 模型，视为未完成。
