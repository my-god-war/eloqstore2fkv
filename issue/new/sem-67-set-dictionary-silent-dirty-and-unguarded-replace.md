# sem-67 `Store::set_dictionary` 注入字典不置 dirty：manifest 已建的表 reopen 丢字典，Dictionary 压缩页永久 Corrupted；附属无守卫替换活字典可静默错值

- 领域：语义 / 盘面格式（字典生命周期 × manifest 持久化不变式 + 单侧
  新增公开 API 的误用面，文档承诺与行为不符）
- 优先级：P2（文档化公开 API，按其文档语义使用即触发「已确认写成功的值
  在 reopen 后永久不可读」；替换面读路径可静默返回错值，无任何错误）
- 置信度：高（fkv `d25e47c` 双实验复现 + 逐行代码链路 + C++ `82b774c`
  源码对照；缺陷同样存在于审查起点 `80f887d`，期间历史第 5 次重写不改内容）

## 现象

### 声明 1（主缺陷）：注入字典不标 dirty → manifest 增量分支丢字典

链路（fkv `d25e47c` 行号）：

1. `fkv_store/src/state.rs:279-291` `Store::set_dictionary`：
   `compression.take()` → `load_dictionary(dict)` → 放回，**不置 dirty**；
2. `fkv_compress/src/lib.rs:203-213` `load_dictionary` 只替换
   `dict` / `prepared`；`dirty = true` 只在 `build_dictionary`（`:248`）；
3. `fkv_store/src/manifest.rs:243-258` flush 分支条件
   `dict_dirty || 大小超限 → 全量 snapshot；否则增量 record`。全量
   snapshot 含 `dict_bytes`（`state.rs:556-563` `SnapshotArg::dict_bytes`），
   增量 record（mapping/segment delta + finalize）**不含字典**。该处注释
   即不变式原文：「字典刚训练出来时必须写 snapshot 持久化 dict_bytes，
   否则日志增量不含字典，reopen 后无法解压 Dictionary 页（对齐 C++
   `dict_dirty`）」。

后果：`manifest_started` 的表（已有首批写或经 reopen）调用
`set_dictionary` 后：写批的 Dictionary 压缩页正常落盘、内存态可读；
flush 因 `dict_dirty == false` 走增量 record → dict_bytes 不入 manifest；
crash / reopen 后 `replay.dict_bytes` 为空（`manifest.rs:662`）不装载
字典 → `has_dictionary == false`，读这些页
`decompress_value(Dictionary, dict=None)` → `Err(Corrupted)`——已确认
写入成功的值永久不可读（除非用户恰好在进程外保留同样字节再次注入）。

**实验一**（`d25e47c`，`fkv_store/tests/compression.rs` 临时用例后删除）：
CompioStore + `with_compression_level(3)`，先 put 建首 manifest（无字典）→
`set_dictionary(1024×'d')` → put 600×'d'（Dictionary 压缩）→ 内存态
read 等值 Ok → drop 重开 `reopen(&tbl)` → **`has_dictionary=false,
read=Err(Corrupted)`**。

既有测试 `dictionary_compression_end_to_end_reopen` 全绿的巧合：其
`set_dictionary` 发生在首批写**之前**，首个 manifest 本来就是全量
snapshot（`first_manifest`），dict_bytes 无条件包含——恰好绕过缺陷窗口，
掩盖了「manifest 已建后注入」的主路径。

### 声明 2（附属）：无守卫替换活字典 → 静默错值

`set_dictionary` 对已有字典的表无条件替换（`take()` + `load`，无
`has_dictionary` 检查、无警告）。旧 Dictionary 压缩页此后用新字典解压：
zstd 帧不内嵌字典、也不校验字典身份，解码结果数据依赖——自匹配值可能
碰巧仍正确；依赖字典内容的值**静默返回错值**。**实验二**（`d25e47c`）：
随机字典 A（1024B）、value = `A[..600]`，替换为 `A⊕0xFF` 后 read →
`Ok(错字节)`（`equal_to_original=false, err=None`），get / read / scan /
floor 全部读路径同样失真，无任何错误信号。

### C++ 对照（`82b774c`）

- C++ `DictCompression`（`compression.cpp:154-252`）**没有对外注入 API**：
  `LoadDictionary`（`:164`）只在 replayer 内部调用（字典来自 manifest，
  无需再持久化，故不置 dirty 语义自洽）；`SampleAndBuildDictionaryIfNeeded`
  （`:196`）有 `HasDictionary()` 门，一旦有字典永不重建。C++ 字典生命
  周期是封闭的（replay 装载 / 一次性训练），不存在「运行中注入 / 换字典」
  这个状态。
- fkv 的 `set_dictionary` 是单侧新增公开 API（`pub` + 端到端测试），复用
  replay 级的 `load_dictionary`（不置 dirty）——把「装载」与「无需再
  持久化」两个语义绑在一个不标脏的入口上，违反自家 manifest 不变式。
- 文档失真同型先例：`0c1ac4f` commit message 对拍声明（sem-66）、bench-01
  验收条款。本条更重：`set_dictionary` doc 明文承诺「manifest 会持久化
  字典字节，reopen 时自动恢复」——对 manifest 已建的表（最常见用法：
  存量表上启用预训练字典）不成立，是文档承诺 × 数据损失双重缺口。

## 影响

- 盘面 / 数据：注入后至下一次大小触发的全量 snapshot 之间写入的全部
  Dictionary 压缩值 reopen 后 `Corrupted`（窗口内 crash 即永久损失）；
  替换面即刻静默错值。
- 触发面：`fkv_store::Store` 公开方法 + 文档明示用法；`fkv_sdk` 经
  `inner()` 不可达（`&Store` 不可变），但 `fkv_store` 本身是公开 crate。
- 未启用压缩时注入（仅用于解码历史页）无新写压缩页，不触发声明 1；
  声明 2 与压缩开关无关。

## 验收方式

1. `set_dictionary` 成功装载后置 dirty（在 state.rs 装载点显式标脏，或给
   `DictCompression` 补置脏入口）。**不得**改 `load_dictionary` 本身置
   dirty：manifest replay（`manifest.rs:666-672`）复用它装载来自 manifest
   的字典，置 dirty 会让每次 reopen 后首个 flush 被迫全量 snapshot
   （C++ `LoadDictionary` 同样不置 dirty）。锁定测试：manifest 已建 →
   set_dictionary → put → drop → reopen → `has_dictionary` 为真且 read
   等值（现有端到端测试补一个「先写后注入」变体即可）。
2. 替换守卫：对 `has_dictionary()` 已真的表返回 `Err`（C++ 封闭生命
   周期口径），或最低限度文档声明危险；锁定测试：替换尝试报错、旧值
   仍可读。
3. `state.rs` `set_dictionary` doc 的持久化承诺与修复后行为对齐。
4. 基线登记：COMPAT_BASELINE 增补「字典注入 API 为 fkv 单侧新增 + 修复
   后的持久化 / 拒绝替换语义」（C++ 无对应物，输入契约差异登记，
   sem-62 同型）。

## 处置记录

- 2026-11-07 第 29 轮语义 / 盘面格式收敛判定轮立项（审查对象 fkv
  `80f887d` 起的字典 / 压缩生命周期面；期间 fkv 历史第 5 次重写为
  init `d25e47c`，缺陷在 `d25e47c` 逐行核对 + 双实验复现确认，内容
  未随重写变化）。
