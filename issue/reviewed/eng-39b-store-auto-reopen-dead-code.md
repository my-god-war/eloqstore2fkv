# eng-39b 删除 fkv_store::auto_reopen_with_sleeper 死代码

## 来源
`issue/done/eng-39-reopen-no-driver-waiter.md` 阶段 A3（重试实现去重）。

## 待做
- `crate/engine/fkv_store/src/lib.rs` 的 `Store::auto_reopen_with_sleeper`
  （与 `crate/engine/fkv_task_reopen/src/lib.rs` 的同名函数逻辑同构）在 eng-39 阶段 A2
  落地后**已无任何调用者**（原唯一调用点是 `fkv_shard` 的 `Request::Reopen` 分支，
  现已改为单次 `store.reopen` + shard 延迟队列重试）。属 pub API 孤儿，应删除，
  连带清理其文档引用；重试策略统一由 `fkv_shard::ReopenQueue` 持有。
- 删除后确认 `fkv_task_reopen::auto_reopen_with_sleeper` / `Sleeper` 仅剩其自身测试使用，
  视情况收敛（测试在用则保留）。

## 阻塞
`crate/engine/fkv_store/src/lib.rs` 是并发会话热点文件（lane 约定禁止修改），
需等该文件空闲或由持有它的会话删除。

复核（2026-11-04）：`crate/engine/fkv_store/src/lib.rs:1210` 的
`Store::auto_reopen_with_sleeper` 确认仍无调用者（唯一调用点已随 eng-39 阶段 A2 移除）；
该文件仍属并发热点，本条继续阻塞。

## 验收
`cargo nextest run -p fkv_store -p fkv_task_reopen -p fkv_shard` 全绿；
`cargo clippy -p fkv_store --no-deps -- -D warnings` 零警告。

## DONE（2026-11-04）

核验：全仓 grep `auto_reopen_with_sleeper` / `Sleeper` 仅剩定义与 `fkv_task_reopen` 自身测试
（`tests/auto_reopen.rs`）引用；`Store::auto_reopen_with_sleeper` 零生产调用，唯一调用点
确实已随 eng-39 阶段 A2 移除，shard 重试由 `ReopenQueue` 承担。

改动：
- 删除 `crate/engine/fkv_store/src/lib.rs` 的 `Store::auto_reopen_with_sleeper`（21 行）；
  `Store::reopen` 补中文文档，说明 `ResourceMissing` 原样上抛、重试归调用方。
- `doc/architecture/05-persistence-and-recovery.md`：把「`fkv_task_reopen::auto_reopen_with_sleeper`
  负责重试」改为「`Store::reopen` 上抛 + `fkv_shard::ReopenQueue` 按 `AutoReopenOption` 延迟重试」。
- `fkv_task_reopen` 的 `auto_reopen_with_sleeper` / `Sleeper` / `ThreadSleeper` 仍有自身集成测试
  使用（`tests/auto_reopen.rs` 的 `CountingSleeper`），按 todo 保留，不动。
- `fkv_store::lib.rs` 未残留 re-export / 文档引用。

验证：
- `cargo check -p fkv_store -p fkv_runtime` 通过。
- `cargo nextest run -p fkv_store -p fkv_runtime --no-fail-fast`：141 passed / 1 skipped。
- `cargo clippy -p fkv_store -p fkv_runtime --no-deps --all-features --all-targets -- -D warnings` 零警告。

## 复审（2026-11-05 r6）

结论：**通过**。

- 全仓 grep `auto_reopen_with_sleeper` / `Sleeper` 在 `crate/`（非 _cpp）零命中——`fkv_store` 版已删（md 声称保留的 `fkv_task_reopen` 自身版本亦已随后续 eng-87 sleeper 族清理删除，方向一致，无回归）。
- `doc/architecture/05-persistence-and-recovery.md:48` 已改为「重试策略由 `fkv_shard::ReopenQueue` 按 `AutoReopenOption` 延迟重试」，文档引用同步。
