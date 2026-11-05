# fixtures

- `cpp/`：C++ oracle 生成，Rust 读取。
- `rust/`：Rust 生成，回喂 C++ 读取。
- `generated/`：运行时临时生成，gitignore；入库语料直接由 `fkv_ledger` 写入
  `cpp/` / `rust/`（每 case 带 `meta.txt`）。

## 生成 / 校验

由 `fkv_ledger` 的 `fixture` / `verify` 驱动，目录结构固定：

```text
<dir>/<kind>/<case>/meta.txt      清单（逐行 key=value）
<dir>/<kind>/<case>/payload.bin   字节，由 C++ oracle 产生；skipped 时为空
<dir>/<kind>/<case>/dict.bin      仅 compress/dict：oracle 训练的 zstd 字典
```

`kind` ∈ `page` / `index` / `overflow` / `manifest` / `compress` / `mapping` / `store`。
7 类全部由 C++ oracle 产真实字节；`mapping` / `store` 的 payload 格式见下节。

### case 矩阵

规范 case 名内置在 `fkv_ledger::fixture::MATRIX`，`fixture --matrix` 按表全量生成
（`--case` 仍可传自由名做临时试验，验收覆盖以矩阵为准）：

| kind | case |
|---|---|
| `page` | `empty` `single` `multi` `restart` `ttl` `large` `overflow` |
| `index` | `leaf` `branch` |
| `overflow` | `chain` |
| `manifest` | `empty` `branch` `checksum` `archive` |
| `compress` | `raw` `dict` |
| `mapping` | `empty` `single` `multi` |
| `store` | `empty` `single` `multi` |

### `mapping` / `store` payload 格式

> `mapping` / `store` 是 **fkv 自定义 format-envelope**，不是 eloqstore 的完整
> 盘面格式，也不能作为「引擎级 store 目录互读」证据。引擎级 V2/V4 仍挂在
> `issue/new/gap-02-engine-level-v2-v4-cross-read.md`。
>
> 统计口径（gap-09）：级别不是独立字段，而是 `kind` 的纯函数 ——
> `kind ∈ {mapping, store}` 记 **format-envelope**；其余 5 类（`page` / `index` /
> `overflow` / `manifest` / `compress`）是**真实 C++ builder 字节**，记
> **format-bytes**。**`level=engine` 当前为 0 条**，所以任何「引擎级覆盖」统计
> 都不得把前两类计入。

`mapping`（`MappingSnapshot` 表序列化 + count 信封）：

```text
page_count(varint32) | page_count × encoded_value(varint64)
```

- `encoded_value` 就是 eloqstore `MappingSnapshot::Serialize` 写出的值：
  低 3 位是 `ValType`（`1` = FilePageId，`2` = PageId，其它为 Invalid），
  高位是 `FilePageId` / `PageId`；
- 槽位顺序即 `PageId` 升序（`MappingTbl` 按位置遍历）；
- `page_count` 前缀让 `empty`（0 页）也有非空 payload，并显式给出逻辑页数；
- case：`empty` 0 页；`single` 1 个 `FilePageId(42)`；`multi` 8 槽，覆盖
  `FilePageId`、`Invalid`、`PageId` 与多字节 varint。

`store`（目录 / 文件清单）：

```text
magic "FST1"(4B) | file_count(varint32) |
file_count × [ name_len(varint32) | name(UTF-8) | data_len(varint32) | data ]
```

- 文件按 `name` 字节序升序，读取方必须校验排序与「恰好耗尽 payload」；
- `name` 用 eloqstore 真实命名规则（`common.h`）：
  `data_<file_id>_<term>`、`manifest_<term>`、`manifest_<term>_<ts>`、
  `segment_<file_id>_<branch>_<term>`；
- `data` 内容来自真实 `DataPageBuilder`，`manifest_*` 来自
  `ManifestBuilder::Snapshot` / log record，`segment_*` 为固定 pattern；
- case：`empty` 0 文件；`single` 1 个 data 页；`multi` = 2 个 data 页 +
  branch snapshot + archive snapshot + segment 文件（5 个文件）。

Rust 侧解码入口：`fkv_ledger::corpus::decode_case`（verify 使用）与
`crate/compat/fkv_compat/tests/{mapping,store}_oracle.rs`（`MappingTbl::deserialize`
+ format parser 断言）。

```bash
# 只有清单（无 oracle 时 status=skipped），先打通流水线
cargo run -p fkv_ledger -- fixture --kind page --case smoke --out fixtures/generated

# 按矩阵全量生成清单
cargo run -p fkv_ledger -- fixture --matrix --out fixtures/generated

# 有 oracle 时由其产生 payload 并标记 pass/fail
cargo run -p fkv_ledger -- fixture --kind page --case smoke \
  --oracle /path/to/fkv_oracle --out fixtures/generated

# 校验并输出 Markdown 报告；有 fail 时退出码非 0
cargo run -p fkv_ledger -- verify --fixtures fixtures/generated \
  --oracle /path/to/fkv_oracle --report /tmp/fixture-report.md

# CI 同款强制模式：缺语料 / payload 为空必须失败（gap-08）
FKV_REQUIRE_ORACLE_FIXTURES=1 cargo test -p fkv_compat
```

`meta.txt` 字段：

| key | 含义 |
|---|---|
| `schema` | 清单格式版本（当前 `1`），`verify` 不符即判 fail，旧 fixture 不会静默通过 |
| `kind` | fixture 类别 |
| `case` | case 名 |
| `baseline` | 生成时的 `COMPAT_BASELINE`（多行压成一行，` | ` 分隔） |
| `status` | `skipped` / `pass` / `fail` |
| `oracle` | 实际使用的 oracle 路径；未用 oracle 时为 `-` |
| `payload` | payload 文件名 |

`verify` 计数语义：

- `schema` 不符或缺失 → `fail`；
- `status=fail`（oracle 生成阶段就失败）→ 始终计 `fail`，不因本次未传 oracle 而降级为 `skipped`；
- `status=skipped`（生成时没有 oracle）→ `skipped`，需先重新生成；
- `status=pass` 但 `payload.bin` 缺失或为 0 字节 → `fail`（oracle 没真正产出）；
- `status=pass`：**先跑 Rust 侧解码断言**（`fkv_ledger::corpus::decode_case`：
  语义逐条 + 与 Rust 重建字节逐字节比对，失败在报告里给出首个差异 `offset`），
  再传 oracle 调 `<oracle> verify --kind K --case C --case-dir DIR` 判定 pass/fail；
  没传 oracle 记为 `skipped`（Rust 断言已通过，C++ 交叉读待补）。

oracle 约定接口：

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

退出码 0 视为成功；`verify` 必须显式传 `kind` / `case`，同一类页字节
（data / index / 压缩 / manifest）由 kind 选择解析器。

## C++ oracle（`cpp/oracle`）现状

- 源码：`cpp/oracle/src/main.cpp` + `standalone.cpp`（`Page` 分配 / `Option()` 的
  最小运行期 shim，不参与字节编码）；
- 已实现 kind / case：
  - `page`：`empty` `single` `multi` `restart` `ttl` `large`（别名 `large_values`）`overflow`；
  - `index`：`leaf` `branch`；
  - `overflow`：`chain`（data entry + 单组连号 overflow 页，覆盖 overflow 页自身格式）；
  - `manifest`：`empty` `checksum` `branch` `archive`。`branch` / `archive`
    是 `ManifestBuilder::Snapshot` 的真实快照布局；`empty` / `checksum` 是
    log record 布局（覆盖 delta / checksum）。snapshot 的可选 segment 段只在
    segment mapping 存在时写入。
  - `compress`：`raw` `dict`（`dict` 额外写 `dict.bin`）。
  - `mapping`：`empty` `single` `multi`。用 `MappingSnapshot::MappingTbl` +
    `MappingSnapshot::Serialize` 产出映射表字节，外加 `page_count` 前缀（格式见上节）。
  - `store`：`empty` `single` `multi`。产出有序 `name/len/bytes` 文件清单；
    其中 data 页来自 `DataPageBuilder`，manifest 来自 `ManifestBuilder`，
    文件名遵循 eloqstore `common.h` 的真实命名规则（格式见上节）。
- `manifest` fixture 只保留 `header + payload_len`，不写 OS page-size 对齐 padding：
  padding 不参与 checksum，且 macOS（16KiB）与 Linux（4KiB）不同，写进语料会让
  字节不可跨平台复现；读取方按 `payload_len` 解析，与真实读路径一致。
- 构建：`cpp/oracle/build.sh`（只编格式层 TU，不需要 io_uring / S3 / 整库构建）。
  `ManifestBuilder::Snapshot` 需要构造 `MappingSnapshot`，其 `MappingTbl` 依赖
  absl，因此脚本会用 eloqstore pin 住的 `external/abseil` 源码构建静态库再链接
  （不能混用系统 absl：inline namespace / ABI 不一致）：

```bash
scripts/fetch_eloqstore.sh                 # 取 eloqstore 到 cpp/eloqstore（pin COMPAT_BASELINE）
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle
/tmp/fkv_oracle fixture --kind page --case multi --out /tmp/fx/page/multi
/tmp/fkv_oracle verify  --case-dir /tmp/fx/page/multi --kind page --case multi
# 或走容器（就是 CI 的形态）：
docker compose -f docker/docker-compose.yml run --rm oracle bash -lc \
  'ELOQSTORE_SRC=/src/eloqstore bash /src/eloqstore2fkv/cpp/oracle/build.sh /data/fkv_oracle'
```

构建依赖（Linux 容器）：`cmake`（构建 eloqstore 自带的 abseil 静态库）、
`g++ >= 13`（eloqstore 头用到 `<format>`）、`libzstd-dev`、
`libgoogle-glog-dev`、`libgflags-dev`、`libjsoncpp-dev`、`liburing-dev`、
`libcurl4-openssl-dev`、`libboost-all-dev`（见 `docker/Dockerfile.oracle`）。
macOS 本地也可构建：`build.sh` 自动为 `<jsoncpp/json/json.h>` 与缺失的 `liburing.h`
生成临时 include shim，并用 `-idirafter` 避免 eloqstore 根目录的 `version` 文件
遮蔽 C++ 标准头 `<version>`。

- **已入库的固定语料**：`fixtures/cpp/<kind>/<case>/{payload.bin,meta.txt}`
  （compress/dict 另含 `dict.bin`），7 类 22 个 case（C++ 写、Rust 读）。
- 本仓 `crate/**/*_cpp` 共 **39** 个 C++ 对照骨架 crate，与 fkv 主仓的
  Rust crate 一一对应；它们只做差分 / 对照，不进入生产路径。
- **反向语料**：`fixtures/rust/<kind>/<case>/`（Rust builder 写同一语料 → C++ oracle
  读回），`cargo run -p fkv_ledger -- fixture --rust --matrix --out fixtures/rust`
  生成；语料定义在 `tool/fkv_ledger/src/corpus.rs`，与 `cpp/oracle/src/main.cpp`
  一一对应，两侧必须同步修改。
- Rust 侧解码测试（不需要 C++ 工具链，CI 常规 `cargo nextest` 就能跑）：
  - `tool/fkv_ledger/tests/corpus.rs`：全矩阵解码断言 + 篡改报首个差异 offset + 反向自检；
  - `crate/compat/fkv_compat/tests/page_oracle.rs`：data page + index page；
  - `crate/compat/fkv_compat/tests/compress_oracle.rs`：raw / dict 解压；
  - `crate/compat/fkv_compat/tests/manifest_oracle.rs`：manifest snapshot（branch /
    archive，含可选 segment 段的有无）与 log checksum / branch metadata；
  - `crate/compat/fkv_compat/tests/mapping_oracle.rs`：`MappingTbl::deserialize`；
  - `crate/compat/fkv_compat/tests/store_oracle.rs`：目录文件清单 + 内嵌页 / manifest 解析。
- CI：`.github/workflows/oracle.yml` 的 `fixture-diff` 作业 = LFS checkout →
  构建 oracle → `fkv_ledger fixture --matrix --oracle` 直接写 `fixtures/cpp`
  （含 `meta.txt`，不再先写 `generated` 再 cp）→ `git diff --exit-code
  fixtures/cpp` 检查语料漂移 → Rust 侧解码 → `fkv_ledger verify`
  （Rust 断言 + C++ 交叉读）→ 反向 fixture 生成与读回 → 上传产物。
  两个作业都设置 `FKV_REQUIRE_ORACLE_FIXTURES=1`（缺语料直接失败，不静默 skip），
  并在开头断言 `getconf PAGESIZE == 4096`（manifest framing / 4 KiB 语料口径）。
- **一键重新生成**（幂等，字节可复现；版本记录见 meta.txt 的 `baseline` 字段
  与仓库根 `COMPAT_BASELINE`）：

```bash
ELOQSTORE_SRC=/path/to/eloqstore bash scripts/gen_fixtures.sh
```

## 待补（P0 剩余）

- V2/V4（含续写 / 引擎级）未系统化执行；`COMPAT_BASELINE` 证据行与本目录
  `meta.txt` `baseline=` 字段的口径修订见
  `issue/reviewed/eng-91-compat-baseline-evidence-line-stale-and-baked-into-fixtures.md`。
- branch + large value：C++ `CreateBranch` 不写 snapshot 的可选 segment 段，
  Rust 为保留 large value 继承仍写；字节差异与决策见
  `issue/reviewed/02-cpp-create-branch-omits-segment-mapping.md`。
