# eloqstore2fkv

eloqstore → fkv 的迁移与兼容验证仓库。

在线主页与性能对比：<https://my-god-war.github.io/eloqstore2fkv/>

fkv 主仓（Rust 实现）保持独立；本仓保留 C++ 参考实现 oracle、双向字节语料、迁移账本、历史审查记录，以及把 fkv 与 eloqstore 做逐字节 / 性能对比的 CI 与看板。

## 目录

- `cpp/oracle`：链接 eloqstore 格式层 TU 的 C++ oracle，生成 / 校验 page、index、overflow、manifest、compress 语料。
- `cpp/shim`：C++ → Crubit 的简单签名入口。
- `fixtures/`：`cpp/`（C++ 写 → Rust 读）与 `rust/`（Rust 写 → C++ 读）双向语料，`.bin` 走 Git LFS。
- `COMPAT_BASELINE`：锁定的 eloqstore commit 与兼容等级。
- `tool/fkv_ledger`：tree-sitter 扫描 eloqstore、motedb 迁移账本、fixture 生成 / 校验。
- `crate/fkv_compat`：依赖 fkv 仓的 Rust 侧兼容测试（oracle 语料解码、格式金标）。
- `crate/**/*_cpp`：按 fkv 分层建立的 C++ 对照 crate 骨架。
- `issue/`：迁移与审查记录。
- `doc/migration*.md`、`doc/progress.md`、`doc/metrics.md`：迁移方法、账本快照、指标契约。
- `docker/`：oracle 构建环境。
- `web/`：webc 多语言站点（Markdown + YAML，无 JSON 数据层）。
- `todo.md` / `questions.md`：初始迁移设计与待澄清项。

## 依赖关系

本仓的 Rust workspace 通过相对路径依赖 fkv：

```text
../fkv
```

本地要求两个仓库并列：

```text
kv/
├── eloqstore/        # C++ 参考实现源码
├── eloqstore2fkv/    # 本仓
└── fkv/              # Rust 实现
```

CI 会把 fkv checkout 到工作区内的 `fkv/`，再创建 `../fkv` 符号链接，使路径依赖成立。

## 本地验证

```bash
# 只跑 Rust 兼容测试（需要 ../fkv）
cargo nextest run -p fkv_compat

# 拉 eloqstore 并构建 oracle
scripts/fetch_eloqstore.sh
ELOQSTORE_SRC=cpp/eloqstore cpp/oracle/build.sh /tmp/fkv_oracle

# 重新生成双向语料
ELOQSTORE_SRC=cpp/eloqstore bash scripts/gen_fixtures.sh

# 账本
cargo run -p fkv_ledger -- sync
cargo run -p fkv_ledger -- report --out doc/progress.md
```

## CI

- `oracle.yml`：构建 oracle，重生成 `fixtures/cpp` 并检查漂移，跑 `fkv_compat` 与 `fkv_ledger verify`，再生成 `fixtures/rust` 反向读回。
- `web.yml`：构建并发布 webc 多语言站点。

性能对比按 `web/doc/zh/doc/performance.md` 的步骤手动执行；站点不读取 JSON。
