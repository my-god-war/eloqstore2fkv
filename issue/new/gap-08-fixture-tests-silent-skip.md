# gap-08 oracle 语料缺失时测试静默通过（P1）

## 证据（2026-11-05）

- `crate/compat/fkv_compat/tests/page_oracle.rs`、`manifest_oracle.rs`、
  `compress_oracle.rs` 在语料目录 / 文件缺失时只 `eprintln!` 然后 `return`。
- 因此 CI 绿灯不能证明 oracle 真跑过；LFS 未拉取、语料未生成都表现为 pass。

## 目标

1. 缺语料时必须 `panic!` / `assert!`，或显式要求环境变量
   `FKV_REQUIRE_ORACLE_FIXTURES=1` 时失败；默认本地可 skip，CI 必须 fail。
2. `oracle.yml` 的 compat-rust / fixture-diff 作业设置强制变量。
3. 同步修正 `fixtures/README.md` 中指向 fkv 仓旧路径的说明。

## 验收

- 删除一个 `payload.bin` 后，CI 兼容测试必须红；
- 本地无 LFS 时可看到明确的 “fixtures missing, set FKV_REQUIRE_ORACLE_FIXTURES=1” 提示；
- 文档路径与实际测试目录一致。
