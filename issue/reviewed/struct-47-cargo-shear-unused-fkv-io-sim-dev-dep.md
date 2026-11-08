# struct-47 `cargo shear` 门禁回归：缓存重构删掉 fkv_task_read 的 sim 侧索引缓存测试后，dev-dep `fkv_io_sim` 成为孤儿（HEAD 门禁红）

- 领域：结构 / 代码质量（未使用依赖 × 门禁）
- 优先级：P3（零行为影响、一行机械修复；但 `cargo shear` / `bun sh/unused.js`
  在 HEAD 确定性红）
- 置信度：高（门禁原样命令实测复现；`grep -rn fkv_io_sim
  crate/engine/fkv_task_read` 只命中 Cargo.toml，无任何 `.rs` 引用）

## 现象（fkv `52650d6`，worktree `/tmp/r29/fkv` 干净检出）

```text
$ cargo shear ; echo $?      # → 1
shear/unused_dependency
  × unused dependency `fkv_io_sim`
    ╭─[crate/engine/fkv_task_read/Cargo.toml:56:1]
 55 │ fkv_io_compio = { path = "../../io/fkv_io_compio" }
 56 │ fkv_io_sim = { path = "../../io/fkv_io_sim" }
    ·      ─────┬────
    ·           ╰── not used in code
```

`bun sh/unused.js` 同源命中（其「未使用依赖」小节即 cargo-shear 透传）：本轮
输出 = 既有基线 2 处 pub 方法误报（`fkv_shard/src/reopen.rs:62 partial_cmp`、
`fkv_io_compio/src/buf.rs:64 as_uninit`）+ **新增 1 处未使用依赖**，退出码 2。

## 根因

- `fkv_task_read` 的 `[dev-dependencies]` 里有 `fkv_io_sim`，但该 crate 现存
  `src/lib.rs` / `src/page_buf.rs` / `tests/read.rs` / `tests/read_pool.rs`
  全部只经 `fkv_io_mem`（`MemStore`）——`fkv_io_sim` 无任何引用。
- 该孤儿来自并行会话的缓存重构（`f6f08c8` = `refactor(cache): 删除 index 逻辑页
  缓存`；其主体已并入 `init` `ff63590`）：重构删除 `fkv_task_read` 的
  `IndexRead` / `PageFlight`（`src/flight.rs`）后，原先用 sim 后端驱动的索引
  缓存用例一并删除，dev-dep 失去唯一使用者。
- 全仓其余持有 `fkv_io_sim` 的 crate（`fkv_store`，`tests/sim_store.rs`）仍在使用，
  仅 `fkv_task_read` 被孤立。
- 归属为并行在途面（同 struct-46 / struct-29/38/42 先例）。

## 建议（一行级机械修复）

删除 `crate/engine/fkv_task_read/Cargo.toml` 的
`fkv_io_sim = { path = "../../io/fkv_io_sim" }`（在 `52650d6` 为 `[dev-dependencies]`
下第 56 行；`4d1f11c` 已改成 `[dev-dependencies.fkv_io_sim]` 表，删除对应表即可）。

## 验收方式

```bash
cargo shear                       # exit 0、零输出
bun sh/unused.js                  # exit 2 但仅剩既有 2 处 pub 方法误报
```

## 处置记录

- 2026-11-08 第 35 轮结构域门禁全表扫描发现（fkv `52650d6`）。归属并行缓存重构
  在途面；`4d1f11c`（并行会话下一笔）保留该 dev-dep 未清理，故 main HEAD 同样红。

## 实现记录（2026-11-08，第 35 轮结构域）

- 修复 fkv commit `5b4ecd8`：删除 `fkv_task_read/Cargo.toml` 的
  `[dev-dependencies.fkv_io_sim]` 块（全仓 src/tests 零引用），Cargo.lock -1 行。
- 验证：`cargo shear` → **✓ no issues found**；`fkv_io + fkv_task_read +
  fkv_task_scan` 22/22。
