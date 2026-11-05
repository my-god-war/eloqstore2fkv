# r8a-02 P3：fkv_compress 两处文档引用不存在的 `issue/new/08`，且「待字典自动训练接线」表述与已接线事实相反

## 现象（亲读）

1. `crate/format/fkv_compress/src/lib.rs:309-310`（`DictCompression::prepare` doc）：

   ```rust
   /// 与 `Prepare` 对齐：短值不压缩；超上限走 standalone；有字典的中间区间走
   /// 字典；无字典的中间区间暂不压缩（见 `issue/new/08`，待字典自动训练接线）。
   ```

2. `crate/format/fkv_compress/tests/compress.rs:78`（`prepare_threshold_boundary` 注释）：

   ```rust
   // 无字典时中间区间不压（issue/new/08），所以恰好等于阈值也只能透传。
   ```

两处指向的 `issue/new/08` 在仓库内**不存在**：`issue/new/` 现仅有 `00-index.md`、`r8c-01`、`r8c-02`、`r8d-01`；`issue/reviewed|done|reject|todo` 无 `08-*` 亦无 `fmt-12`；`git log --all --name-only` 检索字典相关 issue 文件零命中（仓库单 init 提交，无更早历史可追）。

「待字典自动训练接线」也与代码事实相反：

- 自动训练已接线：`crate/engine/fkv_store/src/lib.rs:1620-1638` 在每次 `put_at_inner` 里 `get_or_insert_with(DictCompression::new)` 并调用 `compression.maybe_build_dictionary(&sample_li)`，注释明言「自动字典训练（对齐 C++ `SampleAndBuildDictionaryIfNeeded`）：样本够了就建字典，同批写入即可走 Dictionary 压缩；字典字节随 manifest 持久化」。
- 训练「不重试、样本即弃」的语义已由同 crate 测试 `dictionary_training_does_not_retry_or_keep_samples`（`tests/compress.rs:135`，fmt-22）锁定——模块文档仍停留在接线前的旧叙事。

连带失真：`fkv_store/src/lib.rs:1635` 注释写「样本保留，后续批次再尝试训练」，与 `maybe_build_dictionary` 的 `training_attempted` 早退（首次尝试后样本 `take` 清空、永不重试，对齐 C++ `has_dictionary_ = true` 副作用）相矛盾（该文件不在本路 scope，随本条一并提示，修不修由 store 归口裁定）。

## C++ 对照

行为本身无差异：C++ `Prepare`（`src/compression.cpp:94-121`）在无字典时同样返回 `None` 不压。C++ `SampleAndBuildDictionaryIfNeeded`（:196-245）按批采样、达标即 `BuildDictionary`；`BuildDictionary`（:266-292）先置 `has_dictionary_ = true` 再训练，失败也不重试——Rust `training_attempted` 即对齐此副作用（代码内注释已写明）。本条纯为 Rust 侧文档滞后，不涉盘面。

## 目标

1. `fkv_compress/src/lib.rs:309-310` doc 重写：去掉 `issue/new/08` 悬空引用与「待接线」，改为现状口径——「无字典的中间区间不压（与 C++ `Prepare` 一致）；字典由 `maybe_build_dictionary` 自动训练（fmt-12 记录整批入样差异），训练失败不重试（fmt-22）」。
2. `tests/compress.rs:78` 注释同步去掉悬空引用。
3. 提示 `fkv_store/src/lib.rs:1635` 附近「样本保留，后续批次再尝试训练」注释与 `training_attempted` 语义矛盾，归口更正（本条验收不强制，需 store 侧确认）。

## 验收

- `grep -rn "issue/new/08" crate doc issue` 零命中。
- `fkv_compress` 模块 doc / 测试注释与 `fkv_store` 接线现状、`fmt-22` 测试语义一致，无「待接线」类旧叙事。
- `./sh/clippy.sh`、`./test.sh` 通过（纯注释改动）。

## 核验记录

- 亲读 `fkv_compress/src/lib.rs` 全文（:296-329 `prepare`、:233-268 `build_dictionary` / `maybe_build_dictionary`、:153-164 `training_attempted`）与 `tests/compress.rs:55-145`。
- 亲读 `fkv_store/src/lib.rs:1595-1645`（训练接线 + 失实注释）；`fkv_store` 归属其他路 scope，本条仅以 `fkv_compress` 侧两处文档为立项锚点。
- 悬空验证：`grep -rn "issue/new/08" crate doc issue tool` 全仓 0 命中文件本体；`grep -rn "maybe_build_dictionary" issue/` 无任何 issue 承接训练接线叙事。
- C++ 亲读 `src/compression.cpp:94-121/:196-292`、`include/compression.h:45-47`（kMaxDictBytes/kSampleTargetBytes/kMinSamples 与 Rust 常量一致）。
- 查重：r6/r7 报告、`issue/reviewed/fmt-*`、`issue/reject/` 均无本条；`rev-48`（eng-33a 文档滞后）与 `r8d-01`（write_gate.rs 文档滞后）同类不同点，无重叠。
- 锚点文件 mtime：`fkv_compress/src/lib.rs` 1.7 小时前、`tests/compress.rs` 1.8 小时前，非并行写入区。


## DONE（2026-11-05，第二轮复审修复）

DONE：删除不存在的 `issue/new/08` 引用；模块 / 测试注释改为“对齐 C++ Prepare”，store 注释改为首次尝试后不重试。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`fkv_compress/src/lib.rs:309-310` 已改为「无字典的中间区间暂不压缩（对齐 C++ `Prepare`；自动训练在 `fkv_store` 写路径接入）」，无 `issue/new/08`、无「待接线」；`fkv_compress/tests/compress.rs:78` 改为「（对齐 C++ Prepare）」。
- DONE 附带的 store 注释同步亦核实：`fkv_store/src/lib.rs:1708` 现为「首次尝试后即置 `training_attempted`，失败也不重试、样本随 `take` 丢弃」，与 `maybe_build_dictionary` 语义一致（旧「样本保留，后续批次再尝试训练」失实注释已消）。
- 验收：`grep -rn "issue/new/08" crate doc` 零命中（仅本条 issue 自身正文含该串，属跟踪记录非源码引用）。
- 判定：完善。
