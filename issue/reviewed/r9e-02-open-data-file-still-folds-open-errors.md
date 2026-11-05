# r9e-02 P3：r8e-03 修复残留——`open_data_file` 仍把全部非 NotFound 打开错误摊平为 `IoFail`（issue 目标 2 未做），且验收要求的非 NotFound 用例缺失

## 背景（R2 复核 r8e-03）

`issue/done/r8e-03-open-segment-file-collapse-non-notfound-errors.md` 的目标 1 已核实
在位：`open_segment_file`（`crate/io/fkv_io_compio/src/lib.rs:1963-1991`）两个失败
出口（:1986 重查臂、:1989 首查臂）均改经 `map_io_err`，仅 NotFound 耗尽才回
`ResourceMissing`。目标 2 与验收测试未做。

## 现象（亲读，2026-11-05 06:2x 版）

1. **目标 2 未落地**：`open_data_file`（:1994-2014）三个非 NotFound 出口仍是
   `Err(_) => Err(KvError::IoFail)`（:2009 重查臂、:2012 首查臂）。`EACCES →
   NoPermission`、`EMFILE/ENFILE → OpenFileLimit`、`EAGAIN/EINTR → TryAgain`
   的分类在 data 文件打开路径仍被摊平。实际影响：`OpenFileLimit` /
   `TryAgain` 是 `is_retryable()`（`fkv_error/src/lib.rs:58-59`）唯一认可的重试类，
   摊平成 `IoFail` 后 store/task 层的重试分类失真——与 r8e-03 对 segment 路径
   的立项理由同构，只是错误码从 `ResourceMissing` 换成了 `IoFail`。
   上游 cloud 层 `CloudStore::read_file`（`fkv_cloud/src/lib.rs:3025-3041`）虽然
   不会把 `IoFail` 误判为 cache miss（不像 segment 路径），但 fd 耗尽从可重试
   变成终局失败。
2. **验收用例缺失**：r8e-03 验收要求「把 segment 期望路径替换为同名目录
   （EISDIR），`read_segment` 返回 `IoFail`；data 侧同构断言 `NoPermission`/
   `IoFail`」。全仓 grep `OpenFileLimit|NoPermission`（io / cloud / bench，排除
   `_cpp`）：除 `map_io_err` 本体（src/lib.rs:723/:730）外零测试命中——
   contract.rs 三镜像仍只钉 NotFound → `ResourceMissing`
   （`contract.rs:57-74`），非 NotFound 打开错误的分类无任何测试钉住。

3. **同类折叠点清单**（同一修复批次顺带清扫的候选，非本条强制验收项）：
   - `remove_file_by_kind` / `remove_file_variant_by_kind`
     （src/lib.rs:2086/:2114）：`Err(KvError::IoFail)` 硬编码；
   - `sync_data` 的 `open_for_sync` 失败臂（:2499）；
   - `ensure_dir` 的 `create_dir_all` 失败（:2155-2159 `IoFail`）；
   - `drop_manifest`（:2575）、`remove_manifest_file`（:2633）、
     `read_manifest_path`（:2651）、`try_remove_empty_partition_dir`（:3122/:3131）。
   这些点不产生「资源缺失误判」，危害低于 open_data_file，可按同一原则
   （非 NotFound 一律 `map_io_err`）统一或登记豁免理由。

## 目标

1. `open_data_file` 两个失败出口改 `map_io_err(err)`（`NotFound → Ok(None)`
   语义不动），与 `open_segment_file` 对齐。
2. 补用例（建议 `hardening.rs` 或 contract 新段）：把 segment / data 期望路径
   替换为同名目录（macOS 需以 `EISDIR`/`EACCES` 可复现的方式构造，或直接对
   `open_read_file` 注入不可重试错误），断言 `read_segment` / `read_file` 返回
   `IoFail`（或更细分类）而非 `ResourceMissing` / 恒 `IoFail` 摊平；Linux CI
   上以 `chmod 000` 断言 `NoPermission`。
3. 第 3 节同类折叠点逐个裁定：改 `map_io_err` 或在代码注释登记「为何此处
   摊平无害」。

## 验收

- data 打开路径的 errno 分类经 `map_io_err` 保留；新用例在摊平回退时报错。
- contract 三镜像 / `hardening.rs` / cloud cache-miss 用例不回归；
  `./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- R9 独立审查（r9e，2026-11-05）亲读 `fkv_io_compio/src/lib.rs` 全文
  （:1994-2014、:719-735、:2082-2094、:2482-2526 等）、`fkv_error/src/lib.rs`
  :58-59、`fkv_cloud/src/lib.rs:3025-3041`；测试面 grep 全仓零命中。
- 查重：r8e-03 DONE 只声明 segment 侧；io-41 / io-39 均不涉及打开路径；
  r7b 旧发现（drop_manifest / prewarm / adopt_local_file）已闭环，与本条无交集。


## DONE（2026-11-05）

DONE：`open_data_file` 两个非 NotFound 出口改 `map_io_err`，与 segment 路径对齐。

## 复审（2026-11-05 v，lane B）

通过（目标 1 代码），残余拆 v-05。亲读 `crate/io/fkv_io_compio/src/lib.rs:2001-2021`：
首查臂 `Err(err) => Err(map_io_err(err))`（:2019），NotFound 臂重扫后
`:2015` NotFound → `Ok(None)`、`:2016` 其余 `map_io_err(err)`——目标 1 属实，
与 `open_segment_file` 对齐。**残余**：本 issue 目标 2/验收要求的非 NotFound
用例仍零覆盖（`ResourceBusy`/`OpenFileLimit`/`NoPermission`/`EISDIR` 全仓
测试面无映射断言）；目标 3 的同类折叠点（:2093/:2121、:2166、:2506、
:2582/:2605/:2642/:2682/:2700/:2728 的硬编码 `IoFail`）仍未裁定。两项一并
拆出 [`v-05`](../new/v-05-open-error-errno-classification-tests-missing.md)。
归档 `issue/reviewed/`。
