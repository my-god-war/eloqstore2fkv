# v-01 P3：fkv_config `overflow_pointers` 校验无任何测试——`validate` 的 0 / 128 / 129 边界零覆盖（r8a-01 验收残余）

## 来源

验证轮 lane A（2026-11-05 v）复核对 [`r8a-01`](../reviewed/r8a-01-dead-pub-consts-and-magic-128.md) 时，其验收第 3 条
「`overflow_pointers = 128` 的 INI 配置用例仍通过（`fkv_config/tests/config.rs` 补边界断言：128 合法、129 拒绝）」
未落实，主项（复用 `MAX_OVERFLOW_POINTERS`、删 `DirectIoBuf`）已完成，残余测试缺口拆出为本条。

## 现象（亲读，2026-11-05）

- `crate/foundation/fkv_config/src/lib.rs:397`：`if self.overflow_pointers == 0 || self.overflow_pointers > MAX_OVERFLOW_POINTERS { return Err(KvError::InvalidArgs); }`
  ——行为正确（0 拒、128 合法、129 拒），但无任何测试断言该边界。
- 测试缺口证据：
  - `grep -rn "overflow_pointers" crate/foundation/fkv_config/tests/` → 0 命中；
  - `grep -rn "overflow_pointers" crate --include="*.rs"` 排除 `fkv_config/src` → 无一处测试代码（`fkv_btree/src`、`fkv_io_compio`、`fkv_sdk`、`fkv_shard/tests`、`fkv_store/tests` 等均为字段透传或 `DEFAULT_OVERFLOW_POINTERS` 构造，无 validate 边界断言）；
  - `fkv_config/tests/config.rs` 现有 `invalid_options`（:104-140）覆盖 `num_threads=0`、两个 restart interval=0、`max_inflight_write=0`、`max_write_batch_pages=0`、`standby_master_addr`，**不含 `overflow_pointers`**。
- 影响：`MAX_OVERFLOW_POINTERS`（128）上界回归无护栏；128/129 语义仅在代码里正确，无测试锁定。

## 目标

在 `crate/foundation/fkv_config/tests/config.rs` 补 `overflow_pointers` 边界断言（对齐 r8a-01 验收口径）：

1. `overflow_pointers = 0` → `validate()` 返回 `InvalidArgs`；
2. `overflow_pointers = 128`（`MAX_OVERFLOW_POINTERS`）→ `Ok`；
3. `overflow_pointers = 129` → `InvalidArgs`。

可并入现有 `invalid_options` 或独立 `#[test]`；不得写「常量 == 字面量」式废话断言（rust_review SKILL §10），
断言应走 `KvOption { overflow_pointers: X, ..Default::default() }.validate()` 的真实路径。

## 验收

- `grep -rn "overflow_pointers" crate/foundation/fkv_config/tests/` ≥1 命中，且断言覆盖 0 / 128 / 129 三值；
- `./sh/clippy.sh`、`./test.sh` 通过（纯测试补充）。

## 查重

- `grep -rln "MAX_OVERFLOW_POINTERS" issue/` 仅 `r8a-01`（已归档）与 `00-index.md`；`grep -rln "overflow_pointers" issue/` 命中的 `eng-63/63a/72` 均与 validate 边界无关；
- `issue/new/`（本条除外）与 `issue/todo/` 无承接条目。

## 甄别（2026-11-05 v）

**采纳，`issue/new/` → `issue/todo/`。测试缺口属实、校验逻辑本身正确，属纯「补测试」任务。**
验：已归档 r8a-01 验收第 3 条残留，主项（复用 `MAX_OVERFLOW_POINTERS`、删 `DirectIoBuf`）已完成，故边界测试缺口独立成条。

### 核实结论（亲验）

1. **校验逻辑正确**：`fkv_config/src/lib.rs:397` `if self.overflow_pointers == 0 || self.overflow_pointers > MAX_OVERFLOW_POINTERS { return Err(KvError::InvalidArgs); }`；
   常量 `fkv_types/src/lib.rs:134` `pub const MAX_OVERFLOW_POINTERS: u8 = 128;`。
   字段 `overflow_pointers: u8 = 16`（:294），`129` 在 u8 内可表示，三值（0/128/129）均可走真实类型路径造出。
2. **C++ 对照语义一致**：`eloqstore/include/kv_options.h:23` `constexpr uint8_t max_overflow_pointers = 128;`，
   `src/eloq_store.cpp:197-201` 同样 `== 0 || > max_overflow_pointers` 拒绝；Rust 侧与之逐字对齐。
   C++ 单测 `tests/eloq_store_test.cpp:62-64` 仅覆盖 `overflow_pointers = 0`，128/129 亦缺——故本缺口非 Rust 独有，但以 C++ 为准绳该边界确为有意约束。
3. **测试缺口属实**：`grep -rn "overflow_pointers" crate/foundation/fkv_config/tests/` 0 命中（亲验）；
   现有 `invalid_options`（`config.rs:103-140`）覆盖 `num_threads` / 两个 restart interval / `max_inflight_write` /
   `max_write_batch_pages` / `standby_master_addr`，**不含 `overflow_pointers`**。
4. **边界值值得测**：`0`（下界排除）、`MAX`（上界含）、`MAX+1`（上界+1）是真实语义边界，
   非「常量 == 字面量」废话（rust_review SKILL §10）。断言走上界应引用常量（`MAX_OVERFLOW_POINTERS` /
   `MAX_OVERFLOW_POINTERS + 1`）而非硬编码 128/129；下界 0 为字面量可接受。
   影响：128 上界回归当前无护栏，128/129 语义仅存于代码、无测试锁定。
5. **查重**：全 `issue/` grep `overflow_pointers` 命中 r9a-02 / eng-63 / 63a / 72 / 00-v-lane-a / r8a-01，
   均为引用/出处，非承接（`00-v-lane-a.md:46` 自述「`issue/` 无承接条目」）；`issue/new/`（本条除外）
   与 `issue/todo/`（eng-91 / eng-92）无承接条目。

### 开发计划

- 目标文件：`crate/foundation/fkv_config/tests/config.rs`（只加测试，不改 `src`）。
- 新增独立 `#[test] fn overflow_pointers_validate_boundary`（推荐独立；并入 `invalid_options` 亦可）：
  1. `KvOption { overflow_pointers: 0, ..Default::default() }.validate()` → `Err(KvError::InvalidArgs)`；
  2. `KvOption { overflow_pointers: MAX_OVERFLOW_POINTERS, ..Default::default() }.validate()` → `Ok(())`；
  3. `KvOption { overflow_pointers: MAX_OVERFLOW_POINTERS + 1, ..Default::default() }.validate()` → `Err(KvError::InvalidArgs)`。
- 边界来源：`MAX_OVERFLOW_POINTERS`（`fkv_types`，`=128`，同源 C++ `include/kv_options.h:23`）；
  test 顶部补 `use fkv_types::MAX_OVERFLOW_POINTERS;`（现仅 `use fkv_types::StoreMode;`）。
- 可选（对齐 r8a-01 验收 #3 的 INI 口径）：补一条 INI 用例——`[run]\nnum_threads = 1\n[permanent]\noverflow_pointers = 128`
  解析后 `validate()` 为 `Ok`，证明 INI 路径 `parse_num` 不回落/不越界。

### 验收

- `grep -rn "overflow_pointers" crate/foundation/fkv_config/tests/` ≥1 命中，且断言覆盖 0 / 128 / 129 三值；
- `./sh/clippy.sh`、`./test.sh` 通过（纯测试补充，无生产代码改动）。


## DONE（2026-11-05）

DONE：新增 `validate_overflow_pointers_boundaries`，覆盖 0 / MAX / MAX+1。
