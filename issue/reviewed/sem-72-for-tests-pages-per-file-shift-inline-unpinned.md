# sem-72 `ShardConfig::for_tests` / `RuntimeConfig::for_tests` 内联 `pages_per_file_shift: 18` 未钉住（防漂移机制漏了第三个共享常量）

- 领域：语义 / 测试配置一致性（并行会话 `4d1f11c` 遗留）
- 优先级：P3（当前值正确、无行为漂移；纯潜在维护隐患）
- 置信度：高（逐行核对 `4d1f11c` diff + 源码；常量当前 == 18）

## 现象

`4d1f11c` 把 46 个测试文件里各自复制的 `ShardConfig` / `RuntimeConfig`
字面量收敛为 `for_tests(root)` 单一来源，并新增测试
`for_tests_defaults_match_source_constants`（`fkv_shard/tests/lifecycle.rs:396`）
钉住两个内联常量：

- `overflow_pointers: 16` == `fkv_page::DEFAULT_OVERFLOW_POINTERS`
- `page_cache_bytes: 64 * 1024 * 1024` == `fkv_config::DEFAULT_PAGE_CACHE_BYTES`

但 `for_tests` 里内联的**第三个**共享常量 `pages_per_file_shift` 未被钉住：

- `crate/engine/fkv_shard/src/lifecycle.rs:139` —— `pages_per_file_shift: 18,`
- `crate/engine/fkv_runtime/src/lib.rs:65` —— `pages_per_file_shift: 18,`

`18` 即 `fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT`（`crate/foundation/fkv_types/src/lib.rs:61`）。
收敛前，有 5 个 runtime 测试（`archive_crond` / `auto_reopen` / `cloud_e2e` /
`large_value` / `prewarm`）与该常量**逐字绑定**（字面量写的是
`fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT`），收敛后这些绑定被替换为硬编码 `18`。

## 证据

```text
$ git -C /tmp/r29/fkv show 4d1f11c -- crate/engine/fkv_runtime/tests/prewarm.rs | grep pages_per_file_shift
-    pages_per_file_shift: fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT,
（新版本无 pages_per_file_shift 覆盖，走 for_tests 的硬编码 18）
```

- `fkv_types` 是 `fkv_shard` / `fkv_runtime` 的**正式依赖**
  （`crate/engine/fkv_shard/Cargo.toml:66`、`crate/engine/fkv_runtime/Cargo.toml:47`），
  因此 `pages_per_file_shift: fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT` 完全可引用，
  内联 `18` 是不必要的重复（不同于 `fkv_page` / `fkv_config` 是 dev 依赖、无法进 src）。
- 钉住测试 `for_tests_defaults_match_source_constants` 只断言了上述两个字段，
  未断言 `pages_per_file_shift`。

## 影响

当前 `DEFAULT_PAGES_PER_FILE_SHIFT == 18`，**无任何语义 / 盘面漂移**（本轮语义域
已逐文件核对：所有收敛值与原字面量一致）。风险仅在将来：若该常量被改动（例如为对齐
C++ 而调整每文件页数），`for_tests` 会**静默**保持 18，所有 shard 测试与该 5 个 runtime
测试的每文件页数布局一起偏离常量源——正是 `4d1f11c` 想消除的「默认值漂移」类别，
只是这次漂移发生在常量源与夹具之间。

## 建议

二选一（都无行为变化）：

1. 把两处 `for_tests` 的 `pages_per_file_shift: 18` 改为
   `fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT`（`fkv_types` 已是正式依赖，零成本）；或
2. 在 `for_tests_defaults_match_source_constants` 追加一条断言
   `assert_eq!(cfg.pages_per_file_shift, fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT)`。

方案 1 更彻底（消除重复本身），方案 2 与既有两个字段一致。倾向方案 1。

## 验收方式

```bash
cd /tmp/r29/fkv
cargo nextest run -p fkv_shard --test lifecycle --all-features   # 钉住测试仍绿
cargo nextest run -p fkv_runtime --all-features
```

## 实现记录（2026-11-08）

- 修复 fkv commit **`6d3f11a`**：
  - `fkv_shard/src/lifecycle.rs:139` 与 `fkv_runtime/src/lib.rs:65` 的
    `pages_per_file_shift: 18` 改为 `fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT`
    （`fkv_types` 为二者正式依赖，非 dev 依赖，可引用）；
  - 钉住测试 `for_tests_defaults_match_source_constants`
    （`fkv_shard/tests/lifecycle.rs`）补第三项断言（`pages_per_file_shift` ==
    `fkv_types::DEFAULT_PAGES_PER_FILE_SHIFT`）并更新文档注释口径。
- 验证：`fkv_shard --test lifecycle` + `fkv_runtime` 7/7 passed；fmt/clippy 绿。
  行为零变化（`for_tests` 仅测试支持构造）。

## 复审记录（2026-11-08，第 37 轮结构域）

- 判定：**达标**，移入 reviewed/。`fkv_shard --test lifecycle` 7/7、
  `fkv_runtime` 114 passed / 1 skipped；**变异复核**：把 `for_tests` 的
  `pages_per_file_shift` 改为 19 → 钉住测试确定性变红（`left: 19 / right: 18`），
  还原后复绿；第三项断言的「防回归」口径与测试文档一致。
