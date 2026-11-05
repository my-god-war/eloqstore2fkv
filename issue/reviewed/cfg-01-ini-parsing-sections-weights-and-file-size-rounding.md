# cfg-01 INI 解析三处分歧：分节语义（错节键被应用）、store_path 内联权重不支持、data_file_size 非二次幂拒绝而非取整

## 位置
- Rust: `crate/foundation/fkv_config/src/lib.rs:243-337`（`load_from_ini`：`[section]` 行直接 continue「只做忽略」—— 任何节的键都覆盖默认值）；`:306-309`（`store_path` 仅逗号切分，权重需独立 `store_path_weights` 键）；`:326-336`（`data_file_size` 换算 pages 非二次幂 → `Err(InvalidArgs)`）
- C++: `src/kv_options.cpp:93-97,261-265`（缺 `[run]`/`[permanent]` 节返回 `-2`；错节的键被忽略，如 `num_threads` 写在 permanent 节不生效）；`:267-277`（`ParseStorePathListWithWeights` 支持 `path1,path2:<w1,w2>` 内联权重后缀，失败返回 `-3`）；`:341-373`（`data_file_size` 非二次幂倍数**向下取整并 WARNING**（12MB→8MB），小于 page size 回退单页，均不报错）

## 差异与风险
同一份生产 ini 两边语义不同：
- C++ 接受的 `data_file_size=12MB` 被 fkv 拒绝（启动失败）；
- 错节的键 C++ 忽略、fkv 应用（配置漂移，且 fkv 无缺节校验）；
- 带内联权重的 `store_path` 在 fkv 变成含 `:<w>` 的字面路径（目录创建失败或数据放错盘）。

## 对齐建议
1. `load_from_ini` 记录当前节并按 `[run]`/`[permanent]` 分发，缺节返回 `InvalidArgs`；
2. `store_path` 支持 `:w1,w2` 后缀语法（解析失败报错对齐 `-3`）；
3. `data_file_size` 非二次幂时向下取整 + warn，小于 page size 回退单页。

## 严重度
P1（生产配置直接兼容性：三处都让「同一份 ini」两边行为不同）

## 核验记录（2026-11-04）
Rust 三处行为实读确认（`:243-337` 全段）；C++ 三处引用自子代理摘录（含返回码 `-2/-3` 细节），开发时按行号复核。

## 审查结论（2026-11-04）

**采纳（全部属实，三处逐一在仓内核验）。**

C++ 源码本地可读：`/Users/z/git/db/kv/eloqstore`（`COMPAT_BASELINE 82b774c`）。行号与 issue 描述一致。

| 断言 | 结论 | 证据 |
|---|---|---|
| 分节语义：错节键被应用 | 真 | Rust 旧 `load_from_ini` 对 `[` 行直接 continue，键不分节。C++ `src/kv_options.cpp:89-97,259-265`：`HasSection("run")`/`HasSection("permanent")` 缺失返回 `-2`，键用 `HasValue(sec, key)` 绑定节。 |
| `store_path` 内联权重不支持 | 真 | Rust 旧代码 `store_path` 仅 `split_list`。C++ `include/utils.h:115-176` `ParseStorePathListWithWeights` 支持 `path1,path2:w1,w2`，失败返回 `-3`（`kv_options.cpp:267-277`）；`standby_master_store_paths` 同机制（`kv_options.cpp:287-301`）。 |
| `data_file_size` 非二次幂拒绝而非取整 | 真 | Rust 旧代码 `!pages.is_power_of_two()` → `Err`。C++ `kv_options.cpp:341-374`：`pages_per_file==0` 回退单页 + WARNING；非二次幂向下取整 + WARNING，均不报错。 |

### 子项（同一函数内三处，合并为一次提交）

1. 节状态机：新增 `enum Section { Run, Permanent }`，`match (section, key)` 分发；缺 `[run]`/`[permanent]` → `InvalidArgs`。
2. `store_path` / `standby_master_store_paths` 支持 `:w1,w2` 内联权重，个数不匹配或权重非法 → `InvalidArgs`。
3. `data_file_size` 非二次幂向下取整 + `warn!`，小于 page size 回退单页。

### 附带对齐（同源问题）

- C++ `enable_local_standby` 不经 `.ini` 解析（`include/kv_options.h:324`，仅代码设置），归入 fkv 扩展键。
- 删除 Rust 专有键 `store_path_weights`（C++ 无此键，语义被内联后缀取代）；其余 fkv 专有键（`chunk_size`/`segment_size`/`*_shift`/`*compression*`/`auto_*`）集中到 `apply_ext`，两节均可接受。

## DONE（2026-11-04）

### 实现要点

- `crate/foundation/fkv_config/src/lib.rs`：
  - `load_from_ini` 重写为节状态机（`Section` enum + `match (section, key)`），节外键忽略，缺节 `InvalidArgs`。
  - 新增 `parse_store_path`：`path1,path2:w1,w2`，无后缀权重为空，个数不符/非法权重报错。
  - `data_file_size` 换算改为 `ilog2` 向下取整 + `warn!`；`pages==0` 回退 `shift=0`。
  - 新增 `KvOption::apply_ext` 承载 fkv 专有键；移出 C++ 分节键。
  - `use log::warn;`；`cargo add log --features std -p fkv_config`。
- `crate/foundation/fkv_config/tests/config.rs`：`load_ini` 修正 `data_append_mode` 归 `[permanent]`；新增 `ini_section_scope`、`ini_missing_section_rejected`、`ini_store_path_inline_weights`、`ini_data_file_size_rounding`。

### 测试

- `cargo nextest run -p fkv_config` → 9 passed / 0 failed。
- `cargo clippy -p fkv_config --all-targets --no-deps -- -D warnings -W clippy::absolute_paths` → 0 警告。
- `cargo +nightly fmt -p fkv_config -- --check` → clean。

### 备注

`cargo check -p fkv_cli`（唯一 `load_from_ini` 调用方）通过（EXIT=0）。

## 复审（2026-11-05 r6）

**通过。** 核实（`fkv_config/src/lib.rs`）：

- 节状态机：:515 `enum Section`，键经 `apply_ini_field(self, section, key, value)` 按节分发（:274 可见）；`fkv_options!` 宏声明 `[节: 解析器]`，`Run/Permanent/Ext/Skip` 分节。
- 内联权重：:594 `parse_store_path`（`path1,path2:w1,w2`）。
- 取整：:497 `pages.ilog2()` 向下取整（不再 power_of_two 报错）。
- 测试 `tests/config.rs`：:194 `ini_section_scope`、:213 `ini_missing_section_rejected`、:223 `ini_store_path_inline_weights`（断言 `store_path_weights==[3,7]`、standby 权重补等权、个数不符/非法权重 `InvalidArgs`）、:254 `ini_data_file_size_rounding`，四条齐备且断言实质覆盖三处修复。
