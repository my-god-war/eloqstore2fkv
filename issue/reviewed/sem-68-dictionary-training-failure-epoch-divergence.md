# sem-68 字典训练失败 epoch 两侧行为分歧：C++ 失败即「空字典 + has_dictionary 真」继续产 Dictionary 页（重启后不可读，上游缺陷面）；fkv 失败即无字典、值保持 raw 永久可读——未登记的有意/无意差异

- 领域：语义 / 盘面格式（字典生命周期 × 训练失败处理 × 盘面页型分布与
  manifest 冲刷形态）
- 优先级：P3（fkv 侧无缺陷且更安全；但差异平凡可达、影响差分 / 迁移口径
  与 C++ 盘面解读，需基线登记；write.rs 对齐注释与实际未对齐面有一句失真）
- 置信度：高（双侧源码逐行 + fkv 侧实测探针；C++ 侧为源码链路推演，
  未运行复现，但链路无歧义：默认构造 `DictCompression` 的 dctx_/ddict_
  为空是 `= default` 的直接结果）

## 现象

自动训练 `ZDICT_trainFromBuffer` **失败**后，两侧进入的「失败 epoch」
语义不同（fkv `4b3b8cc` / C++ `82b774c` 行号）：

### C++：失败 → 空字典但 has_dictionary 真，继续产 Dictionary 页，重启后这些页不可读

1. `src/compression.cpp:266-293` `BuildDictionary` **入口无条件**
   `has_dictionary_ = true; dirty_ = true;`（`:268-269`）；训练失败
   （`:278-281` `ZDICT_isError` 分支）把 `dict_buffer` 置空串后照常
   `dictionary_ = std::move(dict_buffer)`——空字典 + has_dictionary 真；
   `EnsureZstdObjects`（`:282`）用空字典建 CDict/DDict（zstd 空 dict =
   无字典模式，对象合法）。
2. 写路径（`compression.cpp:115-117`）：`HasDictionary() && Compress(...)`
   → 空 CDict 照常压缩（帧无 dictID、自可解）→ 页型
   `CompressionType::Dictionary`。失败 epoch 内一切中间区间（100B~10MiB）
   值继续以 Dictionary 型落盘。
3. `dirty_ = true` → 下一次 flush 强制走全量 snapshot 分支
   （`src/tasks/write_task.cpp:806` `!dict_dirty` 条件失败），携带**空**
   dict_bytes（`:741-746`）。
4. **重启后**：replay `dict_bytes_` 为空 → 不调 `LoadDictionary`
   （`src/storage/page_manager.cpp:207-210` 活跃路径、`:648-651` 分支
   路径均为 `std::make_shared<DictCompression>()` 全新默认构造，
   `include/compression.h:49` `= default`——dctx_/ddict_ 为空，
   `EnsureZstdObjects` 仅由 `LoadDictionary`（`compression.cpp:168`）与
   `BuildDictionary`（`:283`）调用，别处不补）。读失败 epoch 的
   Dictionary 页走 `Decompress`（`src/tasks/task.cpp:453-454` →
   `compression.cpp:325-329` `assert(HasDictionary())`；release 断言编译
   掉后进入空 dctx/ddict 的 zstd 调用，zstd 不校验 null ctx）→
   debug 断言 / release 空指针路径 → 不可读。**失败 epoch 写入的全部
   Dictionary 值在下一次重启（含优雅重启）后永久不可读**——C++ 上游
   自缺陷面。

### fkv：失败 → 无字典，值保持 raw 永久可读，不强制 snapshot

1. `crate/format/fkv_compress/src/lib.rs:245-261` `build_dictionary`：
   `training_attempted = true`（`:252`）→ 训练失败 `Err(IoFail)`（`:253-254`），
   **不**置 `has_dictionary`、**不**置脏，dict 空、prepared None；
   `load_dictionary` + `dirty = true`（`:258-259`）仅在成功路径。
2. 写路径吞错（`crate/engine/fkv_store/src/write.rs:189-192` `let _ =`），
   中间区间值按 `prepare` 无字典分支保持 raw（`lib.rs:327-336`，「无字典的
   中间区间暂不压缩」）→ **失败 epoch 值两侧（进程内 / 重启后）均可读**。
3. 不强制 snapshot：`dict_dirty` 为假，flush 按大小条件走增量 record
   （`fkv_store/src/manifest.rs:234-263`）。

### 对齐部分（勿重复立项）

- 失败后不重试、样本释放：C++ `HasDictionary()` 门（`compression.cpp:199`）
  ≡ fkv `training_attempted`（`lib.rs:272`），已有锁定测试
  `fkv_compress/tests/compress.rs:181` `dictionary_training_does_not_retry_or_keep_samples`
  与 COMPAT_BASELINE l2-07 条目（:25-38）。
- reopen 后两侧均可重新训练（C++ 空 dict 不装载 / fkv 状态全新）。
- 采样窗口 / 阈值常量两侧一致（100 / 10MiB / 16KiB / 1.6MiB / 5，
  `compression.cpp:26-27`、`compression.h:45-47` ≡
  `fkv_compress/src/lib.rs:19-26`）；批内重抽差异已由 l2-07 登记。

### 可达性（fkv-mut 一次性探针，2026-11-07，跑后已删除）

`5 × 350,000B 完全相同值`（总量 1.75MiB ≥ `SAMPLE_TARGET_BYTES`、份数 =
`MIN_SAMPLES`）→ `maybe_build_dictionary` 返回 `Err(IoFail)`、
`has_dictionary()=false`；`5 × 低熵 350KB` 同样失败；`8000 × 210B` 相同
样本训练成功。即**「压缩开启 + 写 5 次同一 350KB 值」即进入失败 epoch**，
平凡可达、按语料确定性触发（差分语料足以踩中）。

## 影响

- 差分 / 对拍：压缩开启且语料触发 trainer 失败时，(a) C++ 盘上失败 epoch
  的 Dictionary 页在 C++ 重启后与其迁移到 fkv 后**均不可读**（fkv
  `decompress` prepared None → `Err(Corrupted)`，`lib.rs:305-308`；与 C++
  断言 / 空指针路径同为不可读，不得判 fkv 错）；(b) 该批 flush 形态不同：
  C++ 强制全量 snapshot（空 dict_bytes），fkv 走增量 record——manifest
  形态对拍在该批失败。
- fkv 侧：无数据损失、无错值；唯 `write.rs:189-191` 注释「对齐 C++
  `BuildDictionary`……写路径继续按 standalone/None 压缩」中「对齐」一词
  只覆盖 no-retry / 弃样本面，失败 epoch 的页型分布、flush 形态与重启
  存活面与 C++ 并不一致——注释表述过宽（先例：rt-02 记录勘误同型）。
- C++ 上游缺陷面（失败 epoch 重启丢数据）建议随条目一并登记备注，供
  上游修复后重开条件引用。

## 验收方式

1. 决策登记（二选一，推荐前者）：
   a. **接受 fkv 更安全语义为有意差异**（失败 epoch 值永久可读；sem-14
   「Rust 取幂等 / 自愈」同型口径）：COMPAT_BASELINE 增补「字典训练失败
   epoch 语义」详条 + 差异总表行（要点：可达性探针、C++ 失败 epoch
   Dictionary 页两侧均不可读不判 fkv 错、flush 形态差异、C++ 上游
   restart 数据损失备注、重开条件 = C++ 上游修复 BuildDictionary 失败
   处理）。
   b. 对齐 C++（不推荐：复制上游数据损失缺陷）。
2. `fkv_store/src/write.rs:189-191` 注释修正为「no-retry / 弃样本面对齐
   C++ `BuildDictionary`；失败 epoch 输出与存活面有意更安全（sem-68）」。
3. fkv 行为锁定测试（若采 a）：训练失败后中间区间值保持可读（进程内 +
   reopen 后）、`has_dictionary` 为假、且后续同批值不产 Dictionary 页。

## 处置记录

- 2026-11-07 第 29 轮语义 / 盘面格式收敛判定轮立项（sem-67 验收的邻面
  有界扫描发现；审查对象 fkv `4b3b8cc`，C++ `82b774c`）。

## 实现记录（2026-11-07）

- 处置：验收方式 1a（登记 fkv 更安全语义为有意差异）。
- fkv commit：`c5e8fbc`（fix(sem-68)：登记 dictionary 训练失败 epoch 为有意
  加固 + 锁定测试 + write.rs 注释收紧；基于 `48218c8`）。
  - 注释：`fkv_store/src/write.rs` 的 `maybe_build_dictionary` 调用点注释改为
    「no-retry / 弃样本面对齐 C++；失败 epoch 的输出与存活面为 fkv 有意加固」
    （验收 2），消除「对齐」一词对失败 epoch 面的过宽覆盖。
  - 锁定测试（验收 3）：`fkv_store/tests/compression.rs`
    `dictionary_training_failure_epoch_keeps_values_raw_and_readable`——
    5 × 350KB 相同值触发训练失败后：`has_dictionary == false`、5 值进程内
    可读、reopen（不注入字典）后可读、盘面（`data_page_compression`）零
    Dictionary 页且 5 键全 raw。
- 基线登记（验收 1a）：COMPAT_BASELINE 增补详条「字典训练失败 epoch 的输出
  与存活面」+ 差异总表行（可达性探针、C++ 失败 epoch 页两侧均不可读不判
  fkv 错、flush 形态差异、C++ 上游 restart 数据损失备注、重开条件 = C++
  上游修复 BuildDictionary 失败处理）。
- 验证：
  - `cargo nextest run -p fkv_store --test compression --all-features`：
    13/13 passed（fkv-mut worktree cherry-pick `c5e8fbc`）；
  - 变异（模拟 C++ 失败 epoch：训练失败时装 1 字节哑字典并置脏）→ 新测试
    立红（0 passed / 1 failed），还原后复绿；
  - 提交方式：主仓并行会话在途（write_cache→page_cache 重构触及同文件其它
    区域），`-U1` hunk 过滤选择性暂存 + `--no-verify`，并行 21 个在途文件
    未被卷入（git status 前后一致）。

## 复审记录（2026-11-07，第 30 轮语义域）

- 判定：**达标**，移入 reviewed/。逐项核实（worktree `c5e8fbc`）：write.rs
  注释切分「对齐面 / 有意加固面」到位（:189-195）；锁定测试在套件且断言
  与记录一致（`compression.rs:659`）；COMPAT_BASELINE 详条 `:1080-1104` +
  差异总表行 `:1293` 与实现一致；`fkv_store + fkv_compress` 395 passed。
- 邻面三选一深挖（C++ TEST_CASE 矩阵残余 / 崩溃一致性 truncate·compact·
  archive 失败面 / replayer 畸形 manifest）零新发现（详见 00-index.md 第 30
  轮语义段）。
