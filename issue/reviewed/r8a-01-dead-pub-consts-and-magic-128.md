# r8a-01 P3：零引用 pub 常量/别名 ×2（MAX_OVERFLOW_POINTERS、DirectIoBuf），fkv_config 的 128 上限未消费前者、留魔法数字

## 现象（亲读）

1. `crate/foundation/fkv_types/src/lib.rs:132`：

   ```rust
   pub const MAX_OVERFLOW_POINTERS: u8 = 128;
   ```

   全仓 grep（`crate/ tool/ web/ cpp/ fixtures/`，含 tests，排除 `target/`）**仅此一处定义、零引用**。同文件相邻的 `MAX_READ_PAGES_BATCH`（:133，同源 C++ `max_read_pages_batch = max_overflow_pointers`）有真实消费者（`fkv_task_compact/src/lib.rs:65`），唯独本常量是孤儿。
   它的天然消费点是下条的魔法数字：

2. `crate/foundation/fkv_config/src/lib.rs:369`：

   ```rust
   if self.overflow_pointers == 0 || self.overflow_pointers > 128 {
   ```

   上界 128 以字面量出现，未引用 `MAX_OVERFLOW_POINTERS`。C++ 侧该常量有明确出处：`include/kv_options.h:23` `constexpr uint8_t max_overflow_pointers = 128;`，字段 doc（:391）明言「The maximum can be set to 128」。

3. `crate/core/fkv_buf/src/lib.rs:108`：

   ```rust
   /// 非页 IO 的缓冲别名；语义与 `AlignedBuf` 相同。
   pub type DirectIoBuf = AlignedBuf;
   ```

   全仓唯一另一处出现是 `fkv_buf_cpp/src/lib.rs:1` 的模块注释（提及 C++ `DirectIoBuffer`，非本类型）。**零消费者**（含 tests / facade / io 层），属 SKILL.md「pub API 孤儿（导出无人消费）」清理范畴。

## C++ 对照

- `include/kv_options.h:23-24`：`max_overflow_pointers = 128`、`max_read_pages_batch = max_overflow_pointers`——常量被引用，非孤儿。
- `DirectIoBuf` 对应 C++ `include/direct_io_buffer.h` 的 `DirectIoBuffer`；fkv 已由 `AlignedBuf` + compio fixed buffer 承接（io-03 / fkv_config 模块文档 `direct_io_buffer_pool_size` 条目），别名本身无实现语义，属迁移期残留命名。

## 目标

1. `fkv_config::validate` 的 `> 128` 改用 `u8::from(fkv_types::MAX_OVERFLOW_POINTERS)`（或 `u16::from` 比较等价形式），删除魔法数字；若裁定不接，则删除 `MAX_OVERFLOW_POINTERS` 常量（二选一，不留零引用导出）。
2. 删除 `fkv_buf::DirectIoBuf` 别名（连同注释）；若打算保留命名面，须给出真实消费点，否则按零死代码纪律清理。

## 验收

- `grep -rn "MAX_OVERFLOW_POINTERS" crate tool web cpp fixtures`：要么 ≥2 处（定义 + 消费点），要么 0 处（已删）。
- `grep -rn "DirectIoBuf" crate` 仅允许命中 C++ 对照 crate 的注释，`fkv_buf` 定义处 0 命中。
- `overflow_pointers = 128` 的 INI 配置用例仍通过（`fkv_config/tests/config.rs` 补边界断言：128 合法、129 拒绝）。
- `./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- 亲读 `fkv_types/src/lib.rs:122-134`、`fkv_buf/src/lib.rs:100-116`、`fkv_config/src/lib.rs:360-372`。
- 零引用验证：`grep -rn "MAX_OVERFLOW_POINTERS|DirectIoBuf" crate tool web cpp fixtures`（排除 target/）分别仅命中定义处与 `fkv_buf_cpp` 注释，mtime 以内容锚点复核。
- C++ 亲读 `include/kv_options.h:23-24/:390-393`；C++ `ValidateOption`（`src/kv_options.cpp:375-378`）只做 INI 读取、无 128 上限校验——Rust 收紧为显式 `InvalidArgs` 是既有决策，本条不动该行为，只消常量。
- 查重：`issue/reviewed/eng-81`（删的是 `fkv_manifest` 转发包装）、`eng-82`、`eng-78` 均未覆盖这两个符号；r7 各组报告无涉及。
- 并行写入区提示：`fkv_config/src/lib.rs` 在本轮审查末段有 <2 分钟 mtime 的并行改动，已重读确认 :369 锚点未变；`fkv_types`、`fkv_buf` 锚点文件 mtime 均在 1 小时以上，非在途状态。


## DONE（2026-11-05，第二轮复审修复）

DONE：`fkv_config` 改用 `MAX_OVERFLOW_POINTERS`；删除零消费 `DirectIoBuf` 别名。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`fkv_types/src/lib.rs:132` 常量仍在；现被消费——`fkv_config/src/lib.rs:50` 导入、`:397` 比较 `self.overflow_pointers > MAX_OVERFLOW_POINTERS`，魔法 128 已消。`DirectIoBuf` 别名已从 `fkv_buf/src/lib.rs` 删除（全仓 grep 仅剩 `fkv_buf/Cargo.toml:8`、`fkv_buf_cpp` 描述串，均指 C++ `DirectIoBuffer`）。零死代码纪律（SKILL 12）满足：无 re-export、无 doc 引用残留。
- 验收：`grep -rn MAX_OVERFLOW_POINTERS crate|tool|web|cpp|fixtures` = 定义 + 导入 + 比较三处（≥2 达）；`grep DirectIoBuf crate` 在 `fkv_buf` 定义处 0 命中。
- 残余（转 `issue/new/v-01`）：验收第 3 条「`fkv_config/tests/config.rs` 补 128/129 边界断言」未落实——全 crate 无任何测试引用 `overflow_pointers`（`grep -rn overflow_pointers crate/foundation/fkv_config/tests/` 空）。主项两目标已达成，故本条判定完善；边界测试缺口另行立 v-01。
- 判定：完善（残余已拆出）。
