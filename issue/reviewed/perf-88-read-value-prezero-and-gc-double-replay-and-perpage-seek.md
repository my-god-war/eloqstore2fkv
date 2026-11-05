# perf-88 P3：三处小热点合并：大值读预清零白付 / GC 每轮双 replay 活跃 manifest / 分页扫描逐页重定位

来源：第 5 轮 core + read/gc lane 侦查（三条独立小点，均为局部修复，合并单文件避免碎片）。

## 1. `read_value` 预清零多余一遍 memset

`crate/core/fkv_large/src/lib.rs:248-258`：`vec![0u8; actual_len]` 后
`read_batch`（batch.rs:47-59）用 `chunks_mut` 把每个字节交给 IO——清零纯冗余。
百 MB 大 value 可感知。改 `Vec::with_capacity` + `spare_capacity_mut` 承接
（或按 SKILL 口径 `set_len`）。

## 2. GC 每轮对 `manifest.main.0` 完整读 + replay 两遍

`crate/engine/fkv_task_gc/src/lib.rs:317-324`：先 `io.read_manifest(tbl)`
（active manifest = `manifest.main.0`），再遍历
`list_manifest_file_name_li`（列表以 `FILE_NAME_MANIFEST` 前缀过滤，同名命中）
再读一遍。C++（`file_gc.cpp:680-714`）注释明确把这当主导开销优化掉
（replay + 全 mapping 遍历双倍是 GC:Augment 大头）。修法：循环里跳过 active
名字，或第一遍直接复用内存 mapper。

## 3. `scan_page_with` 每页都 `iter.seek(begin)`

`crate/engine/fkv_task_scan/src/lib.rs:207-211`：`loop` 内每页新建迭代器并
`seek(begin)`；第二页起全部 key > begin，seek 必落首 entry——每页一次多余的
区域二分 + restart 扫描；`begin_inclusive` 判定也只在首页有意义。
C++ ScanIterator 的 Next 链不重定位。修法：seek 只做一次（首页），后续页直接
从头迭代。

## 验收

- 各自既有测试全绿（`fkv_large` / `fkv_task_gc` / `fkv_task_scan`）；
  1 与 2 可加 IO/分配计数断言。

## 依赖 / 重叠

- io-03（done）是 IO 层拷贝，不同于本条应用层清零；sto-54/55（GC 族）不覆盖
  双 replay；eng-65 族（遍历契约）不覆盖重定位。

## 核验记录

- 子代理摘录（未逐行复核）：行号如上。

## 审查结论（dragon 复核，2026-11-04）

**三条分别判：① reject（收益未证 + 引入 UB 面）、② 成立（留 todo）、③ 成立（留 todo）。**

### ① `read_value` 预清零 —— **reject**

- 事实核对无误（`fkv_large/src/lib.rs:249-258` `vec![0u8; actual_len]` + `chunks_mut` 承接）。
- 但「百 MB 大 value 可感知」不成立：`vec![0u8; n]` 走 `alloc_zeroed`，n ≥ 分配器
  mmap 阈值时是**新 mmap 页**（内核已零页，memset 被省掉）。这条的目标场景恰好就是
  大 n。
- 换成 `Vec::with_capacity` + `spare_capacity_mut`/`set_len` 会把安全性压到
  「**每个 IO 后端在 Ok 时填满整个 dst**」这条**目前没有文档化的契约**上 ——
  而 io-41 刚证明三个后端在短读上行为并不一致（compio plain 直接报 `IoFail`，
  另两个零填充）。一旦某后端 Ok 却少填，读出来就是未初始化堆内存（信息泄漏级 bug）。
- 结论：**不为未观测到的收益换 UB 面**。若日后有 bench 数据证明它是热点，
  先按 io-41 把「Ok 即填满」写进 `fkv_io` trait 文档，再改。替代低风险做法：
  保留 `vec![0u8; n]`，把「清零」去掉的冲动改为在 `read_batch` 里断言
  `dst.len()` 与段数匹配（已有 `Corrupted` 校验）。

### ② GC 每轮重复 replay 活跃 manifest —— **成立**

- `fkv_task_gc/src/lib.rs:317-324`：先 `io.read_manifest(tbl)`（活跃
  `manifest_main_0`）replay 一次，再遍历 `list_manifest_file_name_li`（**列表包含
  同名活跃文件**）再读再 replay 一遍。核实无跳过逻辑。
- 修法（**不依赖任何新 API、零语义风险**）：把第一遍的 bytes 留下，循环里
  `if active_bytes.as_deref() == Some(bytes.as_slice()) { continue; }` ——
  字节相同 ⇒ 对 retained 集合 / guard 的贡献完全相同（`apply` 对集合是幂等插入，
  `fold` 取 max 幂等），跳过纯冗余的第二遍。**不要**按名字硬编码跳过
  （`manifest_file_name(MAIN,0)` 只是当前约定，跳错文件会少算 retained → GC 误删）。
- `fkv_task_gc` 不是禁改热点但近 25 分钟内被改过（`00:41`），落地前先与 owner 对齐。

### ③ 分页扫描逐页 `seek(begin)` —— **成立**

- `fkv_task_scan/src/lib.rs:207-211`：每页新建迭代器并 `seek(begin)`；第二页起
  所有 key > begin，seek 必落首 entry → 每页一次多余的二分 + restart 扫描。
- 修法：首页 seek，后续页直接从头迭代；`begin_inclusive` 只在首页判定。
  但 `fkv_task_scan/src/lib.rs` 是另一会话的**明令禁改热点**（本轮多次秒级修改），
  只能留 todo 给 owner。

## 阻塞（留 todo：②③）

① 无需动作（本条已 reject 该子项，理由如上）。

## DONE（2026-11-04）

结论：① reject（沿用上文 dragon 复核）；② ③ 落地。

- ② `crate/engine/fkv_task_gc/src/lib.rs::manifest_retained_file_page_set`：
  活跃 manifest 仍先 `read_manifest` + `apply`；随后列表里内容与之相同的具名文件
  跳过第二遍 `apply`（`apply` 对集合 / guard 幂等），不按文件名硬编码，命名约定
  变化时也不会漏算 retained。
- ③ `crate/engine/fkv_task_scan/src/lib.rs::scan_page_with`：`seek(begin)` 与
  `begin_inclusive` 判定只在首页执行，叶子链后续页直接 `advance()` 从首 entry 迭代。
- ① `read_value` 保持 `vec![0u8; n]`（`alloc_zeroed`，大 value 走 mmap 零页），
  不动 IO 后端「Ok 即填满 dst」这条未文档化的隐式契约。

验证：

- `cargo check -p fkv_task_read -p fkv_task_gc -p fkv_store`（另含 `fkv_large` / `fkv_task_scan`）通过
- `cargo nextest run -p fkv_task_read -p fkv_task_gc -p fkv_store -p fkv_runtime -p fkv_large -p fkv_task_scan --no-fail-fast`：188 passed / 1 skipped
- clippy `--no-deps --all-features --all-targets -- -D warnings`：零警告

## 复审（2026-11-05 r6）

通过。三子项分别核验：

- ①（reject）：`fkv_large/src/lib.rs:249` 仍为 `vec![0u8; actual_len]`（`alloc_zeroed`），
  未引入 `spare_capacity_mut`/`set_len`——与 reject 结论一致。
- ②：`fkv_task_gc/src/lib.rs:388-393`（`manifest_retained_file_page_set`）循环里
  `if active.as_deref() == Some(bytes.as_slice()) { continue; }` 跳过与活跃 manifest 字节
  相同的具名文件，不按文件名硬编码——与 dragon 复核修法逐字一致。
- ③：`fkv_task_scan/src/lib.rs:180`（`first_page` 标志）+ `:204-212`（仅首页
  `iter.seek(begin)` 与 `begin_inclusive` 判定，后续页直接 `advance()` 从首 entry 迭代）。
