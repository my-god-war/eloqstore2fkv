# sem-70 CURRENT_TERM 文件内容解析边界三向不一致且未登记：C++ local 不 trim / 空文件按 term 0 静默继续 / 非法内容 IoFail；C++ cloud trim + `stoull` 前缀语义（`5x`→5）；fkv 全后端 trim + 严格全解析 / 空与非法 Corrupted

- 领域：语义 / 盘面格式（CURRENT_TERM 文件 × 畸形内容边界 × Drop / attach
  fencing 行为）
- 优先级：P3（畸形 / 外部篡改输入类；两侧写路径均为规范十进制 +
  tmp/rename/fsync，跨实现交接安全；但同一输入三处行为两两不同，且
  基线只登记了 CURRENT_TERM 的删除面（sem-27）未登记解析面）
- 置信度：高（双侧源码逐行；C++ 侧为源码推演，未运行复现，链路无歧义）

## 现象（fkv `c5e8fbc` / C++ `82b774c`）

C++ 有**两个** CURRENT_TERM 内容解析器，语义不同；fkv 一个解析器全后端
共用，与两个 C++ 版本各有一部分不同：

| 内容 | C++ local（`ParseBranchTerm`，`include/common.h:565-590`） | C++ cloud（`CloudStoreMgr::ReadTermFile`，`src/async_io_manager.cpp:5058-5103`） | fkv（`fkv_io_compio/src/term.rs:25-39` local；`fkv_cloud/src/term.rs:17-28` cloud） |
|---|---|---|---|
| `"5"` | 5 | 5 | 5 |
| `"5\n"` / `" 5"` | 解析失败 | trim 后 5 | trim 后 5 |
| 空文件 / 空串 | **跳过解析按 term 0 继续**（local 读点 `async_io_manager.cpp:3082-3085` 的 `n > 0` 前置） | `stoull` 抛 → `Corrupted` | `Corrupted` |
| `"5x"` | 解析失败 → IoFail | **前缀解析为 5**（`std::stoull` 无 pos 校验） | `Corrupted` |
| `"+5"` | 解析失败 → IoFail | 5（`stoull` 接受正号） | `Corrupted` |
| 溢出 | false | 抛 out_of_range → `Corrupted` | `Corrupted` |

消费点行为差异（同输入下用户可见）：

- **空 CURRENT_TERM 文件**（外部 `touch` / 截断残留）：C++ local
  `DeleteBranchFiles`（`async_io_manager.cpp:3053-3135`）按 `max_term=0`
  照常删 `manifest_<branch>_0` + term 文件，**分支删除成功**；fkv
  `Request::Drop` 先 `read_term` 做 fencing（`fkv_shard/src/dispatch.rs:700-707`），
  `?` 传播 `Corrupted` → **Drop 整体失败**。
- **带尾随换行 / 空白的内容**（如 `echo 5 > CURRENT_TERM_main_0` 手工
  造的文件）：C++ local `ParseBranchTerm` 逐字符判数字，`'\\n'` 非法 →
  `IoFail`，分支删除失败；fkv `trim_ascii()` 后接受为 5，删除继续。
- **`"5x"` / `"+5"`**：C++ cloud `ReadTermFile` 得 5（前缀 / 正号），
  attach fencing 按该值比较；fkv cloud `from_utf8 + trim + parse::<u64>`
  严格 → `Corrupted`，attach 路径报错。

### 已对齐部分（勿重复立项）

- term 文件**命名** `CURRENT_TERM_<branch>_<pg_id>` 逐字符一致（第 2 轮
  已核，`common.h:543-560` ⇔ `fkv_types/src/lib.rs:398-400`）；branch
  名字符集 / 小写归一一致（`common.h:76-130` ⇔ `fkv_types:339-360`）。
- CAS 单调语义（`stored > term → ExpiredTerm`、相等幂等成功）与
  「value 与 etag 同一次 GET」已对齐（`term.rs:50-53` 注释锚定 C++
  `UpsertTermFile`）。
- CURRENT_TERM 的**删除面**（local 只删无 tag manifest、cloud 连归档）
  已由 sem-27 登记（COMPAT_BASELINE「`DeleteBranch` 的归档删除面」）。
- NotFound：两侧均按「无 term 文件」处理（C++ `{0,"",NotFound}` /
  fkv `Ok(None)`）。

### 未登记部分

- 内容解析边界（trim / 空 / 前缀 / 正号）三向不一致，基线零条目
  （grep `CURRENT_TERM` / `ParseBranchTerm` / `term 文件` 只命中 sem-27
  删除面条目）。
- fkv 单解析器全后端共用本身是合理收敛（C++ 两个解析器互相不一致
  属上游自身缺陷面），但与两侧各自的差异点需要差分口径：外部篡改 /
  手工构造的 term 文件内容不判 fkv 错误（C++ 任一侧的接受行为均非
  对齐目标；fkv 的拒绝 + 明确错误码是期望行为）。

## 可达性

- 双侧写路径均为规范十进制（C++ `TermToString` = `std::to_string` 无
  换行；fkv `itoa::Buffer` 无换行）+ tmp/rename/fsync（local），torn
  write 不可达；跨实现交接（C++ 写 fkv 读 / 反向）内容恒规范，安全。
- 触达需外部篡改 / 手工构造：空文件、`echo` 造的换行尾、编辑器残留
  前缀空白。此时三处行为两两不同（见上表）。

## 影响

不改合法输入下的任何盘面 / 命名 / 数据；只影响畸形 term 文件输入下
Drop / attach fencing 的成败与错误码（IoFail vs Corrupted vs 按 0 继续）。
对「对齐 C++ 全量」的影响是差分口径缺口：畸形输入用例没有一致期望。

## 建议处置

登记型：COMPAT_BASELINE 补「CURRENT_TERM 内容解析边界」条目（三向
矩阵 + 差分口径：畸形内容不判 fkv 错误、fkv 严格拒绝为期望行为；
C++ local「空文件按 term 0 继续」与 cloud「前缀解析」均非对齐目标）。
可选加固：fkv 对空文件回退 `Ok(None)`（对齐 C++ local 的宽容、使被
截断的 term 文件不阻塞 Drop）——但会让「空」与「缺失」同义，与 cloud
C++（空 → Corrupted）相反，建议保持现状仅登记。锁定测试：fkv 侧可补
`term.rs::parse` 单测（空 / 空白 / `5x` / `+5` / 溢出五形态），当前
该函数仅经集成路径间接覆盖。

## 实现记录（2026-11-07）

- 处置：登记型（保持 fkv 严格拒绝为期望行为，不向任一 C++ 侧对齐），
  fkv commit `0b87909`。
  - COMPAT_BASELINE 增补「CURRENT_TERM 文件内容解析边界」条目（三向矩阵 +
    差分口径 + 消费点差异）+ 差异总表行。
  - 锁定单测：`fkv_io_compio/src/term.rs` 内联 `mod tests`——
    `parse_accepts_canonical_and_trimmed_decimal`（`5` / ` 5\n` /
    u64::MAX）与 `parse_rejects_malformed_content`（空 / 纯空白 / `5x` /
    `+5` / 溢出五形态，逐个断言精确 `Corrupted`）；该函数此前仅经集成
    路径间接覆盖。
  - 验证：`fkv_io_compio` 全套绿（含 hardening 155 例）；`0b87909` 复跑。

## 复审记录（2026-11-07，第 32 轮语义域）

- 判定：**达标**，移入 reviewed/。term.rs 单测 2 条（接受 3 形态 / 拒绝 5
  形态精确 Corrupted）在位且绿；基线「CURRENT_TERM 文件内容解析边界」条目
  与总表行一致；fkv_io_compio 套件全绿（串行）。附注：该实现的溢出断言
  超宽一格引入 fmt 回归，同轮 struct-44 处置（`1b064a4`），不影响本条
  语义结论。
