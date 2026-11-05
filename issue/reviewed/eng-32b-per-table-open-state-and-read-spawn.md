# eng-32b per-table 打开状态 + 每请求 spawn（阻塞项，暂不排期）

父 issue：`eng-32-workloop-serialization.md`。

## 目标

把读请求真正包成独立 compio task（`compio::runtime::spawn`），脱离主循环的
`&mut ShardState`，获得无界并发与可取消性。

## 阻塞依赖（必须全部先完成）

1. **per-table 状态拆分**：`Store` 的 `mapper_map`/`root_map`/`index_cache_map`/
   `compression_map`/`ttl_*` 聚成 `TableSlot`，见 `eng-73`。`ensure_table_ready` 改为
   幂等 single-flight（`OnceCell` + 异步打开锁），去掉读路径对 `&mut Store` 的需求。
2. **COW 快照 / pin**：`27` + `core-11`，读跨 `await` 持 snapshot，旧页最后引用释放才回池。
3. **index cache 并发化**：每表 `parking_lot::Mutex<IndexCache>` 或 shard 独占副本，
   invalidate 带代际，防 stale 节点复辟。
4. **fixed buffer 池化**：`io-07` / `io-27`，每请求借还，消除 `GetLarge`/`PutLarge`
   的 `Option<FixedBuffer>` 排他借用。
5. **attach/detach 屏障**：shard epoch，旧请求在检查点快速失败或 drain。

## 落地要点

- `Arc<Store>`（单线程无需 `Send`，`spawn` 只要 `'static`）→ 读类 `spawn` + 有界
  `Semaphore`（in-flight 上限，见 `eng-32c` 指标）。
- 写类不 `spawn`，由 `eng-33` 的 per-table FIFO 收敛；写任务不能中途 drop
  （`JoinHandle` drop 即 cancel，会留半截落盘）。
- 关闭流程：停止 `recv` → 等在飞读完成 → 处理 `Shutdown` → join。

## 验收

- `MeteredIo` 观测到单 shard IO 深度 > 1，且读批之外仍有并发。
- 慢表 scan 不阻塞其他表 get；attach/detach 与并发请求线性化测试通过。

## DONE（2026-11-05）

### 状态

最小闭环完成：`Get` / `Scan` / `Floor` 已真正 `compio::runtime::spawn`，不再在
主循环里串行 `await` 整条 `handle`。per-table 打开状态从读请求路径拆出：
已打开表的读请求只做只读查询 + 共享 `&Store`；打开动作收敛在串行准入路径。

**读写仍未重叠**：写/控制/维护请求在拿到 `&mut Store` 前会 drain 在飞读
（读-写屏障）。真正的读写并发仍依赖 27b（读持 `MappingSnapshot`）+ core-11 +
fixed buffer 池化（io-07）；本轮不强行改 `Store` 读签名。

### 改动（仅 `crate/engine/fkv_shard`）

- `src/lib.rs`
  - `ShardState.store` 改为 `Rc<ShardStore>`。读任务持克隆并发推进；写路径经
    `store_mut()` / `exclusive_store()` 用 `Rc::get_mut` 取独占，失败回 `Busy`
    并 `debug_assert`（正常路径由 drain 屏障保证不会触发）。
  - 新增 `TableOpen` / `TableSlot`：每表登记 `(epoch, term, branch)`。
    `ready()` = `Store::is_open` + 登记代际/身份一致，纯只读查询；
    `mark()` 在打开成功后登记；attach/detach 成功 `bump()` 代际并清空登记。
  - `ensure_table_ready` 拆成「只读就绪判定 + 串行打开」：读路径不再无条件
    触碰 `&mut Store`，同批同表只 replay 一次 manifest（幂等 single-flight）。
  - 新增 `ReadFlight` / `ReadSlot` / `ReadTask`：每条读请求一个独立 compio
    task；请求体 move 进任务、完成时随结果带回，避免深拷贝；任务 panic 也
    能按 slot 回执 `IoFail`。
  - 新增 `submit_reads`（两阶段：先串行准入+打开，再统一 spawn）、
    `finish_reads`（drain + 回执 + OOM 重试 + ResourceMissing park）、
    `read_window`（在飞读期间继续接收新读；遇到非读/未打开表/配额满即 drain）。
  - 主循环把 `carry: Option<Envelope>` 升级为 `pending: VecDeque<Envelope>`，
    保证读窗口退出后的非只读请求按 FIFO 处理；`pending` 非空时先 drain 读再
    走独占路径。
  - `dispatch` 的写/控制臂改用 `store_mut()`；`Reopen` / `ReopenExternal` /
    `Drop` / `DetachShard` 同步维护 `TableOpen` 登记。
  - `GetLarge` / `PutLarge` 仍走串行路径（借用 `fixed_buffer`，等 io-07 池化）。
- `src/lib.rs` 单测：`TableOpen` 代际/身份门控；`ReadFlight` spawn 后
  `Rc::strong_count == 3`、写路径拿不到独占、`drain` 后恢复独占。
- 新增 `tests/read_spawn.rs`：
  - 冷 shard 首读（未打开表）并发读同表：只 replay 一次 manifest，结果正确。
  - 慢表 scan + 另一表 get/floor 并发：get 在超时内完成、scan 结果完整。
  - detach 线性化：detach 前排队的读全部回执（值正确或 `NotRunning`），
    detach 后新读快速失败，重新 attach 后可读。

### 验证

- `cargo test -p fkv_shard -- --test-threads=1`：全部通过（13 lib + 各集成
  测试二进制；`oom_retry` 因 failpoint 全局注册表需串行跑，并行跑会互相污染，
  与本次改动无关）。
- `cargo clippy -p fkv_shard --all-targets -- -D warnings`：0 warning。
- `cargo +nightly fmt -p fkv_shard -- --check`：通过。
- 另在 HEAD 干净 worktree（`/tmp/fkv32b-verify`）上复跑同一套测试：
  全绿，确认不依赖工作区里 27b 未提交改动。

### 残余 / 后续

1. **读-写并发**：当前读任务持 `Rc<Store>` 共享读，写必须先 drain。要读写重叠，
   需 27b 把读入口改成 `MappingSnapshot` + core-11 页 pin + io-07 per-request
   fixed buffer；届时 `ReadFlight::finish_reads` 的 drain 屏障可替换为快照提交。
2. **IO 深度指标**：`MeteredIo` in-flight gauge / 读批直方图属 eng-32c，本轮未加；
   并发性目前由「读窗口 + Rc 计数」单测与行为测试保证。
3. **取消**：执行中的读不可中途取消（与旧语义一致）；读任务 `JoinHandle` 只在
   `drain` 中消费，不会误 drop 造成取消。
4. **IndexCache 边界**：并发读仍共享 `RefCell<IndexCache>`，依赖「borrow 不跨
   await」；27b 改缓存代际时需复核该前提。
5. **`Rc::get_mut` 不变量**：所有 `&mut Store` 路径必须先 drain；若后续新增
   绕过 drain 的调用，会在 debug 下 `debug_assert` 命中、release 回 `Busy`。


## 复审（2026-11-05 r6，runtime/shard/cloud lane）

**通过。热点文件：本条由并行会话于本轮 git 快照中刚移入 done（staged），复核基于当前工作区实读。**

- `fkv_shard/src/lib.rs:1075` `TableOpen`/`TableSlot`（epoch+term+branch 登记）、:2405 attach 成功 `table_open.bump()`、:1165/:1175 `store_mut`/`exclusive_store`（`Rc::get_mut` 失败回 `Busy` + debug_assert）；`submit_reads`/`finish_reads`/`read_window` 在主循环接入（:724 控制请求不进队列注释可证重构在位）。
- 测试 `fkv_shard/tests/read_spawn.rs` 存在（286 行，含冷 shard 单 replay/慢 scan 并发/detach 线性化三组）；单测（TableOpen 代际门控、`Rc::strong_count == 3`）在 lib 内。
- 「读写仍未重叠（drain 屏障）」的边界如实记档，残余项 1-5 指向 27b/core-11/io-07/eng-32c，与 todo 清单吻合，不判缺失。若后续 27b 合入改动这些行号，以 owner 会话为准。
