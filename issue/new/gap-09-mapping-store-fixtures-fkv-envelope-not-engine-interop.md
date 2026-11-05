# gap-09 mapping/store 是 fkv 自定义信封，`status=pass` 不等于引擎级互操作（P2，文档面已消解，仅剩口径分离）

## 来源

对 gap-02 实现的复审（2026-11-05）。本条只保留 HEAD `7c2f018` 之后**仍然成立**的部分，
不复述已被该提交消解的结论。

## HEAD 复核（2026-11-05，`eloqstore2fkv` HEAD `7c2f018`）

**已被消解**（无需再做）：

- `COMPAT_BASELINE:99` 已把旧句「`mapping` / `store` 仍是占位 skipped」改成
  「布局解析一致，**不是**引擎级 store 目录互读。V2/V4（含续写 / 引擎级）仍未…」；
- `fixtures/README.md:39-59` 已显式写明：两个 kind 是**本仓信封**、不能作为
  「引擎级 store 目录互读」证据，并列出布局
  （`mapping` = `MappingSnapshot` 表序列化 + count 信封；`store` = `FST1` 文件清单）。

因此原先最担心的「`status=pass` + CI 全绿被读成互操作结论」这一风险已被文档堵住。

**仍然成立**（本条继续保留的部分）：

- 语料本身确实是信封而非盘面格式，`mapping/empty` 的 1 字节 payload
  （`xxd -p` → `00`）就是 fkv 自己加的 `count(varint32)` 前缀
  （`cpp/oracle/src/main.cpp:320-322` 注释自述「count 前缀让空映射也有非空 payload」）；
- 引擎级目标（Rust `open` C++ 产出的 store 目录并读出期望键值）**仍为 0**，
  `L1` 升级前置（`COMPAT_BASELINE`：`sto-51` 修复 + V2/V4 执行）不受本次改动影响；
- 缺少把「格式层信封用例」与「引擎级交叉读用例」分开的统计口径。

## 证据

```
$ for k in mapping store; do for c in $(ls fixtures/cpp/$k); do \
    printf "%-8s %-8s status=%-6s payload=%s\n" $k $c \
      "$(rg -o '^status=(.*)' -r '$1' fixtures/cpp/$k/$c/meta.txt)" \
      "$(wc -c < fixtures/cpp/$k/$c/payload.bin)"; done; done
mapping  empty    status=pass   payload=1        # xxd -p → 00，仅 count 前缀
mapping  multi    status=pass   payload=12
mapping  single   status=pass   payload=3
store    empty    status=pass   payload=5
store    multi    status=pass   payload=9047
store    single   status=pass   payload=4112
```

消费侧测试（断言的是上述信封语义，本身强度不弱）：
`crate/compat/fkv_compat/tests/mapping_oracle.rs:33-51`、
`crate/compat/fkv_compat/tests/store_oracle.rs:41-118`。

## 建议（缩小后）

1. 在 `meta.txt` 增加一行级别标记（如 `level=format-envelope`），与将来的
   `level=engine` 用例分开，使「格式层信封语料数」与「引擎级交叉读用例数」
   在账本 / 报告里不混算。
2. `fixtures/README.md` 的信封说明建议同时回指 `COMPAT_BASELINE:99`，
   保证两处口径一致、改动一处时另一处可被检索到。
3. 引擎级用例继续挂在 gap-02 下，不因本项收尾而关闭。

## 验收

- 存在可机械区分的级别标记，统计口径不再把信封用例计入引擎级覆盖；
- `COMPAT_BASELINE` 与 `fixtures/README.md` 关于「非引擎级」的表述互相可检索到；
- gap-02 保持开启（引擎级 V2/V4 未执行）。
