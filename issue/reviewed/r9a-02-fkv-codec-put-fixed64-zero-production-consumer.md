# r9a-02 P3：fkv_codec::put_fixed64 零生产消费（唯一消费是本 crate 自测），零死代码纪律残留

## 现象（亲读）

`crate/foundation/fkv_codec/src/lib.rs:27-31`：

```rust
pub fn put_fixed64(dst: &mut [u8], value: u64) -> Option<usize> {
  let out = dst.get_mut(..8)?;
  out.copy_from_slice(&value.to_le_bytes());
  Some(8)
}
```

全仓 `grep -rn --include="*.rs" -w put_fixed64 crate tool web`（排除 `target/`、`*_cpp/`）仅两处命中：

- 定义本体（`:27`）；
- `crate/foundation/fkv_codec/tests/codec.rs:3/:13` —— 本 crate 自测的导入与断言。

**零生产消费**。对应的 `get_fixed64` 有真实消费者（`fkv_page/src/header.rs:88` `validate_checksum`、
`fkv_manifest/src/lib.rs:359` `validate_record`），put 侧没有：Rust 全部 64 位写出点直接
`to_le_bytes()` + `copy_from_slice`（`fkv_page/src/header.rs:82-84` `set_checksum`、
`fkv_manifest/src/lib.rs:304-305` `finalize` 的 checksum 回填），不经过本函数。

同批扫描（scope 全部 pub 符号 × 全仓引用计数）未发现第二个同类项；`fkv_large::decode` /
`decode_metadata_only`、`fkv_mapping::chunk_alloc_num` / `chunk_dir_copy_num` 虽也仅测试消费，
但注释明示「metadata-only 读取」「测试 / 诊断用」，属有意测试接缝，不在本条立项（见报告观察段）。

## C++ 对照

`include/coding.h` 的 `PutFixed64` 在 C++ 侧被 manifest checksum（`root_meta.cpp:160`
`EncodeFixed64(buff_.data(), CalcChecksum(content))`）等广泛使用。Rust 侧该写出点已内联为
`to_le_bytes` 直写，编码器函数成了迁移期残留——与 r8a-01 删除的 `DirectIoBuf` 别名同类
（SKILL.md「pub API 孤儿（导出无人消费）定期清理，删除后连带清理 re-export 与文档引用」）。

## 目标

二选一，不留零消费导出：

1. 删除 `put_fixed64` 与 `tests/codec.rs` 对应导入/断言（get/put 不对称无妨：get 有消费、put 没有）。
2. 或给出真实消费点（当前没有自然位置，不推荐为保留而造调用）。

## 验收

- `grep -rn "put_fixed64" crate tool web --include="*.rs"`：0 处（已删）或 ≥2 处（定义 + 消费点）。
- `./sh/clippy.sh`、`./test.sh` 通过。

## 核验记录

- 亲读 `fkv_codec/src/lib.rs:10-41`（fixed16/32/64 全组）确认 get_fixed64/put_fixed32/put_fixed16
  均有消费、唯独 put_fixed64 零消费；`put_varint64`/`append_varint64` 消费点在 `fkv_page/src/data.rs`、
  `fkv_mapping/src/lib.rs:292`、`fkv_manifest/src/lib.rs:249`。
- 零引用验证：`grep -rn -w put_fixed64` 覆盖 `crate/ tool/ web/`（全仓无 `bin/` 目录），含全部 tests。
- 查重：r8a-01（MAX_OVERFLOW_POINTERS / DirectIoBuf）同类不同符号；eng-82（unused pub 扫描）与
  r6 catchup 未覆盖本符号；r7 各组报告无涉及。
- 锚点 mtime：`fkv_codec/src/lib.rs` 02:17（2026-11-05），非并行写入区。


## DONE（2026-11-05，第三轮复审修复）

DONE：`put_fixed64` 已用于 page checksum 与 manifest checksum 写出，不再是零消费导出。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`put_fixed64` 现有真实生产消费（走 issue 目标 2 的「给出真实消费点」路径，非造调用）——
  - `fkv_page/src/header.rs:82` `set_checksum`：`put_fixed64(page, sum)`；
  - `fkv_manifest/src/lib.rs:307` `finalize`：`put_fixed64(&mut self.buf, sum)` 回填 checksum。
  与读侧 `get_fixed64`（header.rs:88 validate_checksum、manifest:359 validate_record）对称；两处原为内联 `to_le_bytes`+`copy_from_slice` 的等价写出，行为不变。
- 验收：`grep -rn -w put_fixed64 crate tool web` = 定义 + 2 生产消费 + tests（≥2 达）。
- 判定：完善。
