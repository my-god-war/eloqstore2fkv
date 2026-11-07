# eloqstore2fkv

eloqstore 到 fkv 的迁移与兼容验证仓库。

fkv 主仓保持纯 Rust、独立构建；本仓保留 C++ 参考实现 oracle、双向字节语料、
迁移账本、历史审查记录与验证 CI。

## 内容

- C++ oracle：链接 eloqstore 格式层，生成 page / index / overflow / manifest /
  compress 语料。
- 双向语料：C++ 写 → Rust 读，Rust 写 → C++ 读。
- `fkv_compat`：Rust 侧兼容测试，依赖 fkv 仓的 format crate。
- `fkv_ledger`：扫描 eloqstore、记录迁移账本、生成与校验语料。
- 性能对比：把 fkv 与 C++ 基线放到同一套 workload 下比较。

## 入口

- [验证流程](workflow.md)
- [C++ oracle](oracle.md)
- [迁移账本](ledger.md)
- [性能](performance.md)
- [审查记录](issues.md)
