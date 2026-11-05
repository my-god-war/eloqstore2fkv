# eng-85 P3：定长读取的 `try_into().map_err` 是静态不可达的错误映射

来源：第 5 轮自查（foundation/format lane）。

## 证据（2026-11-04 核验）

`src.get(..N).ok_or(EndOfFile)?` 之后 `&[u8]` 长度恰为 N，紧随的
`TryInto<[u8; N]>` **静态必然成功**，`map_err(|_| KvError::EndOfFile)` 是
永远不会执行的防御分支，违反规范「避免防御式编程；100% 安全处减少开销」：

- `crate/foundation/fkv_codec/src/lib.rs:16-23`（`get_fixed16`）
- 同文件 `:37-44`（`get_fixed32`）、`:46-53`（`get_fixed64`）
- `crate/format/fkv_manifest/src/lib.rs:106`、`:114`（同模式两处）

## 目标

改为单次边界检查 + 不可达转换收敛，例如：

```rust
pub fn get_fixed16(src: &[u8]) -> Result<u16> {
  let bytes: &[u8; 2] = src.get(..2).ok_or(KvError::EndOfFile)?.try_into().unwrap();
  Ok(u16::from_le_bytes(*bytes))
}
```

（`try_into().unwrap()` 在长度已验证处是惯用不可达写法；或直接
`src.first_chunk::<2>()` 风格统一 16/32/64 三档。）5 处一次改净，
热路径（页解码 / manifest 解析逐字段）少一层 Result 分派。

## 验收

- `cargo nextest run -p fkv_codec -p fkv_manifest`；clippy 零警告。

## 依赖 / 重叠

- 无依赖；与 fmt-13（varint 严格性）同文件不同函数，不重叠。

## 核验记录

- 本人实读：`fkv_codec/src/lib.rs` 全文、`fkv_manifest/src/lib.rs:100-120`。

## DONE（2026-11-05，本会话）

- `fkv_codec::get_fixed16/32/64` 改 `src.first_chunk::<N>()`（nightly 1.101），
  单次边界检查、零防御分支。
- `fkv_manifest::read_u32/read_u64` 改 `data.get(*pos..).and_then(|rest| rest.first_chunk::<N>())`。
- 验证：`cargo nextest run -p fkv_codec -p fkv_manifest` 12 passed；
  clippy 零警告。

## 复审（2026-11-05 r6）

通过。`fkv_codec/src/lib.rs:17/34/39` get_fixed16/32/64 均 `src.first_chunk::<N>().ok_or(KvError::EndOfFile)`；`fkv_manifest/src/lib.rs:105/114` read_u32/read_u64 走 `first_chunk`。防御分支清零。
