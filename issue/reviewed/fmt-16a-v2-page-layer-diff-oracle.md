# fmt-16a V2 差分的 page 层：本机 C++ oracle 打通 + 逐字节金标测试

源自 fmt-16（拆出已完成部分）。范围：`fkv_page` 的 data / index 页 builder 输出与 C++ **逐字节**一致。

## 审查结论（2026-11-04）

**采纳并已落地。** 纠正如实：既有结论里「macOS 本地只能跑 Rust 侧单测」对**整个 store**成立，但**对格式层不成立**。

实测（`/opt/homebrew` 已有 boost/glog/gflags/xxhash/zstd；`async_io_manager.h`（`<liburing.h>`）不在格式层 include 链上）：

```
clang++ -std=c++20 -fsyntax-only -DGLOG_USE_GLOG_EXPORT -Iinclude -I. -I/opt/homebrew/include -Iexternal \
  src/storage/data_page_builder.cpp src/storage/index_page_builder.cpp \
  src/storage/data_page.cpp src/compression.cpp src/coding.cpp      # 全部 RC=0
```

因此 page 层 V2 不依赖 Linux，也不依赖缺失的 submodule（abseil / concurrentqueue / inih）。
链接期只有两个无关符号需要桩（见下）。

## 本机 oracle 复现

`build.sh`（`eloqstore` 路径可传参）：

```bash
E=${1:-/Users/z/git/db/kv/eloqstore}; BREW=$(brew --prefix)
clang++ -std=c++20 -DNDEBUG -DGLOG_USE_GLOG_EXPORT -include unistd.h \
  -I"$E/include" -I"$E" -I"$E/external" -I"$BREW/include" -o oracle oracle.cpp \
  "$E/src/storage/data_page_builder.cpp" "$E/src/storage/index_page_builder.cpp" \
  "$E/src/compression.cpp" "$E/src/coding.cpp" -L"$BREW/lib" -lglog -lzstd
```

`oracle.cpp` 要点（完整源码见本节末）：

- 桩 4 个符号，均不影响格式字节：
  - `Comparator::DefaultComparator()` → `nullptr`（仅在 NDEBUG 关闭的 debug 断言里用）；
  - `Option()`（`tasks/task.h:44` 的自由函数）→ 返回静态 `KvOption{}`，`compression.cpp` 只读 `enable_compression` / zstd 级别；
  - `TypeOfPage` / `SetPageType` → 逐字照抄 `src/storage/page.cpp:7-20`（该 TU 会拖入 `object_store.h` → jsoncpp，本机没有）。
- 与生产一致的调用序：`DataPageBuilder` **构造后必须先 `Reset()`**（构造函数不设 page type，`data_page_builder.cpp:37-41` 只 resize + 建 restart；`SetPageType(..., Data)` 在 `Reset()` 里，`batch_write_task.cpp:192` 等 4 处都调了）。这是 oracle 的首个坑：不调 Reset 会得到 `type=0`。
- 金标 case（`data_page_size=512`，`data_page_restart_interval=2`，5 条）：
  `a/1 ts=100`、`b/2 ts=120`、`c/3 ts=110 expire=999`、`d/4 ts=200 overflow`、`e/5 ts=300 large_value`；
  index case：`index_page_restart_interval=2`，leaf，`Add("",10,true)` + `b/c/d/e → 20/30/40/50`。

oracle 输出（`content_len` 之后全 0 padding）：

```
data : 0000000000000000023f0000000000000000000001106131c8010001106232280001126333e707dc010001116434b40100011c6535d8041300200030000300
index: 00000000000000000125000a000000000162280001631400016450000165140f0017000200
```

## DONE（2026-11-04）

- `crate/format/fkv_page/tests/format_semantics.rs`：
  - `data_page_bytes_match_cpp_oracle`（63 字节金标，覆盖 overflow 位、large_value 位、expire ts、负 ts delta、3 个 restart 点）；
  - `index_page_bytes_match_cpp_oracle`（37 字节金标，leaf，leftmost ptr + restart array）。
  - 两者断言：`content_len` 与金标等长、`[..content_len]` 逐字节相等、其余 padding 全 0。
- 顺带证明 fmt-20a（构造缓冲懒初始化）产物逐字节不变、fmt-21（page_id 域检查）不改变合法输入编码。

测试：`cargo nextest run -p fkv_page` → 27 passed / 0 failed；`cargo clippy -p fkv_page --all-targets --no-deps -- -D warnings` → 0 警告。

## 未覆盖（留给 fmt-16 主体 / 01）

`fkv_codec`（varint/LE，可用同法快速补）、`fkv_compress`（zstd 字典需固定版本+参数）、
`fkv_manifest`（需 snapshot 完整构造 + CRC）、以及 5 个 `_cpp` crate 的 FFI 形态（Crubit vs 纯 C ABI）
与 `fixtures/` 流水线约定 —— 这些仍归 `01-cpp-oracle-and-fixtures.md` 统一决定，避免并行维护两套 oracle。

## 附：oracle.cpp 完整源码

```cpp
// fkv_page V2 差分 oracle（仅格式层，macOS 可编译）：dump C++ 侧 page 字节的十六进制。
// 编译见同目录 build.sh。桩说明见 issue/todo/fmt-16-cpp-stub-crates-empty.md。
#include <cstdio>
#include <string>
#include <string_view>
#include "kv_options.h"
#include "tasks/task.h"
#include "storage/data_page_builder.h"
#include "storage/index_page_builder.h"

namespace eloqstore
{
const Comparator *Comparator::DefaultComparator()
{
    return nullptr;
}
const KvOption *Option()
{
    static const KvOption opt{};
    return &opt;
}
PageType TypeOfPage(const char *p)
{
    return static_cast<PageType>(p[page_type_offset]);
}
void SetPageType(char *p, PageType t)
{
    p[page_type_offset] = static_cast<char>(t);
}
}  // namespace eloqstore

using namespace eloqstore;

static void dump(std::string_view page)
{
    for (unsigned char c : std::string(page))
    {
        printf("%02x", c);
    }
    printf("\n");
}

int main()
{
    KvOption opt;
    opt.data_page_size = 512;
    opt.data_page_restart_interval = 2;
    opt.index_page_restart_interval = 2;

    // case data：5 条（含 overflow / large_value / expire ts / 3 个 restart 点）
    {
        DataPageBuilder b(&opt);
        b.Reset();  // 生产 BatchWriteTask 构造后必调用（构造函数不设 page type）
        b.Add("a", "1", false, 100, 0, compression::CompressionType::None, false);
        b.Add("b", "2", false, 120, 0, compression::CompressionType::None, false);
        b.Add("c", "3", false, 110, 999, compression::CompressionType::None, false);
        b.Add("d", "4", true, 200, 0, compression::CompressionType::None, false);
        b.Add("e", "5", false, 300, 0, compression::CompressionType::None, true);
        dump(b.Finish());
    }
    // case index：leaf，leftmost 10，其后 4 条
    {
        IndexPageBuilder b(&opt);
        b.Add("", 10, true);
        b.Add("b", 20, true);
        b.Add("c", 30, true);
        b.Add("d", 40, true);
        b.Add("e", 50, true);
        dump(b.Finish());
    }
    return 0;
}
```

## 复审（2026-11-05 r6）

通过。金标测试在位且断言逐字节比对：`crate/format/fkv_page/tests/format_semantics.rs:224`（data 页 GOLDEN=:227，`[..golden.len()]` 逐字节相等 :267-271 + padding 全 0 :272-275）、`:283`（index 页 GOLDEN=:284，同款断言 :301-313）。金标十六进制与本文 oracle 输出一致。
