# io-31 P1：read_page 短读耗尽 / checksum 失败仍落旧 term 候选，checksum 通过即静默陈旧页

> **补立实体（2026-11-05 r6），原仅记录于 00-index**：第二轮 P1 表
> `issue/new/00-index.md:134` 与建议顺序 `:173`；r6 io lane 报告确认无实体、
> 无 owner（`issue/reviewed/00-r6-io-bench.md:79-80`、遗留风险 1 `:84-86`、
> 缺口清单 `:112`）。本文件为该议题的唯一实体，2026-11-05 按当前源码重新核验。

## 现状判定（2026-11-05 亲验）

原议题两个触发面，现状一修一存：

1. **瞬时 TryAgain/EAGAIN → 已修**：`crate/io/fkv_io_compio/src/lib.rs:1942-1954`
   瞬时错误只对同一路径重试（`is_retryable()` = TryAgain/Busy/OpenFileLimit，
   `crate/foundation/fkv_error/src/lib.rs:58-60`），耗尽后 `return Err` 直接上抛，
   **不**回退旧 term 候选。注释 `:1942` 明确写了修 io-31 的意图
   （「避免落到旧 term 候选读回陈旧页」）。系 io-22（errno 映射 + 短 IO 重试）
   与 io-39（EOF/Corrupted 分流 + is_retryable 门控）的副产品。
2. **短读耗尽 / checksum 失败 → 仍在**：见下。本实体收窄为跟踪该残余窗口。

## 现状证据（Rust，2026-11-05 快照）

- `fkv_io_compio/src/lib.rs:1074-1091` `collect_data_path_li`：同一 file id 的
  候选路径 = 当前 identity 优先 + 其余 term 降序；注释明言「旧 term 页必须能
  回退读取」。写侧 `:1113-1115` `write_path` 只按当前 identity 生成 → 同一
  `file_page_id` 可同时存在新/旧 term 两文件，旧文件活到 GC。
- `:1911-1983` `read_page`：
  - `:1936` `for path in path_li` 逐候选尝试；
  - `:1964-1968` 短读（`0 < read_len < page_size`）`continue` 同路径重试
    `READ_RETRY_TIMES = 3`（`:88`）；**内层耗尽后外层 `:1936` 仍推进到旧 term
    候选**，旧页整页 + checksum 通过即 `:1971-1973` `return Ok(())`——调用方
    拿到旧 term 页，无错误、无信号、无日志。`fast path` 注释 `:1942` 宣称
    「短读只对同一路径重试」仅对 3 次尝试内成立，耗尽路径与描述不符。
  - `:1975` 当前候选整页 checksum 失败 → `break` 推进下一候选，同样可静默
    接受旧 term 页（该分支兼作稀疏洞机制，见待查点 1）。
  - `:1978-1982` 仅全部候选耗尽才报 `Corrupted` / `EndOfFile`。
  - `:1987-2004` `read_page_li` 逐页委托 `read_page`，批量路径继承同一窗口
    （eng-63a 对齐的 `ReadPages` 同理受影响）。
- 与 27b 的关系：`issue/reviewed/27b-read-path-hold-mapping-snapshot.md` 固定
  的是**逻辑映射**（逻辑页 → file_page_id + 索引缓存代际），不触 IO 层的
  物理候选解析；本窗口位于映射层之下，27b 未覆盖。

## C++ 对照（eloqstore 82b774c）

- `src/async_io_manager.cpp:710-790` `IouringMgr::ReadPage`：
  `:717-726` 经 `GetBranchNameAndTerm`（`include/common.h:676-689`，对
  `BranchFileMapping` 区间二分）**唯一确定** `(branch, term)`，只开一个文件，
  **无任何候选迭代**；
  `:740-773` do-while 仅对**同一 fd** 重试短读 / TryAgain；`:765-770`
  `res == 0` → `EndOfFile`；`:783-788` checksum 失败 → `Corrupted`。
  任何异常都以错误上抛，从不静默改读更旧 term 的文件。C++ 的守卫是
  「per-file term 唯一映射 + 同 fd 重试」，并非 term 回退带守卫——根本不回退。
- `ReadPages`（`:792` 起）同一模式（`:822-831` 同样先 GetBranchNameAndTerm）。

## 语义论证（为何短读回退是错的，而 EOF 回退是对的）

- 追加式、整页写的文件尺寸恒页对齐：未迁移页读取得 **EOF**（offset ≥ size，
  `:1959-1963`，io-39 保留的设计）；已建新 term 文件中的**洞**读出零页 →
  checksum 失败 → 回退是唯一能读到旧页的机制（`:1931-1932` 注释）。
- **短读**（页对齐 offset 上 `0 < len < page_size`）只可能来自 torn 写 /
  中途截断 / 外部破坏，不是「尚未迁移」信号；此时旧 term 若有同名页，其
  内容是被新 term 覆写前的旧版本——静默返回即陈旧读（违背已提交内容的
  可见性），且掩盖了当前 term 文件的损坏事实。C++ 对应场景报错。

## 修法

1. **最小修正（推荐）**：短读耗尽同路径重试后视为终局信号，直接
   `return Err(KvError::Corrupted)`，停止推进旧 term 候选（对齐 C++「短读
   属于该文件自身损坏」语义）；EOF 与 checksum 失败两分支维持候选回退
   （迁移 / 稀疏洞机制不动）。同时：任何旧 term 回退命中时打点（日志或
   metric）——当前全程静默，窗口完全不可观测。
2. **彻底对齐（可选，与 owner 评估）**：本地 backend 实现
   `set_file_range_li`（`crate/io/fkv_io/src/lib.rs:17-19`，本地后端当前忽略）
   按 `BranchFileRange` 唯一解析 file id → (branch, term)，复刻 C++
   `GetBranchNameAndTerm`，从根上消除候选歧义（远端对象 backend 已有此链）。

## 验收

- 新增 `fkv_io_compio/tests/hardening.rs` 用例：
  1. 同一 `file_page_id` 造新/旧 term 两候选文件（页内容不同），把新 term
     文件截断到**非页对齐**长度模拟 torn：断言 `read_page` 返回 `Corrupted`
     （不是旧 term 页、不无限重试）；
  2. 稀疏洞回归：新 term 文件该页为洞（零页）、旧 term 有数据页 → 仍回退
     读到旧页（防止修法误杀迁移机制）；
  3. EOF 回归：新 term 文件长度不及该页 offset → 仍回退旧 term
     （io-39 既有语义不变，沿用 `empty_page_file_returns_end_of_file`）。
- `cargo nextest run -p fkv_io_compio`；clippy 零警告。
- linux-gated 路径（O_DIRECT）在 Linux 容器实跑或交叉编译 clippy 为凭
  （沿 r6 惯例，`00-r6-io-bench.md:97`）。

## 待查点

1. checksum 失败 → 旧 term 回退是否保留：最小修法下保留，意味着 torn
   （非零）页仍会静默换旧值；彻底消除依赖修法 2。需 owner 决策记录。
2. C++ `:772-773` 对确定性短读的 do-while 理论上自旋（其隐含假设 data 文件
   恒页对齐）；Rust 修法用有限重试 + 终局 Corrupted，勿复刻该循环。
3. 旧 term data 文件的存活期决定窗口时长：manifest 剪枝（`fkv_io/src/lib.rs:188-222`）
   只覆盖 manifest；data 文件的旧 term GC 时机与 `todo/rev-39`（GC file-id
   粒度）联动核查。
4. 与 `todo/eng-89`（compact 缺 data fsync）相互放大：manifest 提交前数据页
   torn 使短读真正可达，建议同批或先修 eng-89。

## 依赖 / 并发

- `fkv_io_compio` 为并行会话热点文件（`00-r6-io-bench.md:94-95` 遗留风险 7），
  落地前与 io lane owner 对批；第二轮建议顺序（00-index `:173`）将其列为
  并发化 03/eng-32 落地前必修。

## DONE（2026-11-05）

落实修法 1（最小修正）：短读耗尽终局化、瞬时错误只重试同路径、旧 term 回退
打点。未做修法 2（本地 backend 按 `BranchFileRange` 唯一解析 term），留 owner
评估。

改动（仅 `fkv_io_compio` 及其测试）：

- `crate/io/fkv_io_compio/src/lib.rs`
  - `read_page` 拆出 `read_page_candidate` + `CandidateRead { Hit, Eof, Miss }`：
    - 瞬时错误（`is_retryable()`：TryAgain / Busy / OpenFileLimit）与短读只在
      同一路径重试（`READ_RETRY_TIMES = 3`）；
    - **短读重试耗尽 → 终局 `Corrupted`**，不再推进旧 term 候选（原缺陷窗口）；
    - **瞬时错误重试耗尽 → 上抛原错误**（如 `TryAgain`），不换候选；
    - EOF（`read_len == 0`）与 checksum 失败保留候选回退（迁移 / 稀疏洞机制
      不动），全部候选耗尽后的 `EndOfFile` / `Corrupted` 判定不变；
    - 索引路径被外部删除（open `NotFound`）仍按「该候选不存在」跳过。
  - 可观测性：新增 `stale_read_num()`（更新候选 EOF / checksum 失败后旧 term
    命中次数）与 `read_at_num()`（页读尝试次数），均 `AtomicU64` 只读 getter。
  - 测试注入：`inject_read_try_again(times)`；`read_page_chunk` 注入
    `WouldBlock` 错误，经真实 `map_io_err` → `TryAgain` → 重试分支。
- `crate/io/fkv_io_compio/tests/hardening.rs` 新增 4 条：
  1. `short_read_exhaustion_is_terminal_not_old_term_fallback`：新 term 半页 +
     旧 term 完整页 → `Corrupted`，3 次尝试全在新 term，`stale_read_num == 0`；
  2. `checksum_hole_falls_back_to_old_term_page`：稀疏洞仍回退旧页，计数 +1；
  3. `eof_candidate_falls_back_to_old_term_page`：EOF 仍回退旧页（io-39 语义）；
  4. `transient_read_error_retries_same_path_only`：注入 2 次 `TryAgain` 后第 3
     次成功读到新 term 页；注入 3 次则上抛 `TryAgain`，dst 不被旧页覆盖。

验证：

```
cargo test -p fkv_io_compio -- --test-threads=1   # 全绿，hardening 16/16
cargo clippy -p fkv_io_compio --all-targets -- -D warnings
cargo +nightly fmt -p fkv_io_compio
```

- 并行跑 `--test failpoint` 时，进程级 `fkv_failpoint` 注册表与并行 lane 未提交
  用例 `short_manifest_append_truncates_torn_record_before_retry` 竞争，偶发两
  用例互抢注入点；`--test-threads=1` 下 2/2 通过，与本条改动无关（未改该文件）。

残余 / 语义冲突：

- 待查点 1 维持原状：checksum 失败仍回退旧 term；若新 term 出现「整页非零但
  checksum 错」的 torn 页，仍可能静默换旧值。根除依赖修法 2，需 owner 决策。
- 待查点 3/4 不变：旧 term data 文件 GC 窗口、eng-89 缺 data fsync 仍可能让
  短读可达。
- 本机 darwin，仅覆盖 buffered 路径；O_DIRECT / io_uring（linux-gated）需
  Linux 容器实跑补验。
- 未 `git commit`（按并行 lane 约定）。

## 复审（2026-11-05 r7）

**通过。** 重点核对项（短读终局 Corrupted 上抛而非静默旧页；回退打点 / 受限）逐行亲读：

- `fkv_io_compio/src/lib.rs` `read_page_candidate`（:1518-1569）：短读
  （`0 < read_len < page_size`，:1553-1557）只 `continue` 同路径重试
  `READ_RETRY_TIMES=3`（:91/:1531），循环耗尽（只有短读能走到）:1567-1568
  `return Err(KvError::Corrupted)`——**终局上抛，不落旧 term 候选**；瞬时错误
  （`is_retryable()` :1540-1544）耗尽上抛原错误（`TryAgain` 等），不换候选；
  EOF（:1549-1552）与 checksum 失败（:1561-1565）返回 `Eof` / `Miss` 保留回退，
  全部候选耗尽后 `EndOfFile` / `Corrupted` 判定不变（read_page :2298-2302）。
- 调用方 `read_page`（:2256-2303）：:2283-2285 `read_page_candidate(...).await?`
  直接传播终局错误；`read_page_li` :2316-2322 `try_join_all` 逐页委托 `read_page`，
  批量路径继承同一终局语义。
- 回退打点：`stale_read_num`（AtomicU64 :708，getter :996）仅在候选 `index > 0`
  的 `Hit` 时 +1（:2287-2291），即「更新候选 EOF / checksum 失败后旧 term 命中」
  才计数；`read_at_num`（:705/:990）每次页读尝试 +1（:1532）——回退窗口从全程
  静默变为可观测。
- 测试 4 条亲读断言（tests/hardening.rs :356-457）：短读终局用例断言 `Corrupted`
  + `read_at_num` 恰 3（全在新 term）+ `stale_read_num == 0` + dst 保持 0xEE 未被
  旧页覆盖（:370-377）；稀疏洞 / EOF 两回归用例断言仍回退旧页且计数 +1
  （:396-400/:418-421，io-39 语义未误杀）；瞬时错误用例注入 2 次第 3 次成功读新页、
  注入 3 次上抛 `TryAgain`（:439-456，`inject_read_try_again` :1010 →
  `read_page_chunk` 注入 `WouldBlock` :1482 → `map_io_err` :723 → `TryAgain`）。
- 残余（checksum 失败仍回退、本地 backend 未按 `BranchFileRange` 唯一解析 term、
  linux-gated 路径）DONE 已自认并留 owner 决策，与源码一致，非失实。

