# cld-57 P3：`global_list_archive_tags` 不按 `(tag, term)` 去重，C++ `std::unique` 去重；且代码注释误述「与 C++ 一致」

热点文件：需与 owner 会话协调（`fkv_runtime/src/lib.rs` 在另一会话 staged 改动中；本差异来自 11-04 已合入的稳定代码，非半途伪差异）。

## 来源

r6 复审（2026-11-05，runtime/cloud lane）复核 eng-84 DONE 段时对读 C++ 发现。

## 证据（file:line + 摘录）

Rust `crate/engine/fkv_runtime/src/lib.rs:186-193`：

```rust
fn list_archive_tags(root_li: &[PathBuf], prefix: &str) -> Vec<ArchiveEntry> {
  let mut entry_li: Vec<ArchiveEntry> = Vec::new();
  for root in root_li {
    list_archive_tags_of(root, prefix, &mut entry_li);
  }
  entry_li.sort();     // 只有排序，没有去重
  entry_li
}
```

`:194-220` `list_archive_tags_of` 对每个分区目录里的每个带 tag manifest 各 push 一条
`ArchiveEntry { tag, term }`（`:217`），跨分区不去重。

`:178-180` 文档注释声称「与 C++ 一致的三点：… ② **不按 tag 去重**（同名 tag 由
不同 branch / term 各占一条）」；`issue/todo/eng-84-*.md` DONE 段同样写「④ 不按
tag 去重」。两处对 C++ 的表述与 C++ 源码不符（见下）。

## C++ 对照（eloqstore @ 82b774c）

`src/eloq_store.cpp:2206-2213` 排序后，`:2217-2221`：

```cpp
req->entries_.erase(
    std::unique(req->entries_.begin(), req->entries_.end(),
                [](const ... lhs, const ... rhs) {
                    return lhs.term == rhs.term && lhs.tag == rhs.tag;
                }),
    req->entries_.end());
```

即 C++ 对 `(tag, term)` 完全相等的条目**去重**：`global_archive(tag)` fanout 到 N 个
分区、每分区写 `manifest_main_<term>_<tag>`，同名 tag + 同 term 只回一条。

## 问题

- 多分区场景 Rust 返回 N 条重复 `(tag, term)`（local 模式 term 恒 0，必然重复），
  调用方按 C++ 语义消费会当成 N 个不同归档点。
- 注释 / eng-84 DONE 把「不去重」记成 C++ 行为，属对齐性误述，会误导后续 oracle
  差分（01）的判定。

## 建议修法

`list_archive_tags` 在 `sort()` 之后 `dedup()`（`ArchiveEntry` 已 derive
`PartialEq`，`Vec::dedup` 直接可用）；同步修正 `:178-180` 注释与 eng-84 DONE 表述。

## 验收

- 单测：两分区同 `(tag, term)` 归档后 `global_list_archive_tags` 只回一条；同 tag
  不同 term 保留两条（C++ 语义）。现有
  `global_tests::global_list_archive_tags_scans_disk_and_sorts` 语料 `[001/1, 001/2]`
  需补一条同 `(tag, term)` 跨分区用例。
- `cargo nextest run -p fkv_runtime`。

## 甄别（2026-11-05，采纳）

复核确认意见三项声称全部成立（行号按当日 HEAD 内容定位；并行会话已把
`list_archive_tags` 改名 `list_archive_tag_li`、`Runtime::global_list_archive_tags`
改名 `global_list_archive_tag_li`，行号恰好未漂移，语义未变）：

1. **Rust 不去重属实**：`fkv_runtime/src/lib.rs:186-193` 多 root × 多分区目录聚合后
   仅 `entry_li.sort()`，无 `dedup()`；`list_archive_tags_of`（:195-222）对每个分区
   目录的每个带 tag manifest 各 push 一条。而 `global_archive`（:1342-1347）fanout
   `Request::CreateArchive` 到每个分区、各分区独立写 `manifest_<branch>_<term>_<tag>`，
   跨分区同 `(tag, term)` 必然重复（local 模式无选举、term 恒 0 时 100% 重复）。
2. **C++ 去重属实且前提正确**：`HandleGlobalListArchiveTagsRequest`
   （eloqstore/src/eloq_store.cpp:2133-2225）先遍历 `options_.store_path` 全部根下
   全部分区目录聚合（:2138-2204），`std::sort` 按 `(tag, term)`（:2206-2216）后
   `std::unique` 以 `lhs.term == rhs.term && lhs.tag == rhs.tag` 去重并 erase 尾部
   （:2217-2223）——去重键即 `(tag, term)` 全等，前提是跨全局聚合，与意见一致。
   注意 C++ 只去掉全等条目，同 tag 不同 term 仍各占一条（eng-84 ④ 若如此表述则对，
   现表述「不按 tag 去重」省略了全等去重，构成对 C++ 的误述）。
3. **误述属实**：lib.rs:179-181「② 不按 tag 去重」与
   `issue/todo/eng-84-…md:82`「④ **不按 tag 去重**」均把不去重记成「对齐 C++」，
   与 C++ 源码相反，会误导后续 oracle 差分（01）判定。
4. **测试缺口属实**：`global_tests::global_list_archive_tags_scans_disk_and_sorts`
   （lib.rs:2174-）语料 `[001/1(branchA), 001/2(main), 0012/3, 002/2]` 全部互异，
   无跨分区同 `(tag, term)` 用例，测不出该差异。
5. **查重**：grep issue/ 全目录，仅 `reviewed/00-r6-runtime-cloud.md:44,68` 的 r6
   登记索引提到本条（非覆盖性修复）；eng-84 为误述对象本身；12c 两份无 dedup 相关
   内容。无重复。

影响面：控制面语义差异——SDK 用户 `global_list_archive_tags` 看到同 `(tag, term)`
重复 N 条（N=分区数），按 C++ 语义消费会误判为 N 个归档点。非数据面、无正确性
风险，P3 定级合理。

## 开发计划

1. `list_archive_tag_li` 在 `entry_li.sort()` 后加 `entry_li.dedup()`
   （`ArchiveEntry` 已 derive `PartialEq`/`Eq`，`Vec::dedup` 相邻去重可用；sort 在前
   保证全等条目相邻，与 C++ `sort`+`unique` 结构对齐）。
2. 修 lib.rs:179-181 注释：改为「② 仅去掉全等 `(tag, term)` 条目（C++
   `std::unique`，多分区 fanout 归档同 tag 同 term 只回一条）；同 tag 不同 term
   各占一条」。同步修 eng-84 DONE ④ 表述（在该文件补勘误行，不改历史结论结构）。
3. 补单测：两分区各写 `archive_file_name("main", T, "003")`（同 tag 同 term）断言
   只回一条；保留现有 `[001/1, 001/2]` 断言（同 tag 不同 term 两条，C++ 语义）。
4. 验证：`cargo nextest run -p fkv_runtime`。
5. 协调：`fkv_runtime/src/lib.rs` 在并行会话热改中，动手前与 owner 会话协调基线，
   以内容（`list_archive_tag_li`）定位而非行号。

## DONE（2026-11-05）

- `list_archive_tag_li`：`entry_li.sort()` 后新增 `entry_li.dedup()`，与 C++
  `std::sort` + `std::unique` 结构一致。
- 修正 `list_archive_tag_li` 文档注释：由「不按 tag 去重」改为「按 `(tag, term)`
  全等去重；同 tag 不同 term 各占一条」。
- 单测 `global_tests::global_list_archive_tags_scans_disk_and_sorts` 补跨分区
  全等 `(tag, term)` 语料（`demo.1`/`demo.2` 各一份 `manifest_main_2_001`），
  断言仍只回一条。
- 集成测试 `crate/engine/fkv_runtime/tests/global_list_archive_tags.rs`：两个
  分区真实 fanout 归档后，期望由 4 条改为按 C++ 语义去重后的 2 条。
- 验证：`cargo test -p fkv_runtime --lib`（8 passed）、
  `cargo test -p fkv_runtime --test global_list_archive_tags`（1 passed）。
- 相关全局控制面语义修正见 `issue/done/12a-archive-lifecycle-and-global-control-plane.md`。

## 复审（2026-11-05 r7）

结论：**通过**。核当前源码（均亲读）：

- `fkv_runtime/src/lib.rs:225-233` `list_archive_tag_li`：`entry_li.sort()` 后
  `entry_li.dedup()`（`:230-231`），与 C++ `std::sort`+`std::unique` 结构一致。
- 注释已改对：`:217-220`「② 排序后按 `(tag, term)` 全等去重（C++ `std::sort` +
  `std::unique`：多分区 fanout 归档同 tag 同 term 只回一条，同 tag 不同 term
  各占一条）」。
- 单测 `:2494-2579`：语料含跨分区全等 `(tag, term)`（`demo.1` 与 `demo.2` 各写
  `manifest_main_2_001`，`:2505/:2510`），断言 `(001,2)` 只回一条且 `(001,1)`/
  `(001,2)` 同 tag 不同 term 各占一条（`:2518-2540`）。
- 集成 `fkv_runtime/tests/global_list_archive_tags.rs:96-122`：两分区真实 fanout
  归档 2 个 tag，期望按去重后的 2 条（从盘面独立推导 `expect_li.dedup()`）。
