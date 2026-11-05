# 01 P0：没有真正的 C++ oracle / 交叉读写验证

## 现象

- `crate/**/*_cpp` 共 44 个，全部只有一行占位注释，例如：
  - `crate/format/fkv_page_cpp/src/lib.rs`
  - `crate/engine/fkv_store_cpp/src/lib.rs`
- `cpp/eloqstore` 为空目录
- `cpp/shim/README.md` 明确写“真正的 shim 逐个补齐”
- `COMPAT_BASELINE` 已存在，但只被 `fkv_bench` 用来读取 baseline 元信息
- `oracle.yml` 只 build C++，没有跑 C++/Rust 差分

## C++ 对照

eloqstore 的 `tests/`、`db_stress`、`benchmark/simple_bench` 都是可执行的真实对照面。
我们的 `_cpp` 设计原本就是为 Crubit/差分准备的，但目前没有产生任何对比结果。

## 影响

当前“字节兼容”只能算代码推断：

- 没有 C++ 写 → Rust 读
- 没有 Rust 写 → C++ 读
- 没有同一 workload 的 page/manifest 字节对比
- 没有确认 branch/term、compression flag、overflow、manifest checksum 的实际布局

任何后续“对齐”改动都缺少回归判据。

## 建议

1. 先做最小 L1 fixture：
   - C++ 生成空表、单页、多页、restart、TTL、large value、archive、branch 的 page/manifest 文件
   - Rust 读取并断言 key/value/timestamp/expire_ts
2. 反向 fixture：
   - Rust 写，C++ replayer 读取并输出同样的摘要
3. 增加 `cargo xtask compat`：
   - 构建 C++ oracle
   - 生成/比对 fixtures
   - 失败时输出首个差异 offset
4. 在 CI 增加 oracle job，不能只 build。

## 验收

- CI 中存在双向交叉读写测试并通过
- `fixtures/` 有可重复生成脚本和版本号
- 至少覆盖 data/index/manifest/overflow/compression/branch 六类格式

---

## 进展（脚手架已实现）

- `fkv_ledger fixture`：生成 `<out>/<kind>/<case>/{meta.txt,payload.bin}`；无 oracle
  时写 `status=skipped`，有 oracle 时调用 `<oracle> fixture --kind --case --out`。
- `fkv_ledger verify`：遍历 fixture 目录逐 case 校验（有 oracle 调
  `<oracle> verify --case-dir`），输出 Markdown 报告，有 fail 时退出码非 0。
- 目录/清单格式与 oracle 接口约定见 `fixtures/README.md`；
  `doc/migration-tooling.md` §3.3 已更新。
- **未完成**：真正的 C++ oracle 可执行文件、真实字节 fixture、Rust 侧解析断言与
  双向交叉读（需要 Linux + eloqstore 构建）。

---

## 审查结论（2026-11-04）

### 总结论

**部分成立，保留 → `issue/todo/`。**

- 原始「现象」四条中，两条已被脚手架消化：
  - `fixture` / `verify` 流水线已实现且有测试；
  - `oracle.yml` 的事实描述仍准确（只 build，无差分）。
- 核心缺口不变：**没有 C++ oracle 可执行文件**，因此没有任何真实字节比对，
  `verified` 仍无兼容性证据。
- `_cpp` 空壳一条与 `fmt-16-cpp-stub-crates-empty.md` 重复，本条不再展开，避免双份任务。
- 建议 3「`cargo xtask compat`」的**功能**已由 `fkv_ledger fixture|verify` 承接，
  不必再建 xtask；只需在文档里写清二者等价，防止后续重复造轮子。

### 逐条核验表

| 原条目 | 现状 | 证据 |
|---|---|---|
| 44 个 `_cpp` 只有一行注释 | **成立** | `for f in crate/*/*_cpp/src/lib.rs` 全部 1 行；样本 `crate/format/fkv_page_cpp/src/lib.rs`、`crate/core/fkv_btree_cpp/src/lib.rs` |
| `cpp/eloqstore` 为空 | **成立** | `ls -la cpp/eloqstore` 只有 `.`/`..`；无 `.gitmodules`，未以 submodule 接入 |
| `cpp/shim` 只有 README | **成立** | `cpp/shim/` 仅 `README.md`，正文写明「逐个补齐」 |
| `COMPAT_BASELINE` 只被 `fkv_bench` 用 | **部分过期** | `fkv_ledger/src/fixture.rs` 已把 baseline 压成单行写入 `meta.txt`（`baseline_text`，有测试） |
| `oracle.yml` 只 build | **成立** | `.github/workflows/oracle.yml` 仅 cmake build + upload artifact；无 `fixture`/`verify` 调用，无差分 |
| 建议 1 最小 L1 fixture（空表/单页/多页/restart/TTL/large/archive/branch） | **未实现** | `fixture::KINDS = ["page","manifest","mapping","store"]`；`case` 为自由字符串，无内置 case 矩阵 |
| 建议 2 反向 fixture（Rust 写，C++ 读） | **未实现** | `fixtures/rust/` 为空目录；`verify` 只 shell out `<oracle> verify`，Rust 侧不解析 payload |
| 建议 3 `cargo xtask compat` | **以其他形态实现** | `tool/` 下只有 `fkv_ledger`，无 xtask；能力由 `fkv_ledger fixture|verify` 提供 |
| 建议 4 CI oracle job 不能只 build | **未实现** | `ci.yml` 无 fixture 步骤；`oracle.yml` 无差分步骤 |
| 验收：双向交叉读写 | **未达成** | 无 oracle 二进制、无真实 payload |
| 验收：可重复生成 + 版本号 | **部分** | 命令可重复；版本号仅 `baseline=` 字段，无 fixture schema 版本号 |
| 验收：六类格式覆盖 | **未达成** | kinds 只有 4 类，且无 overflow / compression / branch 的 case 语义 |

补充事实：

- `fixture` 在无 oracle 时写 0 字节 `payload.bin`（`payload_len = Some(0)`），
  与 README「skipped 时为空」一致。
- `verify` 对 `status=fail` 且无 oracle 的 case 计入 `skipped` 而非 `fail`，
  属可接受的最小实现，但会掩盖 oracle 曾生成失败的情况。
- `fkv_ledger/tests/fixture.rs` 覆盖 skipped / 未知 kind / 缺失目录 / baseline 压行，
  测试扎实，说明脚手架不是纸面设计。

### 剩余有效项（按依赖排序）

1. **C++ oracle 可执行文件**（阻塞项）
   - 目标：`cpp/oracle/` 新增 CMake target，实现 `fixture --kind --case --out`
     与 `verify --case-dir`，内部调用 eloqstore 真实 `DataPageBuilder` /
     `IndexPageBuilder` / `ManifestBuilder` / `CompressRaw`。
   - 阻塞：**必须 Linux + 完整 eloqstore 构建**（`docker/Dockerfile.oracle` 现有镜像已备依赖；
     macOS 本地无法交叉产出）。仓库当前无 eloqstore 源码，需按 `COMPAT_BASELINE`
     的 commit `82b774c` checkout 到 `cpp/eloqstore`（或 submodule）。
2. **case 矩阵落地**
   - 扩展 `KINDS` 与 `case` 语义：`page:{empty, single, multi, restart, ttl, large,
     overflow}`、`manifest:{empty, branch, checksum, archive}`、
     `compress:{raw, dict}`、`index:{leaf, branch}`。
   - 同步修 `fixtures/README.md` 的 kind 枚举与 `status=fail` 计数语义。
3. **Rust 侧解析断言**
   - `verify` 不能只信任 oracle 退出码；新增 Rust 直接解码 `payload.bin` 分支
     （`fkv_page::decode_*` / `fkv_manifest::*`），断言 key/value/timestamp/expire_ts。
   - 失败时输出**首个差异 offset**（对应原始建议 3 的后半句）。
4. **反向 fixture**
   - Rust 写 `fixtures/rust/<kind>/<case>/`，C++ `verify` 读回并输出摘要；
     与 1 共用同一 oracle 协议，无需新接口。
5. **CI 差分 job**
   - `oracle.yml` 增加：build → 生成 fixture → `fkv_ledger verify` → 上传报告；
     有 fail 时 job 失败。`ci.yml` 的 `compose` job 可在有 oracle 时复用。
6. **fixture schema 版本号**
   - `meta.txt` 增 `schema=1`；`verify` 校验，避免格式演进后旧 fixture 静默通过。

### 开发计划与阻塞

| 阶段 | 内容 | 前置 | 验收 |
|---|---|---|---|
| P0-a | 接入 eloqstore（submodule 或 `cpp/eloqstore` checkout `82b774c`）；`cpp/oracle` CMake target | **Linux + 完整 eloqstore 构建**，macOS 本地不可完成 | `docker compose run --rm oracle` 能跑出 `fkv_oracle fixture --kind page --case smoke --out /tmp/x` 并产出非空 `payload.bin` |
| P0-b | 扩展 kind/case 矩阵 + `schema=1` | P0-a 可先并行写清单 | `fkv_ledger fixture` 生成全矩阵目录树，无 oracle 时全 skipped |
| P0-c | Rust 解析断言 + 首差异 offset | P0-a、P0-b | 篡改 payload 一个字节，`verify` 退出码非 0 且打印 offset |
| P1-a | 反向 fixture | P0-a | C++ verify 读 Rust payload 摘要一致 |
| P1-b | CI 差分 job | P0-c | oracle workflow 有 fixture+verify 步骤，artifact 含 Markdown 报告 |

**明确阻塞**：P0-a / P0-c / P1-a / P1-b 均依赖 Linux 上的 eloqstore 构建；
本地 macOS 只能完成 P0-b（清单矩阵）与文档同步。
`cpp/eloqstore` 为空是本条无法推进的根因之一，需先决定 submodule 还是 CI 侧 checkout
（`oracle.yml` 已在 CI 侧 checkout，但本地开发缺同一路径，建议补 submodule 或
`scripts/fetch_eloqstore.sh` 并写进 README）。

### 不采纳的子项

- **`cargo xtask compat`**：与 `fkv_ledger fixture|verify` 功能重叠，
  仅在 `fixtures/README.md` 标注「xtask 形态不采纳，用 ledger 子命令」即可。
- **`_cpp` 空壳**：归 `fmt-16`，本条不重复跟踪。

## 进展（2026-11-04，第三轮）

已落地（本地可完成部分）：

- `fkv_ledger::fixture` 新增 `SCHEMA_VERSION = 1` 与 `MATRIX`（6 类 21 个规范 case：
  `page:{empty,single,multi,restart,ttl,large,overflow}`、`index:{leaf,branch}`、
  `manifest:{empty,branch,checksum,archive}`、`compress:{raw,dict}`、
  `mapping:{empty,single,multi}`、`store:{empty,single,multi}`）；
  `KINDS` 改为由矩阵派生（`kind_li()` / `cases_of()`），`kind` 校验走矩阵。
- `meta.txt` 增加 `schema=1`；`fixture --matrix` 全量生成矩阵。
- `verify` 修两处计数缺陷：`schema` 缺失/不符一律判 `fail`（旧 fixture 不再静默通过）；
  `status=fail`（oracle 生成阶段即失败）始终计 `fail`，不因本次未传 oracle 而降级为
  `skipped`。
- `fixtures/README.md` 同步：kind 枚举、case 矩阵、`schema` 字段、verify 计数语义、
  「待补（需 Linux）」清单。

验证：`cargo test -p fkv_ledger` 11 passed（fixture 8 + ledger 3）；
`cargo clippy -p fkv_ledger --all-targets --no-deps -- -D warnings` 零警告；
`cargo run -p fkv_ledger -- fixture --matrix` 生成 21 个 case，`verify` 报 21 skipped。

## 阻塞（仍需 Linux + 完整 eloqstore 构建）

- `cpp/eloqstore` 仍是空目录，`cpp/oracle` 未实现 → 无真实 payload；
- Rust 侧解码断言与首个差异 offset 输出依赖真实 payload，待 oracle 就绪后补；
- CI 差分 job（`oracle.yml` 增加 fixture+verify 步骤）同上。

## 进展（2026-11-04，oracle 脚手架落地）

本轮把「可复现的 C++ oracle」这一环补上（此前只有不可复现的 golden 十六进制）：

1. **`cpp/oracle/src/main.cpp`**（新增）：用 eloqstore 真实 `DataPageBuilder` +
   `SetChecksum` 产出 data page 字节，实现契约里的 `fixture --kind page --case <C> --out DIR`
   与 `verify --case-dir DIR`；case：`empty/single/multi/restart/ttl/large_values`。
   未实现的 kind **明确报 unsupported 并以非 0 退出**，不静默跳过。
2. **`cpp/oracle/build.sh`**（新增）：只编格式层 TU
   （`data_page_builder/data_page/page/coding/compression/comparator`），
   **不需要 io_uring / S3 / eloqstore 整库构建**，因此 CI 里差分作业很便宜。
3. **`scripts/fetch_eloqstore.sh`**（新增）：按 `COMPAT_BASELINE` 的 commit 取
   eloqstore 到 `cpp/eloqstore`（幂等）。
4. **CI**：`.github/workflows/oracle.yml` 新增 `fixture-diff` 作业 =
   构建 oracle → `fixture` 生成 → oracle `verify` 自检 → Rust 侧解码固定语料 → 上传产物。
5. **容器**：`docker/Dockerfile.oracle` 基础镜像改成 `ARG ORACLE_BASE`（默认 `ubuntu:26.04`，
   CI 不变），本地可用 `ORACLE_BASE=debian:bookworm-slim` 绕开慢镜像；
   `docker-compose.yml` 的 oracle 服务把 fkv 仓挂到 `/src/fkv` 以便在容器里编 oracle。
6. **Rust 侧**：`crate/format/fkv_page/tests/cpp_oracle.rs` 逐 case 解码
   `fixtures/cpp/page/<case>/payload.bin` 并断言 key/value/ts/expire（不需要 C++ 工具链）。

验证状态：

- `cpp/oracle/src/main.cpp` 对 eloqstore 头文件 **`-fsyntax-only` 通过**（API 层正确）；
- `cargo nextest run -p fkv_page cpp_oracle` 通过（语料未生成时打印 skip 提示）；
- 已存在的 `format_semantics::data_page_bytes_match_cpp_oracle` /
  `index_page_bytes_match_cpp_oracle`（golden 十六进制 + 注释写明 oracle 过程）继续通过，
  这是当前 data/index 页字节兼容的直接证据。

### 仍未完成（下一步）

- **语料生成**：本机（macOS）编不了（`page.cpp` 依赖 liburing/jsoncpp），
  Linux 侧容器镜像拉取在本机网络下超时，故 `fixtures/cpp/page/*` 暂缺；
  CI 的 `fixture-diff` 作业或任一有 Docker 的 Linux 机器跑
  `ELOQSTORE_SRC=/src/eloqstore bash cpp/oracle/build.sh <out>` + `fixture` 即可补上。
- 其他 kind（index/manifest/compress/mapping/store）的 oracle 语料；index 页可照
  `index_page_builder.cpp` 接（golden 已有）。
- 反向 fixture（Rust 写 → C++ `verify` 读）与 `verify` 的首个差异 offset 输出。

> 认领（2026-11-05 01:15）：本条由本会话子代理开发中（oracle 构建 + 真实 fixture + Rust 侧断言）。lane：cpp/ fixtures/ scripts/ tool/fkv_ledger。

## 进展（2026-11-05，第四轮：真实语料 + 双向单边闭环）

### 已落地

1. **`cpp/oracle` 构建修复与扩展**
   - `build.sh`：`-I "$SRC"` 改为 `-idirafter`，修掉 eloqstore 根目录 `version`
     文件遮蔽 C++ 标准头 `<version>`；补 `-I "$SRC/external/abseil"`（Linux 无系统
     absl）；链接 `-lzstd -lglog -lgflags -pthread`，`-ffunction-sections` +
     `--gc-sections` / `-dead_strip` 只保留可达格式层；默认源码目录修正为
     `cpp/eloqstore`。
   - macOS 本地构建：自动为 `<jsoncpp/json/json.h>` 与缺失的 `liburing.h` 生成
     临时 include shim（oracle 不调用任何 io_uring 符号），不再需要 Linux 才能产语料。
   - 新增 `standalone.{h,cpp}`：`Page(true)` / `Option()` / `ThdTask()` 的最小
     运行期 shim，**不参与字节编码**；页/index/manifest 字节仍由 eloqstore 真实
     builder / parser 产生。
   - 新增 kind/case：`index:{leaf,branch}`（`IndexPageBuilder`）、
     `compress:{raw,dict}`（`CompressRaw` / `DictCompression`，dict 额外写
     `dict.bin`）、`manifest:{empty,checksum,branch,archive}`（`ManifestBuilder`）；
     `page` 补 `overflow` 与 `large`（保留 `large_values` 别名）。
   - `verify` 增加 `--kind/--case`；manifest checksum 按 `header + payload_len`
     校验（对齐 padding 不参与，与真实读路径一致）。
   - manifest 语料裁掉 OS page-size padding（macOS 16KiB / Linux 4KiB），
     保证跨平台字节可复现。

2. **真实 fixture 入库**：`fixtures/cpp/` 共 4 类 15 case
   （page 7 / index 2 / compress 2 / manifest 4），含 `meta.txt`、`dict.bin`。
   Linux（Ubuntu 24.04 容器）与 macOS 各自生成的 15 个 payload sha256 **全部一致**。

3. **Rust 侧解析断言**
   - `crate/format/fkv_page/tests/cpp_oracle.rs`：data page（含 ttl / restart /
     large / overflow）与 index leaf/branch；
   - `crate/format/fkv_compress/tests/cpp_oracle.rs`：raw / dict 解压；
   - `crate/format/fkv_manifest/tests/cpp_oracle.rs`：checksum、header、branch metadata。

4. **`fkv_ledger`**
   - `verify` 向 oracle 传 `--kind/--case`；
   - `mapping` / `store` 是占位 kind，带 `--oracle` 生成时保持 `skipped`
     （`oracle=-`），不伪造 `fail`；`verify` skipped 文案区分「无实现」；
   - 生成前删除旧 `payload.bin`，避免上一轮结果冒充本轮；
   - 新增 `oracle_supported` / `ORACLE_KINDS` 与假 oracle 集成测试。

5. **CI / 容器**
   - `oracle.yml` 的 `fixture-diff`：生成全矩阵（4 类 15 case）→ Linux oracle
     自检 → 用本次生成字节覆盖 `fixtures/cpp` → Rust 解码同一批字节 → 上传产物。
   - `Dockerfile.oracle` 补 `libgoogle-glog-dev` / `libgflags-dev` / `libcurl4-openssl-dev`
     （oracle 链接与 eloqstore 头链需要）。

### 验证

- `ELOQSTORE_SRC=/tmp/eloqstore_probe bash cpp/oracle/build.sh`：macOS 成功；
  Ubuntu 24.04 容器成功（bookworm 的 g++ 12 无 `<format>`，不能用于实际构建）。
- 15/15 case `fixture` + `verify` 自检通过。
- `cargo run -p fkv_ledger -- verify --fixtures fixtures/cpp --oracle ...`
  → pass 15 / fail 0 / skipped 0。
- `cargo test`：`fkv_ledger` 18 passed；`fkv_page` / `fkv_compress` /
  `fkv_manifest` 的 `cpp_oracle` 各 2 passed。
- `cargo clippy -p {fkv_ledger,fkv_page,fkv_compress,fkv_manifest} --all-targets
  --no-deps -- -D warnings`：零警告。

### 仍未做（另立后续项）

- 反向 fixture（Rust 写 → C++ `verify` 读）。
- `verify` 的首个差异 offset 输出（当前失败报 case / entry 序号）。
- `mapping` / `store` 的 oracle 字节实现。
- `COMPAT_BASELINE` 的证据行仍写「fixtures/cpp 无 payload；C++ oracle 未实现」，
  与本轮事实不符；该文件不属本条 lane，未改。

## DONE（2026-11-05）

P0 剩余项中可在本机闭环的部分已完成：C++ oracle 扩展到 page/index/compress/
manifest 四类共 15 个真实 case，语料入库且 Linux/macOS 字节一致；Rust 侧解码
断言全部通过；`fkv_ledger` 清单/校验语义同步；CI 与容器命令可执行。
反向 fixture、首差异 offset、mapping/store 字节实现未做，已在 `fixtures/README.md`
「待补」与上节列明。


## 复审（2026-11-05 r6）

通过（DONE 声称的「真实语料 + Rust 侧解码断言 + 清单/校验语义」全部核实）。

- `cpp/oracle/src/main.cpp`、`standalone.{h,cpp}`、`cpp/oracle/build.sh` 存在；
  `scripts/fetch_eloqstore.sh` 存在。
- `fixtures/cpp/` 4 类（page 7 case / index / compress / manifest）共 15 个
  `payload.bin` 全部非空（find 计数 15/15）。
- `tool/fkv_ledger/src/fixture.rs:22 SCHEMA_VERSION=1`、`:28 MATRIX`（后续又演进
  为 7 类，含 overflow）、`:49 ORACLE_KINDS`（5 个有字节实现的 kind）、
  `:52 oracle_supported`（mapping/store 占位返回 false 不伪造 fail）；
  `tests/fixture.rs:136 oracle_supported_matches_implemented_kinds` 等测试在。
- Rust 侧解码断言：`fkv_page/tests/cpp_oracle.rs`、`fkv_compress/tests/cpp_oracle.rs`、
  `fkv_manifest/tests/cpp_oracle.rs` 均存在。
- `.github/workflows/oracle.yml:57-58` `fixture-diff` 作业存在。
- 未完成项（反向 fixture、首差异 offset、mapping/store 字节实现）已在 md 列明
  且未声称完成，如实。

## DONE 补全（2026-11-05，第二批：反向 + 首差异 offset + overflow 格式）

复审 r6 列为未完成的三项中两项已落地（oracle 构建与其他会话并行推进，
本批在其成果之上补齐）：

1. **overflow 页格式（六类收口）**：新增 kind `overflow`（case `chain`）——
   data entry（overflow 指针数组）+ 2 个连号 overflow 页（真实 `OverflowPage`
   写构造 + 独立页池 shim），C++ 生成 / 校验与 Rust 解码双侧对齐。
   至此验收要求的 data / index / manifest / overflow / compression / branch
   六类格式全部有真实字节语料（5 类 16 case）。
2. **`verify` 的 Rust 解码断言 + 首个差异 offset**：`fkv_ledger` 新增
   `src/corpus.rs`（语料与 `cpp/oracle/src/main.cpp` 一一对应），
   `verify` 对 `status=pass` 的 case 先跑语义逐条断言 + 与 Rust 重建字节
   逐字节比对，失败在报告里给出 `Rust 解码 offset N`（实测篡改 1 字节 →
   `offset 300` 且退出码非 0）。
3. **反向 fixture 打通**：`fkv_ledger fixture --rust --matrix` 用 Rust
   builder 写出同一语料到 `fixtures/rust/`（Rust 写 → C++ 读），C++ oracle
   `verify` 逐 case 读回 **16/16 PASS**；`verify` 一条命令同时跑
   Rust 解码 + C++ 交叉读（`fixtures/cpp` 与 `fixtures/rust` 双目录）。
4. **CI**：`oracle.yml` `fixture-diff` 增加 overflow 生成 / 自检循环、
   `fkv_ledger verify`（双向）与反向 fixture 两个步骤，报告随 artifact 上传。
5. **可重复生成脚本**：`scripts/gen_fixtures.sh`（构建 oracle → C++ 写
   `fixtures/cpp` → Rust 写 `fixtures/rust` → 双向 verify），重跑
   `fixtures/cpp` 与已提交语料 **0 字节差异**（可复现）；版本记录 =
   meta.txt `baseline` 字段 + 仓库根 `COMPAT_BASELINE`。

测试证据：`cargo nextest run -p fkv_ledger` 22/22（新增 corpus 5 项）；
`cargo nextest run -p fkv_page -p fkv_compress -p fkv_manifest cpp_oracle` 8/8；
`cargo clippy -p fkv_ledger --all-features --all-targets -- -D warnings` 零警告；
`scripts/gen_fixtures.sh` 双目录 pass 16+16 / fail 0。

仍未完成：`mapping` / `store` 的 oracle 字节实现（需 PageManager 级引擎
上下文，超出格式层 oracle 范围，已在 `fixtures/README.md` 待补注明）；
`COMPAT_BASELINE` 的证据行待 `fkv_ledger sync` 时随基线块更新（不在本条 lane）。

## 复审（2026-11-05 r6b）

通过。第二批 DONE 的三项声称全部核实：

- overflow kind 真实存在：`cpp/oracle/src/main.cpp:167,286,461-472`（真实 `OverflowPage` 写构造）、`:608`、`:707`（verify 读回）。
- Rust 解码断言 + 首差异 offset：`tool/fkv_ledger/src/corpus.rs:46-56`（`CaseDiff { offset }`）、`:710 decode_case`、`:742 build_payload`、`:781 generate_reverse`；`main.rs:179` `--rust` 旗标、`:339-360` 反向生成。
- 反向 fixture：`fixtures/rust/` 16 个 `payload.bin` 实测在盘；`fixtures/cpp/` 22 个 = 16 个 oracle 字节 case + 6 个 mapping/store 占位（`payload.bin` 0 字节、`meta.txt status=skipped`，与「不伪造 fail」一致）。
- CI 双向：`.github/workflows/oracle.yml:117`（oracle 自检）、`:140`（fkv_ledger verify）、`:146`（反向 fixture）；`scripts/gen_fixtures.sh` 存在。
- 测试在盘：`tool/fkv_ledger/tests/corpus.rs`（8 fn）、`tests/fixture.rs`（12 fn）。
- 未完成项（mapping/store 字节）已在 `fixtures/README.md:137`「待补」如实登记，未声称完成。
- 遗留：`COMPAT_BASELINE:10` 证据行仍写「fixtures 无 payload；C++ oracle 未实现」，与本轮事实相反且被压进全部语料 `meta.txt` 的 `baseline` 字段，原条目两次声明不在本 lane、无归口 → 另立 `issue/new/eng-91`。

## 修复记录（2026-11-05，manifest snapshot 语料与 branch segment 段）

独立复审发现两处 P0，已在本批修复（未 commit）：

1. **manifest `branch` / `archive` 语料不是真实 snapshot**：旧 `BuildManifest`
   对全部 case 走 `UpdateMapping` + `AppendBranchManifestMetadata` + `Finalize`，
   产出的是 log record（`mapping_len | mapping | branch_len | branch`），与
   `ManifestBuilder::Snapshot` 布局（`max_fp_id | dict | mapping | branch |
   [segment]`）不同；旧注释「archive 只是快照文件名带 tag，record 字节与
   snapshot 同构」错误。
   - `cpp/oracle/src/main.cpp`：`branch` / `archive` 改用
     `ManifestBuilder::Snapshot`。`branch` 走 `CreateBranch` 语义（不传 segment
     mapping，无 segment 段）；`archive` 走 `CreateArchive` 语义（带
     `max_segment_fp_id` + segment mapping，并覆盖非空 dict）。`verify` 按
     snapshot 布局逐段校验（max_fp_id / dict / mapping 值 / branch metadata /
     可选 segment 段是否存在及内容）。
   - `cpp/oracle/build.sh`：`Snapshot` 需构造 `MappingSnapshot`，其 `MappingTbl`
     依赖 absl；改用 eloqstore pin 的 `external/abseil` 构建静态库（系统 absl
     的 inline namespace/ABI 与 pin 版本不一致，会链接失败），并在
     `standalone.cpp` 补 `PageManager::FreeMappingSnapshot` 空实现（不参与字节
     编码）。macOS 本地与 Linux 容器共用同一脚本。
   - `fixtures/cpp/manifest/{branch,archive}/payload.bin` 与 `fixtures/rust/`
     反向语料重新生成；`crate/format/fkv_manifest/tests/cpp_oracle.rs` 增加
     snapshot 段解码断言（branch 无 segment、archive 有 segment + dict）。

2. **Rust branch/archive snapshot 多写可选 segment 段**：`create_branch` 无条件
   `get_or_insert` 一个空 segment mapper 并传 `Some(branch_segment_fp_id)`，
   使 plain store 的 branch snapshot 带 5B 空 segment 段；C++
   `CreateBranch -> Snapshot(...)` 的 `segment_mapping` 默认 nullptr，整段省略
   （`root_meta.cpp:141-153` 的 `if (segment_mapping != nullptr)`）。
   - `fkv_store/src/lib.rs`：`snapshot_record` 只在该表存在活跃 segment mapping
     （`!tbl.is_empty() || max_fp_id() > 0`，等价 C++ `GetSegmentMapper` 的
     nullptr 判定）时写该段；`create_branch` 仅在启用 large value / 已有 segment
     mapper 时创建并预留下 segment 区间，plain store 不创建 mapper、
     `max_segment_file_id` 记 0。
   - 新增测试：`fkv_store/tests/branch_archive.rs`
     `branch_and_archive_without_segment_mapping_omit_segment_section`、
     `branch_snapshot_with_large_value_keeps_segment_mapping`（后者验证分支重开
     后仍能读回父分支大 value）。
   - 反证 / 有意差异：C++ `CreateBranch` 即使父表有 segment mapper 也不写该段，
     branch 重开后 large value 无法解析。Rust 保留该段以维持 large value 继承，
     已另立 `issue/new/02-cpp-create-branch-omits-segment-mapping.md` 记录。

3. **CI / 证据行**：`oracle.yml` 改为 `fkv_ledger fixture --matrix --oracle`
   直写 `fixtures/cpp`（保留 `meta.txt`），checkout 开 LFS，生成后
   `git diff --exit-code fixtures/cpp` 检查漂移；不再先写 `fixtures/generated`
   再 cp 覆盖。`COMPAT_BASELINE` 证据行与 38 份 `meta.txt` 的 `baseline=` 同步
   刷新（见 eng-91）。

验证：`cargo test -p fkv_manifest` 11 passed（manifest 7 + cpp_oracle 4）；
`cargo test -p fkv_store --test
branch_archive --test branch_isolation --test large_value --test segment_compact
--test branch_store` 全绿（5/1/12/2/6）；`cargo test -p fkv_ledger` 22 passed；
`cargo clippy -p {fkv_store,fkv_manifest,fkv_ledger} --all-targets --no-deps --
-D warnings` 零警告；新 oracle 构建后 `fixture --matrix` + `verify` 双向
16 pass / 0 fail / 6 skipped；重复生成 `diff -r` 0 行。未验证：Linux 容器内
oracle 构建与 CI drift 作业（本机 macOS 已完成同脚本构建与 verify）。
