# fmt-16 _cpp 对照 crate 是空壳，V2 格式差分验证不可执行

## 位置
- Rust: `crate/format/fkv_page_cpp/src/lib.rs`（仅一行 doc comment，无任何函数）、`crate/format/fkv_manifest_cpp/src/lib.rs`、`crate/format/fkv_compress_cpp/src/lib.rs`（同为空壳）；`crate/foundation/fkv_codec_cpp`、`fkv_types_cpp` 同
- C++: `eloqstore/src/storage/data_page_builder.cpp`、`eloqstore/src/storage/index_page_builder.cpp`、`eloqstore/src/storage/data_page.cpp`、`eloqstore/src/compression.cpp`、`eloqstore/src/storage/root_meta.cpp`（真实实现）
- 契约: `doc/migration.md` §3 将 `fkv_page`/`fkv_compress`/`fkv_manifest`/`fkv_codec`/`fkv_types` 列为"真 C++ 对照（Crubit）"档，§4 V2"格式差分：同输入，Rust 与 C++ 编解码结果一致"是验收阶梯

## 现状
格式层五个 crate 的 `_cpp` 对照包只有 doc comment，没有暴露任何与 Rust 同形的函数。`Cargo.toml` 里 Crubit 绑定与 `cpp/shim` 依赖均注释掉（"迁移时用 cargo add --path ... 添加"）。

## C++ 实现
C++ 侧 page/manifest/compress/codec 的编解码函数齐全（`PutVarint32`、`DataPageBuilder::Add/Finish`、`IndexPageBuilder`、`ManifestBuilder::Snapshot/Finalize/ValidateChecksum`、`CompressRaw/DecompressRaw`、`DictCompression::BuildDictionary` 等）。

## 差异与风险
- V2 格式差分（同输入逐字节对比 Rust 与 C++ 编解码）目前**无法执行**：没有对照函数可调。验收阶梯 V2 缺证据。
- `_cpp` crate 存在但为空 → `cargo check` 通过、差分测试名存实亡；若有人按 `doc/migration.md` 认为 V2 已覆盖，会漏掉真实分叉（本目录 fmt-01/02/06/08 等正是 V2 该抓的类型）。
- Crubit 只能绑 `cpp/shim` 简单签名；page builder 的 `std::string_view` 入参/出参可绑，但 `DictCompression` 的 zstd 上下文对象需要 shim 收敛，工作量未落地。

## 对齐建议
- 先补 `cpp/shim` 的 C 入口（`data_page_build/add/finish`、`index_page_build`、`manifest_snapshot/finalize/validate`、`compress_raw/decompress_raw`、`varint_encode/decode` 等纯字节进出签名），再让 `_cpp` crate 经 Crubit/FFI 调用，跑 V2 差分。
- 短期替代：用 `fixtures/cpp` 生成的 C++ page/manifest 字节做 Rust 单向解析测试（V4 读侧），并在 issue 里把 V2 标 blocked。

## 性能优化建议
无。

## 严重度
P1（行为不一致/功能缺失）

---

## 审查结论（2026-11-04）

### 总结论

**部分采纳。** 核心事实成立：格式层五个 `_cpp` crate 确为空壳，V2 格式差分无任何可执行入口。

但本 issue 的两处依据已过期，且与 `01-cpp-oracle-and-fixtures.md`（P0）范围重叠。处理方式：不单独立项，作为 **01 的格式层（V2）细化子任务** 移入 `issue/todo/`；开发顺序服从 01。

### 逐条核验

| # | 原文断言 | 核验 | 证据 |
|---|---|---|---|
| 1 | 五个格式层 `_cpp` crate 只有 doc comment | 真 | `crate/format/{fkv_page,fkv_manifest,fkv_compress}_cpp/src/lib.rs`、`crate/foundation/{fkv_codec,fkv_types}_cpp/src/lib.rs` 各仅一行 `//!` |
| 2 | `Cargo.toml` 里 Crubit / `cpp/shim` 依赖均注释 | 真 | 每个 `*_cpp/Cargo.toml` 第 13 行 `# Crubit 绑定与 cpp/shim 在此接入`，`[dependencies]` 为空 |
| 3 | `cpp/shim` 无实现 | 真 | `cpp/shim/` 仅 `README.md`，无 `.cpp/.h/CMakeLists.txt` |
| 4 | `cpp/eloqstore` 空 | 真 | 目录 0 文件；`cpp/build` 亦空 |
| 5 | C++ 侧编解码函数齐全 | **无法在仓库内核实** | 源码不在仓库，靠 `docker-compose` 的 `EKV_ELOQSTORE_PATH` 外挂 / CI checkout baseline `82b774c`；文中符号名应标注来源 commit |
| 6 | `doc/migration.md` §3 把五个 crate 列为“真 C++ 对照（Crubit）”档 | **不实（引用过期）** | 当前 §3 是“迁移流程”，全文无该档位表；该分级实际在 `todo.md` D5：真 C++ 对照限格式/算法层约 11 个 |
| 7 | §4 V2 格式差分是验收阶梯 | 编号过期、内容真 | 验证阶梯现为 §5；V2 定义与 fmt-06 checksum 约定（C++ `Finish` 不写 checksum，Rust 用 `finish_without_checksum()`）已写入 |
| 8 | V2 差分当前不可执行 | 真 | 无 harness；`fixtures/cpp`、`fixtures/rust` 为空；`fkv_ledger fixture` 无 oracle 时写 `status=skipped`；`oracle.yml` 只 build 并上传产物 |
| 9 | “按 migration.md 会误以为 V2 已覆盖” | 不成立 | §2 已明写“C++ oracle 仅有占位；`cpp/eloqstore` 为空，44 个 `_cpp` crate 未实现”，§5 明写“V2–V6 还没有系统化执行”；文档无夸大 |
| 10 | 与 01 的关系 | 实质重复 | `01` 已覆盖 44 个空壳、空 `cpp/eloqstore`、无交叉读写/差分，并已给出 `fixture`/`verify` 脚手架；本文独有的只是格式层 shim 入口清单 |
| 11 | 把 `fkv_types` 纳入 V2 字节差分 | 不适用 | `fkv_types` 是类型别名/POD/常量（`u32/u64`、`StoreMode`、文件名常量），无编解码；应做布局/常量对齐断言，不需要 FFI |
| 12 | 先上 Crubit 再跑差分 | 过度设计 | D5 已定“Crubit 只覆盖 `cpp/shim`”；本层签名全可收敛为 `ptr+len`，直接 C ABI 成本更低，Crubit 的 nightly/`cc_std` 复杂度无收益 |

### 采纳的开发计划（挂在 01 下）

阶段 0（阻塞，不满足则本条保持 blocked）

- 01 的 oracle 可执行落地：Linux + `docker compose`，`eloqstore` 源码按 `COMPAT_BASELINE 82b774c` 挂载。
- `cmake` 能产出 oracle，且 `fixtures/` 生成链路可跑。

阶段 1：`cpp/shim` C ABI（纯字节进出）

- `varint32/64 encode/decode`、`data_page_build/add/finish`、`index_page_build/add/finish`、`manifest_snapshot/finalize/validate`、`compress_raw/decompress_raw`。
- 入参 `const uint8_t* + len`，出参 caller 提供 buffer + 返回长度（失败返回负码），禁止跨边界 `std::string`/`std::function`。
- page/manifest 入口一律走“无 checksum”变体，对齐 fmt-06。

阶段 2：五个 `_cpp` crate 暴露与 Rust 同形函数

- 内部 `extern "C"` 调 shim；`unsafe` 集中在单个 `-sys` 风格模块，外层给安全包装。
- 仅差分用途，不进生产依赖图；`fkv_types_cpp` 改为布局/常量断言（`size_of`、`align_of`、常量值），不做字节流差分。

阶段 3：差分 harness

- `cargo test -p fkv_page_cpp -p fkv_compress_cpp -p fkv_manifest_cpp -p fkv_codec_cpp`，固定输入表（空页、单条、多条、restart、乱序、TTL、溢出指针、字典开关）。
- 首异 offset 直接打印；结果写入 ledger `evidence kind=diff`，再把相关 unit 从 `done` 升 `verified`。

阶段 4：范围收敛

- 先做 `fkv_codec` + `fkv_page` + `fkv_compress` 三类；`fkv_manifest` 次之（依赖 snapshot 完整构造与 CRC）；`fkv_types` 只做静态断言。
- 字典压缩差分必须固定 zstd 版本与参数，否则只比 `decompress(compress(x))` 语义，不比字节。

与 01 的关系

- 01 是唯一总入口，负责 oracle 二进制、fixture 流水线、双向交叉读写（V4）。
- 本文只负责 V2 的 builder 输出逐字节差分，是 01 的格式层子集；01 未完成前不启动阶段 1。
- 不新增并行工作流，不重复维护 oracle 构建。

阻塞条件

1. `eloqstore` 源码可得（外挂路径或 CI checkout）。
2. Linux 环境（`liburing`/`zstd`/`boost`），macOS 本地只能跑 Rust 侧单测。
3. 01 的 oracle 可执行与 fixture 目录约定冻结。
4. 编译依赖 `rustc-dev`（已在 `rust-toolchain.toml`），但 C ABI 方案不强制 Crubit。

### 对原文的修正

- 删除/改写“`doc/migration.md` §3 / §4”引用，改为 `todo.md` D5 与 `doc/migration.md` §2、§5。
- “C++ 实现齐全”补 `COMPAT_BASELINE` commit 号与文件路径来源。
- `fkv_types` 移出字节差分清单，改为布局断言。

## 驳回重复核验（2026-11-04）

对既有审查结论逐条复核，仍成立，无需改写：

- 五个 crate 仍只有 `//!` 一行（`fkv_page_cpp` / `fkv_manifest_cpp` / `fkv_compress_cpp` / `fkv_codec_cpp` / `fkv_types_cpp`）。
- `cpp/` 下只有 `cpp/shim/README.md`；`cpp/eloqstore`、`cpp/build` 为空目录。
- `fixtures/cpp`、`fixtures/rust` 为空目录。

## 阻塞（2026-11-04）

**不可在本 lane 本地完成，按既有结论折叠进 `issue/todo/01-cpp-oracle-and-fixtures.md`，不启动阶段 1。**

阻塞点（实测）：

1. `eloqstore` 源码虽可在本地读到（`/Users/z/git/db/kv/eloqstore`，`COMPAT_BASELINE 82b774c`），但**不是可构建的 checkout**：
   - 3 个 git submodule 全部未初始化（`external/abseil`、`external/concurrentqueue`、`external/inih`，`git submodule status` 全为 `-`）。`inih` 缺失直接挡死 `kv_options.cpp`（`INIReader`）。
   - 语法探测 `clang++ -std=c++20 -fsyntax-only -Iinclude -Iexternal src/storage/data_page_builder.cpp` 立即失败：`zstd.h` 找不到（另需 glog / boost；`include/async_io_manager.h` 硬依赖 `<liburing.h>`，Linux 专属）。
2. 本机为 macOS：`liburing` 不可用，因此即使补齐 submodule 也编不出完整 oracle；只能跑 Rust 侧单测。
3. `01` 尚未冻结 oracle 可执行与 fixture 目录约定，`_cpp` 的 FFI 侧（阶段 2/3）没有对照产物的落地位置。
4. 与 `01` 范围重叠：`01` 负责 oracle 二进制、fixture 流水线、双向交叉读写（V4）；本条只是 V2 的格式层 builder 逐字节差分，单独开工会造成并行 oracle 构建。

需要什么：Linux（或 CI）+ 初始化 submodule + zstd/glog/boost/liburing 的构建环境，且 `01` 的 oracle/fixture 约定冻结后，再按既有结论的阶段 1→3 落地。

不硬做的理由：本 lane 允许改的只有 `fkv_config` / `fkv_page` / `fkv_codec` / `*_cpp`（空壳）/ `fkv_task_write`；`_cpp` 侧没有可调用的 oracle，任何实现都只是空转，且与 `01` 的产物约定冲突。

## 进展与阻塞收窄（2026-11-04 第二轮）

**page 层 V2 已打通并落地**，见 `issue/done/fmt-16a-v2-page-layer-diff-oracle.md`：

- 格式层（`data_page_builder` / `index_page_builder` / `data_page` / `compression` / `coding`）在 **macOS 本机可编译**（brew boost/glog/gflags/xxhash/zstd 已具备；`<liburing.h>` 不在该 include 链上）。
- 已产出并链接出可用 oracle，`fkv_page` 侧新增两个逐字节金标测试（data 页 63 字节 / index 页 37 字节）全绿。

修正上一条 `## 阻塞` 中的两点：
1. 「macOS 本地只能跑 Rust 侧单测」——对**整个 store**成立，对**格式层**不成立。
2. 「`eloqstore` 不是可构建 checkout」——对完整 target 成立（submodule abseil/concurrentqueue/inih 未初始化、需 liburing），但对格式层 page/coding/compression 不成立。

### 剩余阻塞

- `fkv_codec` / `fkv_compress` / `fkv_manifest` 差分的 oracle 入口、以及 5 个 `_cpp` crate 的 FFI 形态（Crubit vs 纯 C ABI）与 `fixtures/` 流水线，属 `01-cpp-oracle-and-fixtures.md` 的统一决定；
  `_cpp` crate 目前仍只有 doc comment，独立实现会与 `01` 的产物约定冲突（本 lane 不改 `_cpp` 之外的结构）。
- `fkv_manifest` 需 snapshot 完整构造 + CRC，`fkv_compress` 需固定 zstd 版本与参数，均超出本轮范围。

需要什么：先由 `01` 冻结 oracle/fixture 约定（或明确把格式层差分判给本 lane）；届时按 `fmt-16a` 已验证的方式补齐 codec/compress/manifest 三类。

## 审查结论（2026-11-04，第三轮）：reject（有意占位，非缺陷）

**判定：`_cpp` 空壳是既定的迁移脚手架，不是待修缺陷。** 依据：

1. **脚手架是设计产物，不是遗漏。** `Cargo.toml` 写明「Crubit 绑定与 `cpp/shim` 在此接入；默认不进入生产依赖图」；
   `todo.md:503` D10「89 个 package（44 Rust + 44 `_cpp` + `fkv_ledger`）已生成」——空壳是**有意生成**的占位。
2. **填充顺序有明文约定。** `cpp/shim/README.md`：「真正的 shim 随对应 crate 的迁移任务逐个补齐」，
   并规定了签名收敛规则（`string_view`/`span` 输入、POD/opaque 输出、不跨边界暴露 `std::function`/`boost::context`/absl）。
   `todo.md:498` D5 已定：真 C++ 对照限格式/算法层约 11 个，Crubit 只覆盖 `cpp/shim`。
3. **oracle 缺位是公开已知状态，不是被掩盖的漏项。** `doc/migration.md:26`「C++ oracle | 仅有占位；`cpp/eloqstore` 为空，44 个 `_cpp` crate 未实现」；
   `cpp/` 下只有 `cpp/shim/README.md`；`fixtures/cpp`、`fixtures/rust` 皆空；`cpp/eloqstore`、`cpp/build` 为空目录。
   原文「按 `migration.md` 会误以为 V2 已覆盖」不成立（§2 与 §5 都明写尚未系统化执行）。
4. **本 issue 最有价值的诉求（V2 缺证据）已部分兑现，其余有明确归属。**
   page 层 V2 已打通：本机可编译/链接 C++ oracle，`fkv_page` 新增 data/index 页逐字节金标测试，见
   `issue/done/fmt-16a-v2-page-layer-diff-oracle.md`。剩余 `fkv_codec`/`fkv_compress`/`fkv_manifest`
   的 oracle 入口、`_cpp` 的 FFI 形态（Crubit vs 纯 C ABI）与 `fixtures/` 流水线，属
   `issue/todo/01-cpp-oracle-and-fixtures.md` 的统一决定（唯一总入口），不应另起并行 oracle。

### 与既有结论的关系

第二轮的 `## 进展与阻塞收窄` 保留了有效信息（格式层本机可编译、fmt-16a 的 oracle 复现方式），
本轮据 dragon 的判定规则结束该 issue：**占位是设计的一部分，V2 执行归 01；本条不再追踪。**

关闭：无代码改动需求；`fmt-16a` 携带已完成的 page 层差分证据。
