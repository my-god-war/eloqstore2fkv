# sem-69 配置运行时校验 / INI 零值强制转换边界与 cloud 改写集差异未登记：restart_interval=0 与 data_page_size=0 在 C++ 可启动而 fkv 拒绝；data_file_size=0 两侧回退布局相反（8MB vs 单页）；cloud fd_limit 钳制 / local_space_limit 下限抬升无对应；布局三旋钮 Ext 键超出已登记扩展键清单

- 领域：语义 / 盘面格式（配置运行时校验 × 启动判定 × data 文件布局派生）
- 优先级：P3（均为病态配置 / 手工构造输入，正常 INI 不触达；但 sem-06
  （INI 解析边界）只覆盖「解析失败」形态，本条覆盖「合法解析出的 0 经
  字段级强制转换 / validate 拒绝」形态，差分口径缺失；其中 (c) 是双侧
  均可启动但**盘面布局相反**的静默分叉）
- 置信度：高（双侧源码逐行 + fkv-mut 实证探针 4/4；C++ 侧为源码推演，
  本机 darwin 无法构建 eloqstore，未运行复现，但链路无歧义）

## 现象（fkv `c5e8fbc` / C++ `82b774c`）

C++ 的启动校验分两层：`KvOptions::LoadFromIni`（`src/kv_options.cpp:86-386`，
含字段级 0 值回退）与 `EloqStore::ValidateOptions`（`src/eloq_store.cpp:118-451`，
拒绝 + 改写）。fkv 对应 `load_from_ini`（`crate/foundation/fkv_config/src/lib.rs:742-833`）
+ `validate`（`:488-730`）。逐条对照后，拒绝集主体 1:1（num_threads /
max_inflight_write / max_global_request_batch / 对齐族 / overflow_pointers
（上界两侧同为 128）/ write_buffer 对 / max_write_batch_pages / standby
master 地址与路径族——后者已由基线 sem-17 条目登记，含 `local` 大小写
不敏感与 cld-58 根数量解耦）。未登记差异集中在**零值边界与 cloud 改写集**：

### (a) `restart_interval = 0`：C++ 接受（退化格式），fkv 启动失败

- C++：`kv_options.cpp:103-112` 无范围检查；`ValidateOptions` 也不查。
  `data_page_restart_interval = 0` 下 `DataPageBuilder::NeedRestart()`
  （`include/storage/data_page_builder.h:56` `counter_ >= interval`）恒真
  ——每条 entry 都是 restart 点的合法退化格式，正常读写。
- fkv：`lib.rs:501-502` `data_page_restart_interval == 0 || index_page_restart_interval == 0`
  → `InvalidArgs`。C++ 单独校验同名字段处为零处。
- 同一份 INI：C++ 可启动（fkv 可读其产物——restart 点页内自描述），
  fkv 启动失败。fkv 无「C++ 靠写侧不变量」之外的对齐注释。

### (b) `data_page_size = 0` / `= 0KB`：C++ 静默回 4096 启动，fkv 启动失败

- C++：`kv_options.cpp:334-340` 字段级回退 `data_page_size = (parsed_size > 0)
  ? parsed_size : (1 << 12)`——显式 0 / 0KB 被替换为 4096 并正常启动。
- fkv：`parse_size("0KB")` 返回 `Ok(0)`（`lib.rs:935-959`，0 是规范十进制），
  字段存 0，`validate`（`lib.rs:506`）拒绝。sem-06 条目第 5 项只覆盖
  「解析失败静默回 0」形态（`abc`、`8abc` 前缀），不覆盖「合法 0 被
  C++ 字段级替换为默认值」形态。

### (c) `data_file_size = 0`：双侧均可启动但回退布局相反（盘面分叉）

- C++：`kv_options.cpp:341-346` `(parsed_size > 0) ? parsed_size : (8 * MB)`
  → 8MB；4KB 页下 `pages_per_file_shift = 11`。
- fkv：`lib.rs:787-789` 接收 `Some(0)`，`:816-819` `pages == 0` → 回退
  **单页**（shift 0）+ warn。
- 同一份 INI 两侧布局完全不同（每文件页数 2048 vs 1、file_id 分配节奏、
  manifest 段形状），且**不报错只告警**——比 (a)(b) 的启动失败更隐蔽，
  差分器按「同 INI 双侧启动」跑会得到不可对拍的盘面。基线仅有
  `data_file_size > u32::MAX` 截断条目（COMPAT_BASELINE「INI 解析边界」
  节），零值形态未覆盖。

### (d) cloud 模式两处 C++ 改写在 fkv 无对应（模型替代，未登记）

- C++ `ValidateOptions` cloud 分支：`local_space_limit` 低于单数据文件时
  抬到 `data_file_bytes`（`eloq_store.cpp:318-326`）；`fd_limit` 按
  `max_cached_files = local_space_limit / data_file_bytes` 钳制
  （`:328-338`）。
- fkv `validate`（`lib.rs:657-693`）无这两步——本地 cache 预算改为全局
  字节记账（`fkv_runtime/src/cloud_env.rs:55-58`、`backend.rs:87-89`
  `CacheGroup::new(local_space_limit)`），fd 上界独立于空间预算
  （`startup.rs:64-72` 只做 `store_fd_limit / num_threads` 均分）。
  资源保护目标由另一机制达成（字节驱逐），但「同一 INI 在 C++ 会被
  改写 fd_limit / local_space_limit、在 fkv 原值生效」这一配置语义
  差异无基线条目（grep `fd_limit` / `local_space_limit` 零命中）。

### (e) 布局三旋钮的 Ext 键超出已登记扩展键清单

- 基线 sem-06 条目第 8 项只把 `enable_compression` /
  `zstd_compression_level` 登记为 fkv 单侧扩展键（C++ INI 不解析）。
- `segment_size` / `segments_per_file_shift` / `pages_per_file_shift`
  同为 C++ INI 不解析（`kv_options.cpp` 全文无对应键，C++ 只能代码 /
  C API 设置）而 fkv `[Ext]` 两节均可（`lib.rs:345,348,370`；实测经
  INI 全部生效）。这三个键直接决定**盘面布局**（段大小 / 每段文件段数 /
  每数据文件页数）：同一 INI 在 C++ 按默认（256KB / 7 / 11@4KB）启动、
  在 fkv 按显式值生效——两侧写出不可互读的布局。`enable_local_standby`
  （C++ 字段存在、INI 不读）同属此类。其余 Ext 键（`auto_reopen_*` /
  `max_processing_time_us` / `yield_budget_us` / `request_*_us` /
  `index_cache_*` / `segment_file_amplify_factor`）为 fkv 自有运行时
  旋钮、无 C++ INI 对应概念，可不登记；`chunk_size` /
  `gc_global_mem_size_per_shard` / `pinned_tail_scratch_slots` /
  `segment_compact_yield_every` 已在「解析但不生效」条目内。

## 可达性（fkv-mut 一次性探针，2026-11-07，跑后已删除）

- `data_page_restart_interval = 0`（[run] 节）→ `validate` 返回
  `Err(InvalidArgs)`；
- `data_page_size = 0KB` → 字段为 0、`validate` `Err(InvalidArgs)`；
- `data_file_size = 0` → `pages_per_file_shift == 0`（C++ 推演为 11）；
- `segment_size = 64KB` + `segments_per_file_shift = 5` +
  `pages_per_file_shift = 12` → 三值全部经 INI 生效。
  探针文件 `r31_probe_validate_boundary.rs` 4/4 通过后删除，
  `git -C /tmp/r29/fkv-mut status` 回到干净。

## 影响

- (a)(b)：启动判定差分（同 INI C++ 起 / fkv 拒）——sem-06 同类，迁移
  口径需补「显式 0 值」形态；(c)：静默盘面布局分叉；(d)：配置改写语义
  差异（资源模型类，正确性无影响）；(e)：扩展键清单不完整，布局关键
  键缺迁移警示。全部不改变正常配置下的行为与盘面。

## 建议处置

登记型为主（COMPAT_BASELINE「INI 解析边界 / 配置校验」节补条）：
- (a)(b)(e) 按 sem-06 第 5/8 项口径登记差分（「C++ 接受 / 静默回退的
  形态，迁移前清洗，不判 fkv 错误」+ 扩展键清单补全三布局键）；
- (c) 可选择对齐（fkv 的 `pages == 0` 分支回退 `8MB` 派生 shift 而非
  单页）或登记有意差异（当前 warn 已提示）；对齐更符合
  `data_file_size > u32::MAX` 条目已表达的「布局派生向 C++ 收敛」方向；
- (d) 登记为资源模型替代（字节预算 CacheGroup），注明 C++ 钳制 /
  抬升两步的差分口径。锁定测试：fkv 侧可为 (c) 增「零值回退」用例
  （若选择对齐），其余以基线条目承载。

## 实现记录（2026-11-07）

- 处置：(c) 对齐 + (a)(b)(d)(e) 登记，fkv commit `0b87909`（基于并行会话
  的 `6f79f44` 页缓存提交之后）。
  - (c) 对齐：`fkv_config/src/lib.rs` 的 `data_file_size` 派生点在
    `size == 0` 时取 `8 * MB`（对齐 C++ `kv_options.cpp` 字段级回退），
    两侧同 INI 派生同一布局（4KB 页 → shift 11）；`0 < size < page_size`
    的单页回退分支不变（本条范围外）。锁定测试
    `ini_data_file_size_zero_falls_back_to_8mb`（变异：还原旧单页回退 →
    shift 断言 11 != 0 红，复绿确认）。
  - (a)(b)(d)(e) 登记：COMPAT_BASELINE「INI 解析边界」增补第 10-12 项
    （零值边界 / cloud 改写集 / 扩展键清单补全）+ 差异总表行。
  - 验证：`fkv_config + fkv_io_compio` 155 passed（含新用例）；
    `c5e8fbc` 与合并 `6f79f44` 后的 `0b87909` 双基线复跑一致。

## 复审记录（2026-11-07，第 32 轮语义域）

- 判定：**达标**，移入 reviewed/。`fkv_config/src/lib.rs` 的 `0 → 8*MB`
  对齐与 C++ `kv_options.cpp` 逐字一致；锁定测试断言 shift 11 在位且绿；
  基线第 10-12 项 + 差异总表行与实现一致；fkv_config 套件全绿（串行）。
