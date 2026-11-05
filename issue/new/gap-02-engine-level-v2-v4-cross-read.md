# gap-02 引擎级交叉读（V2/V4 含续写）未执行；`mapping`/`store` 语料仍是占位（P1）

## 来源

dragon 的剩余工作盘点；`doc/migration.md` §5 与 `COMPAT_BASELINE` 已自述该项未做。

## 证据（2026-11-05 实测）

- `doc/migration.md` §5「当前实际覆盖」：V2 只覆盖**格式层 5 类 16 case**；V4 只覆盖
  **格式层双向**（`fixtures/cpp`、`fixtures/rust`）；明确写着
  「**引擎级 V4（含续写）未执行**」。
- `fixtures/cpp/mapping/{empty,multi,single}/meta.txt` 与
  `fixtures/cpp/store/{empty,multi}/meta.txt` 的 `status=` 实测均为 **`skipped`**
  （`payload.bin` 为空占位）。
- `COMPAT_BASELINE:83-84`：L1 升「已验证」的前置 = `sto-51` 修复 + **V2/V4 执行**。
- 同一文件另有一条**硬约束**：manifest record framing 用 OS 页大小，
  **Linux 4 KiB / Apple Silicon 16 KiB 不可互读**，双向验收须在同一页大小主机执行。

## 问题

当前 `L1-partial` 只能支撑「格式层互操作」，**不能支撑「引擎级双向兼容」结论**：
没有引擎级证据时，任何「与 C++ 兼容」的对外说法都超出已证范围。
另外 `mapping` / `store` 两类语料是**格式层**的缺失（本可就近补），却和引擎级项一起挂着。

## 目标

1. **先补格式层的 `mapping` / `store` 语料**（与现有 16 case 同类，走
   `fkv_ledger fixture --oracle` + `verify`）：mapping 表序列化、store 目录布局。
2. 再上**引擎级**：C++ 写 → Rust 读 → Rust 续写 → C++ 读的闭环，覆盖
   manifest log 续写、data/segment 文件、归档与分支三类形态各至少 1 例。
3. 语料生成与差分**放进 CI**（沿用既有 `oracle.yml` 的 `fixture-diff` 作业形态），
   并在 4 KiB 页 runner 上执行。

## 验收

- `fixtures/cpp/{mapping,store}/**/meta.txt` 由 `skipped` 变为 `pass`，且
  `payload.bin` 非空、可被 Rust 侧解码测试消费（`fkv_page` / `fkv_mapping` 的
  `cpp_oracle` 测试形态）。
- 引擎级用例：Rust 侧能打开 C++ 产出的 store 目录并读出期望键值；续写后再由 C++
  侧读出成功（C++ 侧脚本可先只记录命令与期望输出，不强求本仓 CI 内跑 C++）。
- CI 作业失败时能区分「语料漂移」与「生成失败」（现有 `status=fail` 语义已支持）。

## 阻塞 / 依赖

**外部前置（硬）**：一台 **4 KiB 页的 Linux** + eloqstore 构建依赖
（liburing / jsoncpp 等）+ 网络（拉镜像/依赖）。本机 macOS/Apple Silicon
无法完成：既缺 liburing，`directory_iterator` 侧的页大小语义也不匹配。
→ 本项在本机只能做到「脚本与测试骨架就位、语料缺席时 skip」。

## 核验记录

- 实测：`fixtures/cpp/mapping/*/meta.txt` 的 `status=skipped`；
  `doc/migration.md` §5 与 `COMPAT_BASELINE` 的自述。
- 既有骨架可复用：`cpp/oracle/src/main.cpp`、`cpp/oracle/build.sh`、
  `scripts/fetch_eloqstore.sh`、`fkv_page/tests/cpp_oracle.rs`。

## 复审（2026-11-05）

原 index 把它归为“验证债务”不成立：`mapping` / `store` oracle 直接 `exit 3`，
6 个 case 全是 0 字节占位；这是缺实现。4 KiB Linux 机器已由 `oracle.yml` 的
`ubuntu-26.04` runner 提供，只需补 `getconf PAGESIZE` 断言。
