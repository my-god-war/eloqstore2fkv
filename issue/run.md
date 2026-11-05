开子代理审查 `./new` 下的意见（不一定对，必须逐条核对源码）。

规则：

- 每个子代理一次只处理一个 issue，禁止批量移动。
- 有道理：拆分到 `./todo`，开发，跑测试，完成后移到 `./done`。
- 不合理：拆分到 `./reject`，写清拒绝理由与证据。
- 甄别一个、开发一个、跑测试、修问题，小步迭代。
- 子代理并发上限 5。

验证命令：

```bash
# fkv 主仓
cd ../fkv && cargo nextest run --workspace --all-features

# 本仓兼容测试（需要 ../fkv）
cd .. && cargo nextest run -p fkv_compat
```

代码规范：`../fkv/.agents/skills/rust_review/SKILL.md`。
