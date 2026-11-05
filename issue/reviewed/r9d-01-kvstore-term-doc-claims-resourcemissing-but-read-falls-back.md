# r9d-01 P3：KvStore::new term 批次三处「term 分叉 → 读成 ResourceMissing」机制声明与 sto-70 后的读路径相悖（活跃 manifest 固定名 + data/segment 变体回退，读不会挂）

## 现象（亲读，2026-11-05 06:xx 版）

本轮新落地的 `KvStore::new(root, option, term)` 批次（fkv_sdk/src/lib.rs 06:01、
fkv_cli/src/main.rs 06:27、fkv_cli/tests/cli.rs 06:30 mtime）在三处把「数据面与
控制面必须共用同一 term」的理由写成**读侧失败模式**：

1. `crate/facade/fkv_sdk/src/lib.rs:35-37`（`KvStore::new` doc）：

   ```rust
   /// `term` 是 store 的文件身份：数据 / manifest 文件名都带 `(branch, term)`，
   /// 必须与同一数据根上的控制面（`Runtime::attach_shard`）一致，否则同一份
   /// 数据会因文件名不匹配被读成 `ResourceMissing`。
   ```

2. `crate/facade/fkv_cli/src/main.rs:11-13`（模块 doc）：

   ```text
   //! Local 的数据面（SDK）与控制面（Runtime）共用同一 `--term`：盘面文件名带
   //! `(branch, term)`，两边 term 不一致时同一份数据会因身份不匹配读成
   //! `ResourceMissing`。
   ```

3. `crate/facade/fkv_cli/tests/cli.rs:58-62`（`cli_local_term_identity_survives_archive_and_branch` doc）：

   ```text
   /// 归档 / 建分支 / 重开归档后仍要读回原值：两边 term 分叉时数据文件名
   /// 不匹配，`get` 会退化成 `ResourceMissing`。
   ```

但 fkv 本地盘面的实际读路径两条腿都**不因 term 分叉失败**：

- **manifest 腿**：本地活跃 manifest 固定名 `manifest_main_0`，term 不进名字
  （`crate/io/fkv_io_compio/src/lib.rs:2268-2274` `manifest_path`，注释明言
  「term 只影响 data/segment 对象名」，sto-70 决策）。SDK term=0 与 Runtime
  term=3 读写的是同一个 manifest 文件，`Store::reopen` 无 term 门。
- **data/segment 腿**：读侧按候选回退——`collect_data_path_li`
  （fkv_io_compio/src/lib.rs:1170-1185）当前 identity 优先、其余 `(branch, term)`
  变体按 term 降序追加；`read_page`（:2301-2340）逐候选尝试，只有「全部候选
  都没有这一页」才报错。term 分叉时旧 term 家族文件以 stale 候选命中返回
  （index > 0 计 `stale_read_num`），不产生 `ResourceMissing`。
  该回退行为已被 io 层测试钉死：`fkv_io_compio/tests/naming.rs:222-233`
  （「当前 term 尚无该 file id 文件、目录里只有旧 term 文件时」读回 / 写不覆写）。
- 同理 sdk doc 第 1 句「数据 / **manifest** 文件名都带 `(branch, term)`」的
  manifest 半句不准：活跃 manifest 的 term 槽位恒为 0，store 的 term 只出现在
  data/segment 文件名。

即：三处注释描述的失败模式不存在；照此口径后续若有人给「term 分叉」补一个
期望 `ResourceMissing` 的负例测试，测试会**拿到数据而失败**，与 cli.rs 测试
注释自相矛盾。

**共用 `--term` 的真实理由依然成立**（修措辞不是改设计）：写路径只按当前
`(branch, term)` 命名（`write_path`，fkv_io_compio/src/lib.rs:1204-1220，
naming.rs:195-214 钉「不得覆写旧 term 文件」）；term 分叉会把同一逻辑盘面
分裂成两个 term 文件族，归档名构造（`archive_file_name(branch, term, tag)`）、
term 文件 CAS（attach fencing）与 GC 变体归属都以统一 term 为前提。SDK 不检查
term 文件、不做 fencing，两族文件并读虽不报错，但盘面归属与诊断口径会乱。

## C++ 对照

C++ 侧活跃 manifest 名带真实 term（`manifest_main_<term>`），term 分叉时
manifest 本身就找不到 —— 声明对 C++ 语义成立。fkv 本地盘面有意改为固定活跃名
`manifest_main_0` + 读侧变体回退（sto-70，混部行已登记
`doc/migration.md` §8.2），声明照搬 C++ 语义后与自身实现相悖。不涉盘面，
纯 Rust 侧 facade 文档滞后。

## 目标

三处措辞改为如实口径（设计不动）：

1. `fkv_sdk/src/lib.rs:35-37`：删「否则同一份数据会因文件名不匹配被读成
   `ResourceMissing`」；改为「term 分叉时读侧有 `(branch, term)` 变体回退
   （当前身份优先、其余按 term 降序，io-31 候选语义 + `stale_read_num`），
   读不报错，但写会分裂成两个 term 文件族，归档名 / term 文件 CAS / GC 归属
   都以统一 term 为前提」；「manifest 文件名都带 term」改为「data/segment
   文件名带 `(branch, term)`；本地活跃 manifest 固定名 `manifest_main_0`
   （sto-70）」。
2. `fkv_cli/src/main.rs:11-13` 与 `fkv_cli/tests/cli.rs:58-62`：同样把
   「读成 / 退化成 `ResourceMissing`」改为「读侧可经变体回退读到，但写身份
   分裂、归档与 fencing 语义要求两边共用同一 term」。
3. 不要求新增 term 分叉回退测试（`naming.rs` 已在 io 层钉死该行为）。

## 验收

- `grep -rn "ResourceMissing" crate/facade/fkv_sdk/src crate/facade/fkv_cli/src crate/facade/fkv_cli/tests`
  无「term 分叉 → 读失败」语义残留；
- 三处新措辞与 `doc/migration.md` §8.2 混部行、`fkv_io_compio::read_page`
  候选回退注释同源，无新失实声明；
- 纯注释改动：`./sh/clippy.sh` 通过，`fkv_cli` / `fkv_sdk` 测试不回归。

## 核验记录

- r9d 独立审查（R2 轮，2026-11-05）亲读 fkv_sdk/src/lib.rs 全文、fkv_cli/src/main.rs
  全文、fkv_cli/tests/cli.rs、fkv_io_compio/src/lib.rs 相关段
  （`manifest_path` :2268、`collect_data_path_li` :1170、`data_path_li` :1187、
  `read_page` :2301、`write_path` :1204、`scan_dir` :774）与
  `fkv_io_compio/tests/naming.rs` :195-240；反证检索：facade 三 crate 测试无
  「term 不一致」负例（cli.rs 的 identity 测试两侧同 term）。
- 查重：r8d-01（write_gate 并发口径）、r9a-01（fkv_config partition_filter
  口径）、rev-48（doc/ 写并发滞后）均为文档滞后同类不同点；sto-70 是读侧放宽
  的落地条目本身，未覆盖 facade 层新注释；`issue/new/` 现有 rev-02（ENOBUFS
  分类）/ rev-03（FixedBuffer drop hook）为 io 层不同主题。
- 锚点 mtime（2026-11-05 读取时点）：`fkv_sdk/src/lib.rs` 06:01、
  `fkv_cli/src/main.rs` 06:27、`fkv_cli/tests/cli.rs` 06:30（读取时距今约 5~10
  分钟，非 <2 分钟并行写入区；但本批次即「KvStore::new term 参数」并行批次
  的产物，行号以内容锚点为准）。fkv_shard/src/lib.rs、fkv_runtime/src/lib.rs、
  partition.rs、cli.rs 在 06:30:50 曾出现 <2 分钟并行写入，本条锚点不在其实现段。

> 认领（2026-11-05 06:50，本会话子代理开发中）。lane：fkv_sdk/fkv_cli 相关注释。

## DONE（2026-11-05）

- 甄别：亲读 `fkv_io_compio::manifest_path`（:2268，活跃 manifest 固定名
  `manifest_main_0`，注释明言 term 只影响 data/segment 对象名）、
  `collect_data_path_li`（:1170，当前 identity 优先、其余按 term 降序）、
  `read_page`（:2301，逐候选尝试，`ResourceMissing` 仅在候选列表为空时；
  index > 0 计 `stale_read_num`）、`write_path`（:1204，只按当前
  `(branch, term)` 命名）与 `tests/naming.rs:195-240`（io-04 两例钉死写不
  覆写 / 首写不落旧 term）；`doc/migration.md` §8.2 混部行确认「attach term
  与 master 写数据时不同也能读到」。issue 证据成立，失败模式不存在。
- 开发（仅注释，行为零改动）：
  - `fkv_sdk/src/lib.rs` `KvStore::new` doc：删「读成 `ResourceMissing`」；
    改为「data/segment 文件名带 `(branch, term)`，活跃 manifest 固定名
    `manifest_main_0`（sto-70）；term 分叉读侧变体回退（io-31 候选语义 +
    `stale_read_num`）读不报错，但写分裂成两个 term 文件族，归档名 / term
    文件 CAS / GC 归属以统一 term 为前提」。
  - `fkv_cli/src/main.rs` 模块 doc：同样改为「读侧可经变体回退读到，但写
    身份分裂、归档与 fencing 语义要求两边共用同一 term」。
  - 第三处 `fkv_cli/tests/cli.rs:58-62` 在本 lane 之外（lane 仅
    `fkv_sdk/src/lib.rs` 与 `fkv_cli/src/`），未触碰，留待 tests lane 同口径
    修正。
- 验收：`grep -rn 'ResourceMissing' crate/facade/fkv_sdk/src
  crate/facade/fkv_cli/src` 仅余两处合法代码 match（`ensure_open` 的
  manifest 缺失懒建空表、CLI `reopen_if_exists`），无「term 分叉 → 读失败」
  语义残留；`cargo check -p fkv_sdk -p fkv_cli` 通过（exit 0，无警告）。
  新措辞与 migration.md §8.2 混部行、`read_page` 候选回退注释同源。

### 协调者补记（2026-11-05）

第三处（`fkv_cli/tests/cli.rs:58-62`）已由协调者按同口径修正（读侧变体回退 +
写身份/归档 CAS/GC 归属依赖统一 term），验证：该测试 `cli_local_term_identity_survives_archive_and_branch`
行为零改动。本条全部三处收口。

## 复审（2026-11-05 v）

通过，归档。亲读三处措辞已如实（设计未动）：

- `crate/facade/fkv_sdk/src/lib.rs:35-44`：改为「data/segment 文件名带
  `(branch, term)`；本地活跃 manifest 固定名 `manifest_main_0`（sto-70）……
  term 分叉时读侧有 `(branch, term)` 变体回退……读不报错；但写只按当前
  `(branch, term)` 命名……归档名构造、term 文件 CAS、GC 变体归属以统一 term
  为前提」。
- `crate/facade/fkv_cli/src/main.rs:11-16`：模块 doc 同口径。
- `crate/facade/fkv_cli/tests/cli.rs:58-64`：`cli_local_term_identity_survives_
  archive_and_branch` doc 同口径（协调者补记的第三处）。
- `grep -rn ResourceMissing crate/facade/fkv_sdk/src crate/facade/fkv_cli/src
  crate/facade/fkv_cli/tests` 仅余 `fkv_sdk:126`（`ensure_open` 懒建空表）与
  `fkv_cli/src/main.rs:171`（`reopen_if_exists`）两处合法代码 match，无「term
  分叉 → 读失败」语义残留。
