# r8b-01 P3：06-gaps 称「branch file id 预留 1024」已记录在 COMPAT_BASELINE，基线全文无此条，原登记 reject/11 也已删除——登记链断裂

## 现象（亲读）

- `doc/architecture/06-gaps.md:95-96`（2026-11-05 05:5x 亲读）：
  「**branch file id 预留 1024**：仍为有意差异（manifest 自带区间，双向可读不受影响），
  已无独立 open issue；记录在 [`COMPAT_BASELINE`](../../COMPAT_BASELINE)。」
- `COMPAT_BASELINE` 全文（工作区版，26 行）亲读 + `grep -n "1024\|预留\|BRANCH_FILE"`
  **零命中**——基线没有任何 branch 预留条目。
- 登记链回溯（`git diff -- doc/architecture/06-gaps.md`）：旧版 06-gaps 的该差异指向
  `issue/reject/11`；现在 `issue/reject/`（10 个文件）与 `issue/reviewed/` 均无
  `11-*` 文件——原登记载体已删除，重构后的 06-gaps 把指针改指 COMPAT_BASELINE，
  但从未把条目写进基线。当前该有意差异的唯一文字记录就是 06-gaps 这两行自身，
  且其引用指向不存在的内容。

代码事实（该差异的主体，均在 scope 内）：

- `crate/engine/fkv_store/src/lib.rs:90`：`BRANCH_FILE_RESERVE_FILE_NUM: u64 = 1024`
  （「main 跳过该区间，避免与 branch 并发写碰撞」）。
- `fkv_store/src/lib.rs:1906-1940`（`open_branch` 预留窗口 `div_euclid`×1024）、
  `:3333-3348`（`create_branch` `div_ceil`×1024 取整）、`:3400-3426`
  （`reserve_until` 推父 data mapper 高水位）、`:3662-3377`（segment 侧同构）。
- 测试断言该行为：`fkv_store/tests/branch_isolation.rs:72-77`、
  `branch_archive.rs:160`。

## C++ 对照

`src/tasks/background_write.cpp:614` `BackgroundWrite::CreateBranch`：**无任何预留**——
`new_max_fp_id = (parent CurrentFileId + 1) << pages_per_file_shift` 只进 branch
manifest，父分支 allocator 不跳区间；跨 branch 同 file id 靠对象名
`<prefix>_<file_id>_<branch>_<term>`（fkv 侧 `fkv_types/src/lib.rs:265-274` 同构）
做命名空间隔离。`include/storage/page_mapper.h` / `page_mapper.cpp` 的 `reserve`
仅是 `MappingTbl` 容量预分配，与 file id 区间无关。

即「父分支跳 1024 个文件」是 fkv 有意差异（06-gaps 自述理由：main/branch 并发
写隔离），与 `rev-39` 复审记录的「同 id 跨 branch 常态存在」（issue/reviewed/
rev-39-*.md:23/:89 引用此常量）互为印证——差异本身无争议，缺的是基线登记本体。

## 目标

1. 在 `COMPAT_BASELINE` 排除清单补一行：branch file id 预留 1024（C++ 只取
   父分支下一个文件 id）；manifest 自带 `BranchFileRange` 区间、双向可读不受
   影响；判定依据 06-gaps 现有表述 + rev-39。
2. 若裁定不进基线（如认为属引擎内契约非互操作排除项），则把 06-gaps:96 的
   指针改指真实载体（如 rev-39 复审小节），并删除「记录在 COMPAT_BASELINE」
   的失实表述。
3. 改基线须同步重跑 `scripts/gen_fixtures.sh` 刷新 38 份 `meta.txt` 的
   `baseline=`（migration.md §8 口径），否则 drift 检查失败。

## 验收

- `grep -n "预留" COMPAT_BASELINE` 命中 branch file id 条目，或
  `doc/architecture/06-gaps.md` 不再引用 COMPAT_BASELINE 作为该条载体；
- `fixtures/**/meta.txt` 与 COMPAT_BASELINE flatten 逐字节一致（若改基线）；
- `doc/architecture/06-gaps.md` 无指向不存在文件 / 不存在基线条目的引用。

## 核验记录

- 亲读 `COMPAT_BASELINE` 全文（工作区版）与 `doc/architecture/06-gaps.md:90-96`；
  `git diff -- COMPAT_BASELINE` / `git diff -- doc/architecture/06-gaps.md` 回溯登记链。
- 亲读 `fkv_store/src/lib.rs` 预留相关四处与两个测试文件的断言。
- 亲读 C++ `background_write.cpp` `CreateBranch`（:614 起）与
  `page_mapper.cpp` `MappingTbl::reserve`（:67）确认 C++ 无区间预留。
- 查重：r8c-01/r8c-02/r8d-01 均不涉本条（GC 护栏 / rev-37 文档 / write_gate 模块
  注释）；eng-93（buffer pool 登记行）已归档，范围不含 branch 预留；
  `issue/todo/` 空，无既有条目承接。
- mtime 备注：`fkv_store/src/lib.rs` mtime 05:19（r8d-01 记录的并行改动），本轮
  亲读以 05:32+ 工作区内容为准；`cargo check` 三 crate 零警告、
  `cargo nextest run` 193/193 通过，本条为纯登记欠账，非代码缺陷。


## DONE（2026-11-05，第二轮复审修复）

DONE：`COMPAT_BASELINE` 增加 branch file id 预留 1024 的差异条目，登记链恢复。

## 复审（2026-11-05 v，lane A）

- 亲读复核：`COMPAT_BASELINE:57-60` 已有条目「branch file id 预留：Rust `create_branch` 把父 allocator 高水位推到下一个 1024 file id 边界（`BRANCH_FILE_RESERVE_FILE_NUM`），C++ `CreateBranch` 不预留……不纳入逐字节等价」，登记链恢复。
- 验收：`grep -n 预留 COMPAT_BASELINE` 命中该条；`fixtures/**/meta.txt`（38 份）`baseline=` 与基线 flatten 同步（例 `fixtures/rust/page/empty/meta.txt` 含同条；`git status` 显示 38 份 meta 在刷）。`doc/architecture/06-gaps.md:95` 指向 COMPAT_BASELINE 的表述现行有效。
- 判定：完善。
