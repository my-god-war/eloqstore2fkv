# io-41 P3：segment 尾读语义四分裂 + PageIo/FileIo 跨后端契约不明（错误码 / 列表顺序）

来源：第 5 轮 io/compio lane 侦查（两条相邻的跨后端契约观察合并成单文件）。

## 证据

「文件短于请求」的行为按后端分裂：

- compio plain `read_segment`：短读即 `Err(IoFail)`（`fkv_io_compio/src/lib.rs:2200-2210`），
  且 Err 路径 `DirectBuf` 直接 drop 不回池（成功路径才 `put_direct_buf`）；
- compio fixed 读：短读**零填充**（`:2347-2350` `fill(0)`）；
- `fkv_io_sim`（`:1007-1009`）与 `fkv_io_mem`（`:267-269`）：均零填充。

C++ `ReadSegments` 短读是重发（fallocate 保证整段，实际不触发）。

错误码 / 顺序契约同样分裂：

- 缺页：Sim/Mem 报 `ResourceMissing`（`fkv_io_sim:596`、`fkv_io_mem:104-106`），
  compio 报 `Corrupted`（`:1859`）——Sim 崩溃测试永远走不到上层 Corrupted 分支。
- `list_file_id_li`：compio（`:1659`）与 Sim（`:1238`）sort+dedup，
  **Mem 不排序**（`fkv_io_mem:387-395` `set.into_iter().collect()`）——
  依赖顺序的 GC 遍历 / 确定性测试在 Mem 后端不可复现。

## 目标

1. `fkv_io` trait 文档写明契约：尾读语义（建议统一零填充，对齐多数实现）、
   缺页错误码、`list_*` 返回「升序去重」。实现向契约收敛：
   plain 尾读改零填充或 fixed 改报错，选一；Mem 排序；错误码统一。
2. `read_segment` Err 路径归还 `DirectBuf`（与 `read_whole` 的归还对齐，
   保对齐池命中率）。

## 验收

- 三后端同套契约测试（放 `fkv_io` 的测试或各后端镜像断言）：
  短文件读、缺页读、列表顺序行为一致。
- `cargo nextest run -p fkv_io -p fkv_io_sim -p fkv_io_mem -p fkv_io_compio`。

## 依赖 / 重叠

- io-03（done）修的是双拷贝/对齐池；io-39 是 EOF 错误码（data 页）——本条是
  segment/file 面的契约统一，互补。

## 核验记录

- 子代理摘录（协调者未逐行复核，开发时按行号验证）：各后端行号如上。

## 审查结论（dragon 复核，2026-11-04）

**成立（契约确实四分裂），本条拆成「已做」与「留 todo」两半。**

逐条实核（行号已随文件变动）：

1. ✅ compio plain `read_segment` 短读 → `Err(IoFail)`（`:2263-2266`）。
2. ⚠️ **原报告这条要改**：`read_segment` 的 **DirectBuf 归还**并非「Err 路径一律
   drop」——短读分支**有**归还（`:2265` `self.put_direct_buf(buf.into_inner())` 后再
   `return Err(IoFail)`）；真正漏归还的是 **syscall 报错**分支：`read_at(...)` 拿到
   `BufResult(result, buf)` 后 `result.map_err(map_io_err)?` 直接 `?`，`buf` 随作用域
   drop 不回池。修法照旧（`match` 里先归还再传播），但结论要写准。
3. ✅ compio fixed 读短读零填充；sim / mem 也零填充（compio plain 是唯一报错的）。
4. ✅ **缺页错误码分裂**：sim（`:597`/`:1006`）/ mem（`:106`/`:266`）报
   `ResourceMissing`，compio 的页读在循环耗尽后报 `Corrupted`（`:1905`）。
5. ✅ **`list_file_id_li` 顺序分裂**：compio / sim 排序，mem 不排序
   （`fkv_io_mem/src/lib.rs:387-395` 原来直接吐 `HashSet` 迭代序）。**注意同一个文件里
   `list_segment_file_id_li`（`:407-416`）和 `list_manifest_file_name_li`（`:199-209`）
   都是排过序的** —— 只有这一个漏了，属实现疏漏而非设计选择。

## DONE（2026-11-04，第 5 条）

- `fkv_io_mem::list_file_id_li` 改为排序返回（与同文件另两个 `list_*` 及 compio/sim 一致）。
- 新增 `fkv_io_mem/tests/mem.rs::list_file_id_li_is_sorted_ascending`：乱序写入
  `{7,0,3,5,1,9}` 六个 file id（外加同 file 的第二页，验证去重）后断言
  `== [0,1,3,5,7,9]` 且 `is_sorted()`。
  **反向验证**：去掉 `sort_unstable()` → FAIL，实测返回 `[5,7,1,3,0,9]`
  —— 这就是 Mem 后端 GC 遍历 / 确定性测试之前的真实顺序。
- `cargo nextest run -p fkv_io_mem -p fkv_io_sim -p fkv_io` 17 passed；
  `cargo clippy -p fkv_io_mem --all-targets --no-deps -- -D warnings` EXIT=0。

## 阻塞（留 todo：第 1-4 条 + 契约测试）

1. **契约落文档**：`fkv_io` trait 上写明「尾读语义 / 缺页错误码 / `list_*` 升序去重」
   三件事。`fkv_io`（`:372` 一带）是并行会话热点，交 owner。
2. **尾读语义二选一**：plain 改零填充（推荐，三个后端里两个已零填充）或 fixed 改报错。
   涉及 `fkv_io_compio`（热点）。
3. **缺页错误码统一**：建议统一为 `ResourceMissing`（compio 的 `Corrupted` 会让
   「文件不存在」与「数据损坏」在上层无法区分，retry 语义也不同）。
   涉及 `fkv_io_compio`（热点）+ 可能影响 `fkv_store` 的 retry 判定，需与 owner 同批。
4. **DirectBuf 归还**：把 `read_at` 的错误分支改成先归还再传播（见上面第 2 点改判）。
5. **契约测试**：三后端同套断言（短文件读 / 缺页读 / 列表顺序）——放 `fkv_io` 的
   测试或各后端镜像，等 1-3 落定后一次写，避免反复改断言。

## DONE（2026-11-05）

### 契约落定（`fkv_io/src/lib.rs`）

- `PageIo::read_page` 写明错误码：
  - 资源不存在（文件 / 页从未落盘）→ `ResourceMissing`；
  - 后端有文件长度、请求越过 EOF → `EndOfFile`；
  - 部分页 / checksum 失败 → `Corrupted`；底层 IO 错误 → `IoFail`。
- `SegmentIo::read_segment` / `FixedSegmentIo::read_segment_fixed`：
  - 段按 `segment_size` 定长、尾部补零；文件短于请求时读取可用前缀，剩余零填充并返回 `Ok(())`；
  - 段文件不存在 → `ResourceMissing`；`dst.len() > segment_size` / fixed 越界 → `InvalidArgs`；
  - 空读 → `Ok(())`（不要求段存在）；短读不返回 `EndOfFile` / `Corrupted`。
- `FileIo::list_file_id_li` / `list_segment_file_id_li`：写明**升序、去重**。

### 实现收敛

- `fkv_io_compio` plain `read_segment`：短读从 `Err(IoFail)` 改为零填充（对齐 fixed / Mem / Sim）；
  同时修复 `read_at` syscall 错误分支先 `put_direct_buf` / `put_read_buf` 再传播
  （原 `result.map_err(map_io_err)?` 会让缓冲随作用域 drop，漏回池）。
- `fkv_io_mem` / `fkv_io_sim`：空读提前返回 `Ok(())`，与 compio 一致（此前会先查段、缺段报
  `ResourceMissing`）。
- `list_*` 排序：Mem 的 `list_file_id_li` 上一轮已修；`list_segment_file_id_li` 与 compio / sim
  本就排序，本轮只补契约与测试，无生产代码改动。

### 测试

- 新增三份镜像契约测试：
  - `crate/io/fkv_io_mem/tests/contract.rs`
  - `crate/io/fkv_io_sim/tests/contract.rs`
  - `crate/io/fkv_io_compio/tests/contract.rs`
  覆盖：segment 尾读零填充、空读、超长 dst、缺段 / 缺页错误码、fixed 尾读不越界改写、
  `list_*` 升序去重（compio 额外覆盖同 file id 的 term 变体去重）。
- 更新 `fkv_io_compio/tests/hardening.rs`：`truncated_page_fails_and_short_segment_zero_fills`
  保留 page 短读 `Corrupted`，segment 短读改为断言零填充。
- 验证：
  - `cargo nextest run -p fkv_io -p fkv_io_sim -p fkv_io_mem -p fkv_io_compio` → 72 passed；
  - `cargo clippy -p fkv_io -p fkv_io_mem -p fkv_io_sim -p fkv_io_compio --all-targets --no-deps -- -D warnings` → EXIT=0；
  - `cargo +nightly fmt -p fkv_io -p fkv_io_mem -p fkv_io_sim -p fkv_io_compio`。

### 偏离（有意保留）

- **缺页错误码没有统一为 `ResourceMissing`**。`io-39`（done）已把 compio 的
  「文件存在但请求越过 EOF」改为 `EndOfFile`、部分页改 `Corrupted`（对齐 C++ `res == 0` /
  `0 < res < size`）。回退成 `ResourceMissing` 会重新混淆 EOF 与数据损坏，故保留。
  三后端统一的是**资源不存在**（无文件 / 无页记录）→ `ResourceMissing`；Mem / Sim 没有
  文件长度模型，同一场景无法区分「空洞」与「截断」，仍统一报 `ResourceMissing`。
  契约与测试按「后端可区分时用 EOF/Corrupted，不可区分时 ResourceMissing」表达。

### 风险

- segment 短读零填充会掩盖「磁盘 segment 文件被外部截断」；`fkv_cloud::segment_file_complete`
  原本依赖短读报错判定本地缓存不完整。compio 的 `write_segment` 会把整文件预分配到
  `sequence_file_size`，崩溃残留本来就是全长零尾（此前也判不出），Mem / Sim 本来就零填充，
  因此本次改动不新增该类回归；外部手工截断场景从 `IoFail` 变为零填充，需要靠上层 checksum
  或 manifest 一致性发现。

## 复审（2026-11-05 r6）

通过。

- 契约文档 `fkv_io/src/lib.rs:38-44`（read_page 错误码分级：
  ResourceMissing/EndOfFile/Corrupted/IoFail + Mem/Sim 无文件长度模型的说明）、
  `:426-468`（segment 定长尾部零填充、短读不返回 EndOfFile/Corrupted、空读
  Ok、InvalidArgs 边界）、`:486/522`（list_* 升序去重）。
- 实现收敛：compio `read_segment` 短读零填充 + syscall 错误分支先归还缓冲
  （`fkv_io_compio/src/lib.rs:2359-2370`）；`fkv_io_mem/src/lib.rs:263-265`、
  `fkv_io_sim/src/lib.rs:1005-1007` 空读 no-op；`fkv_io_mem list_file_id_li`
  排序（`:392-404`）+ `tests/mem.rs:55 list_file_id_li_is_sorted_ascending`。
- 三份镜像契约测试均在：`fkv_io_mem/tests/contract.rs`、
  `fkv_io_sim/tests/contract.rs`、`fkv_io_compio/tests/contract.rs`
  （尾读零填充/空读/超长 dst/缺段缺页错误码/list 排序断言抽读属实）；
  `hardening.rs:114 truncated_page_fails_and_short_segment_zero_fills`。
- 缺页错误码不统一为 ResourceMissing 的有意偏离（EOF/Corrupted 可区分时保留）
  已在 md 记录，无异议。
