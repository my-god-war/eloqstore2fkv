// C++ oracle：用 eloqstore 真实 builder / parser 生成字节 fixture，供 Rust 侧解码断言。
//
// 契约（与 fixtures/README.md 一致）：
//   fkv_oracle fixture --kind <K> --case <C> --out <DIR>   # 写 DIR/payload.bin
//   fkv_oracle verify  --case-dir <DIR> --kind <K> --case <C>
//
// kind / case 矩阵：
//   page      empty single multi restart ttl large overflow（large_values 为 large 别名）
//   index     leaf branch
//   overflow  chain（data 页 + 连号 overflow 页，覆盖 overflow 页自身格式）
//   manifest  empty checksum branch archive
//   compress  raw dict（dict 额外写 DIR/dict.bin）
//   mapping   empty single multi（MappingSnapshot::Serialize 字节 + count 前缀）
//   store     empty single multi（有序 name/len/bytes 目录文件清单）
//
// 未实现的 kind/case 明确报错退出（退出码 3），**不静默跳过**，免得「没跑」被当成「通过」。

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <initializer_list>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "coding.h"
#include "common.h"
#include "compression.h"
#include "kv_options.h"
#include "manifest_buffer.h"
#include "standalone.h"
#include "storage/data_page.h"
#include "storage/data_page_builder.h"
#include "storage/index_page_builder.h"
#include "storage/mem_cached_page.h"
#include "storage/page.h"
#include "storage/page_mapper.h"
#include "storage/root_meta.h"
#include "tasks/task.h"
#include "types.h"

namespace fs = std::filesystem;
using eloqstore::compression::CompressionType;

namespace
{

constexpr int kExitUsage = 2;
constexpr int kExitUnsupported = 3;
constexpr int kExitIo = 4;
constexpr int kExitInvalid = 5;

constexpr std::string_view kPayloadName = "payload.bin";
constexpr std::string_view kDictName = "dict.bin";
// 与 KvOptions 默认 data_page_size 一致；builder 输出的页固定这么大。
constexpr uint16_t kPageSize = 4 * 1024;
// overflow 组的起始页 id（写入 data entry 指针与页内容，跨语言一致）。
constexpr eloqstore::PageId kOverflowPageBase = 7;
constexpr std::string_view kOverflowKey = "of";

// 一个写入条目：key/value 与时间戳、过期时间、overflow / large-value 标志。
struct Entry
{
    std::string key;
    std::string value;
    uint64_t ts;
    uint64_t expire_ts;
    bool overflow{false};
    bool large_value{false};
};

struct IndexEntry
{
    std::string key;
    eloqstore::PageId page_id;
};

struct CompressFixture
{
    std::string payload;
    std::string dict;
};

std::string Padded(std::string_view prefix, size_t index, size_t width)
{
    std::string text(prefix);
    std::string digits = std::to_string(index);
    if (digits.size() < width)
    {
        text.append(width - digits.size(), '0');
    }
    text.append(digits);
    return text;
}

std::string BytesOf(char fill, size_t len)
{
    return std::string(len, fill);
}

// 重复 pattern 直到 target 字节，超出部分截断；跨语言测试用同一规则。
std::string RepeatTo(std::string_view pattern, size_t target)
{
    std::string out;
    out.reserve(target + pattern.size());
    while (out.size() < target)
    {
        out.append(pattern);
    }
    out.resize(target);
    return out;
}

// ---------------------------------------------------------------------------
// page：DataPageBuilder 语料
// ---------------------------------------------------------------------------

std::vector<Entry> PageEntriesOf(std::string_view name)
{
    std::vector<Entry> entry_li;
    if (name == "empty")
    {
        return entry_li;
    }
    if (name == "single")
    {
        entry_li.push_back({"k0", "v0", 100, 0, false, false});
        return entry_li;
    }
    if (name == "multi")
    {
        for (size_t i = 0; i < 8; i++)
        {
            entry_li.push_back({Padded("k", i, 2), Padded("v", i, 2),
                                uint64_t(100 + i), 0, false, false});
        }
        return entry_li;
    }
    if (name == "restart")
    {
        // 40 条 > data_page_restart_interval(16)，强制多段 restart 数组。
        for (size_t i = 0; i < 40; i++)
        {
            entry_li.push_back(
                {Padded("r", i, 3), BytesOf('x', 16), uint64_t(100 + i), 0,
                 false, false});
        }
        return entry_li;
    }
    if (name == "ttl")
    {
        for (size_t i = 0; i < 4; i++)
        {
            entry_li.push_back({Padded("t", i, 2), Padded("v", i, 2),
                                uint64_t(100 + i), uint64_t(1000 + i), false,
                                false});
        }
        return entry_li;
    }
    if (name == "large" || name == "large_values")
    {
        for (size_t i = 0; i < 3; i++)
        {
            entry_li.push_back({Padded("L", i, 2), BytesOf(char('a' + i), 1024),
                                uint64_t(100 + i), 0, false, true});
        }
        return entry_li;
    }
    if (name == "overflow")
    {
        // 中间一条带 overflow 标志：value 是指针数组（每个 PageId 小端 4B）。
        entry_li.push_back({"o00", "v00", 100, 0, false, false});
        entry_li.push_back(
            {"o01", std::string("\x07\x00\x00\x00\x08\x00\x00\x00", 8), 101, 0,
             true, false});
        entry_li.push_back({"o02", "v02", 102, 0, false, false});
        return entry_li;
    }
    return {};
}

std::vector<IndexEntry> IndexEntriesOf(std::string_view name)
{
    if (name == "leaf")
    {
        return {{"i00", 11}, {"i01", 12}, {"i02", 13}, {"i03", 14}};
    }
    if (name == "branch")
    {
        return {{"n00", 101}, {"n01", 102}, {"n02", 103}};
    }
    return {};
}

eloqstore::PageId IndexLeftmostOf(std::string_view name)
{
    return name == "leaf" ? 1 : 100;
}

bool IndexIsLeaf(std::string_view name)
{
    return name == "leaf";
}

// ---------------------------------------------------------------------------
// compress：CompressRaw / DictCompression 语料
// ---------------------------------------------------------------------------

std::string RawInput()
{
    return RepeatTo("fkv-raw-compress|0123456789abcdef|", 2048);
}

std::string DictInput()
{
    return RepeatTo("fkv-dict-compress|branch=main|term=7|", 2048);
}

std::vector<std::string> DictSamples()
{
    std::vector<std::string> sample_li;
    sample_li.reserve(64);
    for (size_t i = 0; i < 64; i++)
    {
        std::string head = "fkv-dict-sample-" + Padded("", i, 2) + "|";
        sample_li.push_back(RepeatTo(head + "branch=main|term=7|", 1024));
    }
    return sample_li;
}

// ---------------------------------------------------------------------------
// manifest：ManifestBuilder 语料
// ---------------------------------------------------------------------------

struct SegmentSpec
{
    eloqstore::FilePageId max_segment_fp_id{0};
    std::vector<eloqstore::FilePageId> fp_li;
};

struct ManifestSpec
{
    eloqstore::PageId root{0};
    eloqstore::PageId ttl_root{eloqstore::MaxPageId};
    eloqstore::BranchManifestMetadata meta;
    std::vector<std::pair<eloqstore::PageId, eloqstore::FilePageId>>
        mapping_li;
    // true：`ManifestBuilder::Snapshot` 布局（branch / archive 真实快照）；
    // false：log record 布局（empty / checksum，覆盖 delta 与 checksum）。
    bool snapshot{false};
    eloqstore::FilePageId max_fp_id{0};
    std::string dict_bytes;
    // 可选 segment 段：C++ `Snapshot` 只在 segment_mapping != nullptr 时写。
    std::optional<SegmentSpec> segment;
};

ManifestSpec ManifestSpecOf(std::string_view name)
{
    using eloqstore::BranchFileRange;
    ManifestSpec spec{};
    if (name == "empty")
    {
        spec.root = 0;
        return spec;
    }
    if (name == "checksum")
    {
        // 只有 mapping + 空 branch metadata：覆盖 checksum 与 mapping 段。
        spec.root = 2;
        spec.mapping_li = {{0, 1}, {1, 2}};
        return spec;
    }
    if (name == "branch")
    {
        // 真实 branch snapshot：`BackgroundWrite::CreateBranch` 调 Snapshot
        // 时不传 segment mapping，因此记录在 branch metadata 之后即结束，
        // 没有可选 segment 段。
        spec.root = 3;
        spec.snapshot = true;
        spec.max_fp_id = uint64_t(58) << 18;
        spec.meta.branch_name = "feature-a3f7b2c1";
        spec.meta.term = 7;
        spec.meta.file_ranges = {
            BranchFileRange{"main", 0, 42, 43},
            BranchFileRange{"feature-a3f7b2c1", 7, 57, 58},
        };
        spec.mapping_li = {{0, 5}, {1, 6}, {2, 7}};
        return spec;
    }
    if (name == "archive")
    {
        // 真实 archive snapshot：`BackgroundWrite::CreateArchive` 在 segment
        // mapper 存在时传入 mapping，因此带 max_seg_fp_id + seg mapping 段；
        // 同时覆盖非空 dict_bytes。
        spec.root = 9;
        spec.ttl_root = 10;
        spec.snapshot = true;
        spec.max_fp_id = 100;
        spec.dict_bytes = "fkv-archive-dict";
        spec.meta.branch_name = "main";
        spec.meta.term = 0;
        spec.meta.file_ranges = {BranchFileRange{"main", 0, 99, 100}};
        spec.mapping_li = {{0, 21}, {1, 22}};
        spec.segment = SegmentSpec{5, {1, 2}};
        return spec;
    }
    return spec;
}

// ---------------------------------------------------------------------------
// mapping：MappingSnapshot::Serialize 语料
// ---------------------------------------------------------------------------

// payload 布局：page_count(varint32) | page_count × encoded_value(varint64)。
// 主体字节就是 `MappingSnapshot::Serialize`（MappingTbl 按 PageId 升序写
// varint64），count 前缀让空映射也有非空 payload，并显式给出逻辑页数。
struct MappingSpec
{
    std::vector<uint64_t> value_li;
};

MappingSpec MappingSpecOf(std::string_view name)
{
    MappingSpec spec{};
    if (name == "empty")
    {
        return spec;
    }
    if (name == "single")
    {
        spec.value_li = {eloqstore::MappingSnapshot::EncodeFilePageId(42)};
        return spec;
    }
    if (name == "multi")
    {
        // 覆盖 FilePageId（含多字节 varint）、Invalid（已删除槽）与 PageId 三种值。
        spec.value_li = {
            eloqstore::MappingSnapshot::EncodeFilePageId(1),
            eloqstore::MappingSnapshot::InvalidValue,
            eloqstore::MappingSnapshot::EncodeFilePageId(5),
            eloqstore::MappingSnapshot::EncodePageId(0),
            eloqstore::MappingSnapshot::EncodeFilePageId(9),
            eloqstore::MappingSnapshot::InvalidValue,
            eloqstore::MappingSnapshot::EncodeFilePageId(257),
            eloqstore::MappingSnapshot::EncodeFilePageId(65536),
        };
        return spec;
    }
    return spec;
}

// ---------------------------------------------------------------------------
// store：目录 / 文件清单语料
// ---------------------------------------------------------------------------

// payload 布局：magic "FST1"(4B) | file_count(varint32) |
//               file_count × [name_len(varint32) | name | data_len(varint32) | data]
// 文件按 name 字节序升序；字节只来自真实 builder（data 页 / manifest record）
// 或固定 pattern（segment），保证跨语言可复现。
constexpr std::string_view kStoreMagic = "FST1";

// store 语料复用 page / manifest 的真实 builder；定义在下方构建段。
std::string BuildPage(std::string_view name,
                      const eloqstore::KvOptions &options);
std::string BuildManifest(std::string_view name);

struct StoreFile
{
    std::string name;
    std::string data;
};

std::vector<StoreFile> StoreFilesOf(std::string_view name,
                                    const eloqstore::KvOptions &options)
{
    std::vector<StoreFile> file_li;
    if (name == "empty")
    {
        return file_li;
    }
    if (name == "single")
    {
        file_li.push_back({"data_0_0", BuildPage("single", options)});
    }
    else if (name == "multi")
    {
        // 真实命名规则见 eloqstore/common.h：
        //   data_<file_id>_<term> / manifest_<term> / manifest_<term>_<ts>
        //   segment_<file_id>_<branch>_<term>
        file_li.push_back({"data_0_0", BuildPage("multi", options)});
        file_li.push_back({"data_1_0", BuildPage("single", options)});
        file_li.push_back({"manifest_0", BuildManifest("branch")});
        file_li.push_back({"manifest_0_1730000000", BuildManifest("archive")});
        file_li.push_back(
            {"segment_0_main_0", RepeatTo("fkv-segment|main|0|", 512)});
    }
    std::sort(file_li.begin(), file_li.end(),
              [](const StoreFile &lhs, const StoreFile &rhs)
              { return lhs.name < rhs.name; });
    return file_li;
}

// ---------------------------------------------------------------------------
// overflow：overflow 页自身格式（data entry + 单组连号 overflow 页）
// ---------------------------------------------------------------------------

struct OverflowChunk
{
    eloqstore::PageId page_id;
    std::string value;
};

struct OverflowSpec
{
    std::string value;
    std::vector<OverflowChunk> chunk_li;
    // data entry 的 value：每页一个 PageId，fixed32 LE（真实写路径 PutFixed32）。
    std::string pointers;
};

OverflowSpec OverflowSpecOf(const eloqstore::KvOptions &options)
{
    OverflowSpec spec;
    spec.value = RepeatTo("eloqstore-overflow-", 8000);
    const size_t page_cap =
        eloqstore::OverflowPage::Capacity(&options, /*end=*/false);
    eloqstore::PageId page_id = kOverflowPageBase;
    for (size_t off = 0; off < spec.value.size(); off += page_cap)
    {
        const size_t len = std::min(page_cap, spec.value.size() - off);
        spec.chunk_li.push_back({page_id, spec.value.substr(off, len)});
        char slot[sizeof(eloqstore::PageId)];
        eloqstore::EncodeFixed32(slot, page_id);
        spec.pointers.append(slot, sizeof(slot));
        page_id++;
    }
    return spec;
}

// ---------------------------------------------------------------------------
// case 表
// ---------------------------------------------------------------------------

bool InList(std::string_view name, std::initializer_list<std::string_view> li)
{
    for (std::string_view candidate : li)
    {
        if (name == candidate)
        {
            return true;
        }
    }
    return false;
}

bool KnownCase(std::string_view kind, std::string_view name)
{
    if (kind == "page")
    {
        return InList(name,
                      {"empty", "single", "multi", "restart", "ttl", "large",
                       "large_values", "overflow"});
    }
    if (kind == "index")
    {
        return InList(name, {"leaf", "branch"});
    }
    if (kind == "overflow")
    {
        return name == "chain";
    }
    if (kind == "manifest")
    {
        return InList(name, {"empty", "checksum", "branch", "archive"});
    }
    if (kind == "compress")
    {
        return InList(name, {"raw", "dict"});
    }
    if (kind == "mapping")
    {
        return InList(name, {"empty", "single", "multi"});
    }
    if (kind == "store")
    {
        return InList(name, {"empty", "single", "multi"});
    }
    return false;
}

// ---------------------------------------------------------------------------
// 构建（返回 payload，可选 dict）
// ---------------------------------------------------------------------------

std::string BuildPage(std::string_view name, const eloqstore::KvOptions &options)
{
    eloqstore::DataPageBuilder builder(&options);
    // eloqstore 的写路径在首次使用前调用 Reset()（构造函数本身不写页类型）。
    builder.Reset();
    for (const Entry &entry : PageEntriesOf(name))
    {
        const bool added = builder.Add(entry.key, entry.value, entry.overflow,
                                       entry.ts, entry.expire_ts,
                                       CompressionType::None,
                                       entry.large_value);
        if (!added)
        {
            std::fprintf(stderr, "oracle: builder.Add failed at key %s\n",
                         entry.key.c_str());
            return {};
        }
    }
    std::string_view page = builder.Finish();
    std::string blob(page.data(), page.size());
    // 真实写路径在 Finish() 之后补 checksum，fixture 必须带上才与盘面一致。
    eloqstore::SetChecksum(blob);
    return blob;
}

std::string BuildIndex(std::string_view name,
                       const eloqstore::KvOptions &options)
{
    eloqstore::IndexPageBuilder builder(&options);
    const std::vector<IndexEntry> entry_li = IndexEntriesOf(name);
    if (entry_li.empty())
    {
        return {};
    }
    // 第一次 Add 只设置 leftmost 指针与页类型。
    if (!builder.Add({}, IndexLeftmostOf(name), IndexIsLeaf(name)))
    {
        return {};
    }
    for (const IndexEntry &entry : entry_li)
    {
        if (!builder.Add(entry.key, entry.page_id, IndexIsLeaf(name)))
        {
            std::fprintf(stderr, "oracle: index Add failed at key %s\n",
                         entry.key.c_str());
            return {};
        }
    }
    std::string_view page = builder.Finish();
    std::string blob(page.data(), page.size());
    eloqstore::SetChecksum(blob);
    return blob;
}

CompressFixture BuildCompress(std::string_view name)
{
    CompressFixture fixture;
    const int level = eloqstore::Options()->zstd_compression_level;
    if (name == "raw")
    {
        if (!eloqstore::compression::CompressRaw(RawInput(), fixture.payload,
                                                 level))
        {
            std::fprintf(stderr, "oracle: CompressRaw failed\n");
            return {};
        }
        return fixture;
    }
    if (name == "dict")
    {
        eloqstore::compression::DictCompression dict;
        for (const std::string &sample : DictSamples())
        {
            dict.AddSample(sample);
        }
        dict.BuildDictionary();
        if (!dict.HasDictionary() || dict.DictionaryBytes().empty())
        {
            std::fprintf(stderr, "oracle: dictionary training failed\n");
            return {};
        }
        if (!dict.Compress(DictInput(), fixture.payload))
        {
            std::fprintf(stderr, "oracle: dictionary Compress failed\n");
            return {};
        }
        fixture.dict = dict.DictionaryBytes();
        return fixture;
    }
    return {};
}

// data 页 + 单组 overflow 页连成一个 payload；两段都补 checksum（真实写路径
// 对每个页对象在落盘前补 checksum）。
std::string BuildOverflow(const eloqstore::KvOptions &options)
{
    const OverflowSpec spec = OverflowSpecOf(options);
    eloqstore::DataPageBuilder builder(&options);
    builder.Reset();
    if (!builder.Add(kOverflowKey, spec.pointers, /*overflow=*/true, /*ts=*/100,
                     /*expire_ts=*/0, CompressionType::None,
                     /*large_value=*/false))
    {
        std::fprintf(stderr, "oracle: overflow data entry rejected\n");
        return {};
    }
    std::string blob(builder.Finish());
    eloqstore::SetChecksum(blob);

    for (const OverflowChunk &chunk : spec.chunk_li)
    {
        // OverflowPage 写构造从 shard 页池取缓冲：standalone shim 提供独立池。
        eloqstore::InitStandalone();
        eloqstore::OverflowPage page(chunk.page_id, &options, chunk.value, {});
        eloqstore::ResetStandalone();
        std::string raw(page.PagePtr(), kPageSize);
        eloqstore::SetChecksum(raw);
        blob += raw;
    }
    return blob;
}

std::string BuildManifest(std::string_view name)
{
    const ManifestSpec spec = ManifestSpecOf(name);
    eloqstore::ManifestBuilder builder;
    std::string_view record;
    if (!spec.snapshot)
    {
        for (const auto &[page_id, file_page_id] : spec.mapping_li)
        {
            builder.UpdateMapping(page_id, file_page_id);
        }
        if (name != "empty")
        {
            // 追加段同时回填 mapping_len，形成合法的 log record。
            builder.AppendBranchManifestMetadata(
                eloqstore::SerializeBranchManifestMetadata(spec.meta));
        }
        record = builder.Finalize(spec.root, spec.ttl_root);
    }
    else
    {
        // 真实 snapshot 布局：max_fp_id | dict | mapping_tbl | branch metadata
        // | [segment mapping]。MappingSnapshot 的 idx_mgr 传 nullptr：oracle
        // 不持有 PageManager，析构直接跳过回收。
        eloqstore::MappingSnapshot::MappingTbl mapping_tbl;
        for (const auto &[page_id, file_page_id] : spec.mapping_li)
        {
            (void) page_id;
            mapping_tbl.PushBack(
                eloqstore::MappingSnapshot::EncodeFilePageId(file_page_id));
        }
        eloqstore::MappingSnapshot mapping(nullptr, nullptr,
                                           std::move(mapping_tbl));

        std::unique_ptr<eloqstore::MappingSnapshot> segment_mapping;
        eloqstore::FilePageId max_segment_fp_id = 0;
        if (spec.segment)
        {
            eloqstore::MappingSnapshot::MappingTbl segment_tbl;
            for (eloqstore::FilePageId fp_id : spec.segment->fp_li)
            {
                segment_tbl.PushBack(
                    eloqstore::MappingSnapshot::EncodeFilePageId(fp_id));
            }
            segment_mapping = std::make_unique<eloqstore::MappingSnapshot>(
                nullptr, nullptr, std::move(segment_tbl));
            max_segment_fp_id = spec.segment->max_segment_fp_id;
        }
        record = builder.Snapshot(spec.root,
                                  spec.ttl_root,
                                  &mapping,
                                  spec.max_fp_id,
                                  spec.dict_bytes,
                                  spec.meta,
                                  segment_mapping.get(),
                                  max_segment_fp_id);
    }
    // Finalize 会把 record 补齐到 OS page size（macOS 16KiB / Linux 4KiB），
    // 该 padding 不参与 checksum，也不是格式语义的一部分。fixture 只保留
    // header + payload_len，保证跨平台可重复生成。
    const uint32_t payload_len = eloqstore::DecodeFixed32(
        record.data() + eloqstore::ManifestBuilder::offset_len);
    const size_t keep = eloqstore::ManifestBuilder::header_bytes + payload_len;
    return std::string(record.substr(0, keep));
}

// ---------------------------------------------------------------------------
// mapping / store 构建
// ---------------------------------------------------------------------------

std::string BuildMapping(std::string_view name)
{
    const MappingSpec spec = MappingSpecOf(name);
    eloqstore::MappingSnapshot::MappingTbl tbl;
    for (uint64_t value : spec.value_li)
    {
        tbl.PushBack(value);
    }
    // 与 manifest snapshot 相同：idx_mgr / tbl_id 传 nullptr，析构不回收。
    eloqstore::MappingSnapshot snapshot(nullptr, nullptr, std::move(tbl));
    eloqstore::ManifestBuffer buff;
    buff.AppendVarint32(static_cast<uint32_t>(spec.value_li.size()));
    snapshot.Serialize(buff);
    return std::string(buff.data(), buff.size());
}

std::string BuildStore(std::string_view name,
                       const eloqstore::KvOptions &options)
{
    const std::vector<StoreFile> file_li = StoreFilesOf(name, options);
    eloqstore::ManifestBuffer buff;
    buff.append(kStoreMagic.data(), kStoreMagic.size());
    buff.AppendVarint32(static_cast<uint32_t>(file_li.size()));
    for (const StoreFile &file : file_li)
    {
        buff.AppendVarint32(static_cast<uint32_t>(file.name.size()));
        buff.append(file.name.data(), file.name.size());
        buff.AppendVarint32(static_cast<uint32_t>(file.data.size()));
        buff.append(file.data.data(), file.data.size());
    }
    return std::string(buff.data(), buff.size());
}

// ---------------------------------------------------------------------------
// 落盘 / 读回
// ---------------------------------------------------------------------------

int WriteFile(const fs::path &path, std::string_view blob)
{
    std::ofstream file(path, std::ios::binary | std::ios::trunc);
    if (!file)
    {
        std::fprintf(stderr, "oracle: open %s failed\n", path.string().c_str());
        return kExitIo;
    }
    file.write(blob.data(), static_cast<std::streamsize>(blob.size()));
    if (!file)
    {
        std::fprintf(stderr, "oracle: write %s failed\n", path.string().c_str());
        return kExitIo;
    }
    return 0;
}

int WritePayload(const fs::path &out_dir, std::string_view blob,
                 std::string_view dict)
{
    std::error_code ec;
    fs::create_directories(out_dir, ec);
    if (ec)
    {
        std::fprintf(stderr, "oracle: create %s failed: %s\n",
                     out_dir.string().c_str(), ec.message().c_str());
        return kExitIo;
    }
    if (const int code = WriteFile(out_dir / kPayloadName, blob); code != 0)
    {
        return code;
    }
    if (!dict.empty())
    {
        return WriteFile(out_dir / kDictName, dict);
    }
    return 0;
}

int ReadFile(const fs::path &path, std::string *blob)
{
    std::ifstream file(path, std::ios::binary);
    if (!file)
    {
        std::fprintf(stderr, "oracle: open %s failed\n", path.string().c_str());
        return kExitIo;
    }
    blob->assign(std::istreambuf_iterator<char>(file),
                 std::istreambuf_iterator<char>());
    return 0;
}

// 把整页字节拷进 eloqstore Page（Page(true) 从独立 PagesPool 分配）。
std::pair<eloqstore::Page, bool> LoadPage(std::string_view blob)
{
    eloqstore::InitStandalone();
    eloqstore::Page page(true);
    eloqstore::ResetStandalone();
    if (page.Ptr() == nullptr || blob.size() > kPageSize)
    {
        return {std::move(page), false};
    }
    std::memset(page.Ptr(), 0, kPageSize);
    std::memcpy(page.Ptr(), blob.data(), blob.size());
    return {std::move(page), true};
}

int RunFixture(std::string_view kind, std::string_view name,
               const fs::path &out_dir)
{
    if (!KnownCase(kind, name))
    {
        std::fprintf(stderr,
                     "oracle: unsupported fixture kind/case `%.*s`/`%.*s`\n",
                     int(kind.size()), kind.data(), int(name.size()),
                     name.data());
        return kExitUnsupported;
    }

    std::string payload;
    std::string dict;
    if (kind == "page")
    {
        payload = BuildPage(name, eloqstore::KvOptions{});
    }
    else if (kind == "index")
    {
        payload = BuildIndex(name, eloqstore::KvOptions{});
    }
    else if (kind == "overflow")
    {
        payload = BuildOverflow(eloqstore::KvOptions{});
    }
    else if (kind == "manifest")
    {
        payload = BuildManifest(name);
    }
    else if (kind == "mapping")
    {
        payload = BuildMapping(name);
    }
    else if (kind == "store")
    {
        payload = BuildStore(name, eloqstore::KvOptions{});
    }
    else
    {
        CompressFixture fixture = BuildCompress(name);
        payload = std::move(fixture.payload);
        dict = std::move(fixture.dict);
    }
    if (payload.empty())
    {
        std::fprintf(stderr, "oracle: build failed for %.*s/%.*s\n",
                     int(kind.size()), kind.data(), int(name.size()),
                     name.data());
        return kExitInvalid;
    }
    std::printf(
        "oracle: kind=%.*s case=%.*s payload_bytes=%zu dict_bytes=%zu\n",
        int(kind.size()), kind.data(), int(name.size()), name.data(),
        payload.size(), dict.size());
    return WritePayload(out_dir, payload, dict);
}

// page / index 都带页头 checksum；先做统一校验。
int CheckPageChecksum(std::string_view blob)
{
    if (blob.size() != kPageSize)
    {
        std::fprintf(stderr, "oracle: page size %zu, expected %u\n",
                     blob.size(), kPageSize);
        return kExitInvalid;
    }
    if (!eloqstore::ValidateChecksum(blob))
    {
        std::fprintf(stderr, "oracle: page checksum invalid\n");
        return kExitInvalid;
    }
    return 0;
}

int VerifyPage(std::string_view name, const std::string &blob)
{
    if (const int code = CheckPageChecksum(blob); code != 0)
    {
        return code;
    }
    if (eloqstore::TypeOfPage(blob.data()) != eloqstore::PageType::Data)
    {
        std::fprintf(stderr, "oracle: not a data page\n");
        return kExitInvalid;
    }
    auto [page, loaded] = LoadPage(blob);
    if (!loaded)
    {
        return kExitInvalid;
    }
    eloqstore::KvOptions options;
    eloqstore::DataPage data_page(0, std::move(page));
    eloqstore::DataPageIter iter(&data_page, &options);
    const std::vector<Entry> want_li = PageEntriesOf(name);
    size_t count = 0;
    while (iter.HasNext())
    {
        if (!iter.Next())
        {
            return kExitInvalid;
        }
        if (count >= want_li.size())
        {
            std::fprintf(stderr, "oracle: extra entry at %zu\n", count);
            return kExitInvalid;
        }
        const Entry &want = want_li[count];
        if (iter.Key() != want.key || iter.Value() != want.value ||
            iter.Timestamp() != want.ts || iter.ExpireTs() != want.expire_ts ||
            iter.IsOverflow() != want.overflow ||
            iter.IsLargeValue() != want.large_value)
        {
            std::fprintf(stderr, "oracle: entry %zu mismatch\n", count);
            return kExitInvalid;
        }
        count++;
    }
    if (count != want_li.size())
    {
        std::fprintf(stderr, "oracle: entry count %zu, expected %zu\n", count,
                     want_li.size());
        return kExitInvalid;
    }
    std::printf("oracle: verify page case=%.*s entries=%zu ok\n", int(name.size()),
                name.data(), count);
    return 0;
}

// overflow kind：data entry 指针 + 逐页 checksum + overflow 页真实读回重组。
int VerifyOverflow(const std::string &blob)
{
    const OverflowSpec spec = OverflowSpecOf(eloqstore::KvOptions{});
    const size_t want_pages = 1 + spec.chunk_li.size();
    if (blob.size() != want_pages * kPageSize)
    {
        std::fprintf(stderr, "oracle: payload %zu bytes, expected %zu\n",
                     blob.size(), want_pages * size_t(kPageSize));
        return kExitInvalid;
    }

    const std::string_view data_view(blob.data(), kPageSize);
    if (!eloqstore::ValidateChecksum(data_view))
    {
        std::fprintf(stderr, "oracle: data page checksum invalid\n");
        return kExitInvalid;
    }
    auto [page, loaded] = LoadPage(data_view);
    if (!loaded)
    {
        return kExitInvalid;
    }
    eloqstore::KvOptions options;
    eloqstore::DataPage data_page(0, std::move(page));
    eloqstore::DataPageIter iter(&data_page, &options);
    if (!iter.HasNext() || !iter.Next() || iter.HasNext())
    {
        std::fprintf(stderr, "oracle: overflow data page entry count wrong\n");
        return kExitInvalid;
    }
    if (iter.Key() != kOverflowKey || iter.Value() != spec.pointers ||
        !iter.IsOverflow() || iter.Timestamp() != 100 || iter.ExpireTs() != 0)
    {
        std::fprintf(stderr, "oracle: overflow data entry mismatch\n");
        return kExitInvalid;
    }

    std::string rebuilt;
    for (size_t i = 0; i < spec.chunk_li.size(); i++)
    {
        const size_t off = (1 + i) * kPageSize;
        // 直接指向 blob 的视图；substr 的临时 string 会让 string_view 悬垂。
        const std::string_view slice(blob.data() + off, kPageSize);
        if (!eloqstore::ValidateChecksum(slice))
        {
            std::fprintf(stderr, "oracle: overflow page %zu checksum invalid\n",
                         i);
            return kExitInvalid;
        }
        auto [overflow_page, ok] = LoadPage(slice);
        if (!ok)
        {
            return kExitInvalid;
        }
        // 读构造断言页类型 == Overflow，并按真实布局取值。
        eloqstore::OverflowPage parsed(spec.chunk_li[i].page_id,
                                       std::move(overflow_page));
        if (parsed.GetValue() != spec.chunk_li[i].value ||
            !parsed.GetEncodedPointers(&options).empty())
        {
            std::fprintf(stderr, "oracle: overflow page %zu content mismatch\n",
                         i);
            return kExitInvalid;
        }
        rebuilt.append(parsed.GetValue());
    }
    if (rebuilt != spec.value)
    {
        std::fprintf(stderr, "oracle: overflow reassembly mismatch\n");
        return kExitInvalid;
    }
    std::printf("oracle: verify overflow pages=%zu bytes=%zu ok\n",
                spec.chunk_li.size(), rebuilt.size());
    return 0;
}

int VerifyIndex(std::string_view name, const std::string &blob)
{
    if (const int code = CheckPageChecksum(blob); code != 0)
    {
        return code;
    }
    const eloqstore::PageType type = eloqstore::TypeOfPage(blob.data());
    const eloqstore::PageType want_type =
        IndexIsLeaf(name) ? eloqstore::PageType::LeafIndex
                          : eloqstore::PageType::NonLeafIndex;
    if (type != want_type)
    {
        std::fprintf(stderr, "oracle: index page type mismatch\n");
        return kExitInvalid;
    }
    auto [page, loaded] = LoadPage(blob);
    if (!loaded)
    {
        return kExitInvalid;
    }
    eloqstore::KvOptions options;
    const uint16_t content_len = eloqstore::DecodeFixed16(
        page.Ptr() + eloqstore::MemCachedPage::page_size_offset);
    eloqstore::IndexPageIter iter(
        std::string_view(page.Ptr(), content_len), &options);
    // 第一个 Next() 是 leftmost 虚拟 entry（空 key）。
    if (!iter.Next() || iter.GetPageId() != IndexLeftmostOf(name) ||
        !iter.Key().empty())
    {
        std::fprintf(stderr, "oracle: leftmost entry mismatch\n");
        return kExitInvalid;
    }
    const std::vector<IndexEntry> want_li = IndexEntriesOf(name);
    size_t count = 0;
    while (iter.Next())
    {
        if (count >= want_li.size())
        {
            std::fprintf(stderr, "oracle: extra index entry at %zu\n", count);
            return kExitInvalid;
        }
        const IndexEntry &want = want_li[count];
        if (iter.Key() != want.key || iter.GetPageId() != want.page_id)
        {
            std::fprintf(stderr, "oracle: index entry %zu mismatch\n", count);
            return kExitInvalid;
        }
        count++;
    }
    if (count != want_li.size())
    {
        std::fprintf(stderr, "oracle: index entry count %zu, expected %zu\n",
                     count, want_li.size());
        return kExitInvalid;
    }
    std::printf("oracle: verify index case=%.*s entries=%zu ok\n",
                int(name.size()), name.data(), count);
    return 0;
}

int VerifyCompress(std::string_view name, const fs::path &case_dir,
                   const std::string &blob)
{
    std::string out;
    if (name == "raw")
    {
        if (!eloqstore::compression::DecompressRaw(blob, out) ||
            out != RawInput())
        {
            std::fprintf(stderr, "oracle: raw decompress mismatch\n");
            return kExitInvalid;
        }
    }
    else if (name == "dict")
    {
        std::string dict;
        if (const int code = ReadFile(case_dir / kDictName, &dict); code != 0)
        {
            return code;
        }
        eloqstore::compression::DictCompression decompressor;
        decompressor.LoadDictionary(std::move(dict));
        if (!decompressor.Decompress(blob, out) || out != DictInput())
        {
            std::fprintf(stderr, "oracle: dict decompress mismatch\n");
            return kExitInvalid;
        }
    }
    else
    {
        return kExitUnsupported;
    }
    std::printf("oracle: verify compress case=%.*s bytes=%zu ok\n",
                int(name.size()), name.data(), out.size());
    return 0;
}

bool SameBranchMetadata(const eloqstore::BranchManifestMetadata &lhs,
                        const eloqstore::BranchManifestMetadata &rhs)
{
    if (lhs.branch_name != rhs.branch_name || lhs.term != rhs.term ||
        lhs.file_ranges.size() != rhs.file_ranges.size())
    {
        return false;
    }
    for (size_t i = 0; i < lhs.file_ranges.size(); ++i)
    {
        const auto &left = lhs.file_ranges[i];
        const auto &right = rhs.file_ranges[i];
        if (left.branch_name_ != right.branch_name_ ||
            left.term_ != right.term_ ||
            left.max_file_id_ != right.max_file_id_ ||
            left.max_segment_file_id_ != right.max_segment_file_id_)
        {
            return false;
        }
    }
    return true;
}

// snapshot 布局（`ManifestBuilder::Snapshot`，eloqstore 82b774c root_meta.cpp）：
//   max_fp_id(varint64) | dict_len(varint32) | dict
//   | mapping_len(4B) | mapping values(varint64...)
//   | branch_len(4B) | branch metadata
//   | [max_segment_fp_id(varint64) | seg_len(4B) | segment values(varint64...)]
// 最后一段只在 segment_mapping != nullptr 时存在。
int VerifySnapshotManifest(const ManifestSpec &spec, std::string_view payload)
{
    std::string_view rest = payload;
    uint64_t max_fp_id = 0;
    if (!eloqstore::GetVarint64(&rest, &max_fp_id) ||
        max_fp_id != spec.max_fp_id)
    {
        std::fprintf(stderr, "oracle: snapshot max_fp_id mismatch\n");
        return kExitInvalid;
    }
    uint32_t dict_len = 0;
    if (!eloqstore::GetVarint32(&rest, &dict_len) || rest.size() < dict_len)
    {
        std::fprintf(stderr, "oracle: snapshot dict bytes out of bounds\n");
        return kExitInvalid;
    }
    if (std::string_view(rest.data(), dict_len) != spec.dict_bytes)
    {
        std::fprintf(stderr, "oracle: snapshot dict bytes mismatch\n");
        return kExitInvalid;
    }
    rest.remove_prefix(dict_len);

    if (rest.size() < sizeof(uint32_t))
    {
        std::fprintf(stderr, "oracle: snapshot mapping_len missing\n");
        return kExitInvalid;
    }
    const uint32_t mapping_len = eloqstore::DecodeFixed32(rest.data());
    rest.remove_prefix(sizeof(uint32_t));
    if (rest.size() < mapping_len)
    {
        std::fprintf(stderr, "oracle: snapshot mapping out of bounds\n");
        return kExitInvalid;
    }
    std::string_view mapping_view = rest.substr(0, mapping_len);
    rest.remove_prefix(mapping_len);
    size_t mapping_cnt = 0;
    while (!mapping_view.empty())
    {
        uint64_t value = 0;
        if (!eloqstore::GetVarint64(&mapping_view, &value) ||
            mapping_cnt >= spec.mapping_li.size() ||
            value != eloqstore::MappingSnapshot::EncodeFilePageId(
                         spec.mapping_li[mapping_cnt].second))
        {
            std::fprintf(stderr, "oracle: snapshot mapping value mismatch\n");
            return kExitInvalid;
        }
        ++mapping_cnt;
    }
    if (mapping_cnt != spec.mapping_li.size())
    {
        std::fprintf(stderr, "oracle: snapshot mapping count mismatch\n");
        return kExitInvalid;
    }

    if (rest.size() < sizeof(uint32_t))
    {
        std::fprintf(stderr, "oracle: snapshot branch_len missing\n");
        return kExitInvalid;
    }
    const uint32_t branch_len = eloqstore::DecodeFixed32(rest.data());
    rest.remove_prefix(sizeof(uint32_t));
    if (rest.size() < branch_len)
    {
        std::fprintf(stderr, "oracle: snapshot branch metadata out of bounds\n");
        return kExitInvalid;
    }
    eloqstore::BranchManifestMetadata meta;
    if (!eloqstore::DeserializeBranchManifestMetadata(
            rest.substr(0, branch_len), meta))
    {
        std::fprintf(stderr, "oracle: snapshot branch metadata decode failed\n");
        return kExitInvalid;
    }
    rest.remove_prefix(branch_len);
    if (!SameBranchMetadata(meta, spec.meta))
    {
        std::fprintf(stderr, "oracle: snapshot branch metadata mismatch\n");
        return kExitInvalid;
    }

    if (spec.segment)
    {
        uint64_t max_segment_fp_id = 0;
        if (!eloqstore::GetVarint64(&rest, &max_segment_fp_id) ||
            max_segment_fp_id != spec.segment->max_segment_fp_id)
        {
            std::fprintf(stderr, "oracle: snapshot max_segment_fp_id mismatch\n");
            return kExitInvalid;
        }
        if (rest.size() < sizeof(uint32_t))
        {
            std::fprintf(stderr, "oracle: snapshot segment_len missing\n");
            return kExitInvalid;
        }
        const uint32_t seg_len = eloqstore::DecodeFixed32(rest.data());
        rest.remove_prefix(sizeof(uint32_t));
        if (rest.size() < seg_len)
        {
            std::fprintf(stderr, "oracle: snapshot segment out of bounds\n");
            return kExitInvalid;
        }
        std::string_view seg_view = rest.substr(0, seg_len);
        rest.remove_prefix(seg_len);
        size_t seg_cnt = 0;
        while (!seg_view.empty())
        {
            uint64_t value = 0;
            if (!eloqstore::GetVarint64(&seg_view, &value) ||
                seg_cnt >= spec.segment->fp_li.size() ||
                value != eloqstore::MappingSnapshot::EncodeFilePageId(
                             spec.segment->fp_li[seg_cnt]))
            {
                std::fprintf(stderr, "oracle: snapshot segment value mismatch\n");
                return kExitInvalid;
            }
            ++seg_cnt;
        }
        if (seg_cnt != spec.segment->fp_li.size())
        {
            std::fprintf(stderr, "oracle: snapshot segment count mismatch\n");
            return kExitInvalid;
        }
    }
    else if (!rest.empty())
    {
        std::fprintf(stderr,
                     "oracle: snapshot has unexpected segment section\n");
        return kExitInvalid;
    }
    if (!rest.empty())
    {
        std::fprintf(stderr, "oracle: snapshot has trailing bytes\n");
        return kExitInvalid;
    }
    std::printf(
        "oracle: verify manifest case=snapshot branch=%s segment=%s ok\n",
        meta.branch_name.c_str(),
        spec.segment ? "present" : "absent");
    return 0;
}

int VerifyManifest(std::string_view name, const std::string &blob)
{
    if (blob.size() < eloqstore::ManifestBuilder::header_bytes)
    {
        return kExitInvalid;
    }
    const std::string_view raw(blob);
    const uint32_t root =
        eloqstore::DecodeFixed32(raw.data() +
                                 eloqstore::ManifestBuilder::offset_root);
    const uint32_t ttl_root =
        eloqstore::DecodeFixed32(raw.data() +
                                 eloqstore::ManifestBuilder::offset_ttl_root);
    const uint32_t payload_len =
        eloqstore::DecodeFixed32(raw.data() +
                                 eloqstore::ManifestBuilder::offset_len);
    const size_t record_len =
        size_t(payload_len) + eloqstore::ManifestBuilder::header_bytes;
    if (record_len > blob.size())
    {
        std::fprintf(stderr, "oracle: manifest payload out of bounds\n");
        return kExitInvalid;
    }
    // checksum 只覆盖 header + payload；Alignment padding 不参与（与真实读路径一致）。
    const std::string_view record = raw.substr(0, record_len);
    if (!eloqstore::ManifestBuilder::ValidateChecksum(record))
    {
        std::fprintf(stderr, "oracle: manifest checksum invalid\n");
        return kExitInvalid;
    }
    const ManifestSpec spec = ManifestSpecOf(name);
    if (root != spec.root || ttl_root != spec.ttl_root)
    {
        std::fprintf(stderr, "oracle: manifest header mismatch\n");
        return kExitInvalid;
    }
    if (name == "empty")
    {
        if (payload_len != 0)
        {
            std::fprintf(stderr, "oracle: empty manifest has payload\n");
            return kExitInvalid;
        }
        std::printf("oracle: verify manifest case=empty ok\n");
        return 0;
    }
    const char *payload_ptr =
        record.data() + eloqstore::ManifestBuilder::header_bytes;
    const std::string_view payload_view(payload_ptr, payload_len);
    if (spec.snapshot)
    {
        return VerifySnapshotManifest(spec, payload_view);
    }

    // log record: [mapping_len 4B][mapping bytes][branch_len 4B][branch bytes]
    const char *payload = payload_ptr;
    if (payload_len < 2 * sizeof(uint32_t))
    {
        return kExitInvalid;
    }
    const uint32_t mapping_len = eloqstore::DecodeFixed32(payload);
    const size_t branch_len_offset = sizeof(uint32_t) + mapping_len;
    if (branch_len_offset + sizeof(uint32_t) > payload_len)
    {
        std::fprintf(stderr, "oracle: manifest mapping/branch bounds\n");
        return kExitInvalid;
    }
    const uint32_t branch_len =
        eloqstore::DecodeFixed32(payload + branch_len_offset);
    if (branch_len_offset + sizeof(uint32_t) + branch_len > payload_len)
    {
        return kExitInvalid;
    }
    eloqstore::BranchManifestMetadata meta;
    if (!eloqstore::DeserializeBranchManifestMetadata(
            std::string_view(payload + branch_len_offset + sizeof(uint32_t),
                             branch_len),
            meta))
    {
        std::fprintf(stderr, "oracle: branch metadata decode failed\n");
        return kExitInvalid;
    }
    if (!SameBranchMetadata(meta, spec.meta))
    {
        std::fprintf(stderr, "oracle: branch metadata mismatch\n");
        return kExitInvalid;
    }
    std::printf(
        "oracle: verify manifest case=%.*s payload=%u branch=%s ok\n",
        int(name.size()), name.data(), payload_len, meta.branch_name.c_str());
    return 0;
}

int VerifyMapping(std::string_view name, const std::string &blob)
{
    const MappingSpec spec = MappingSpecOf(name);
    std::string_view rest(blob);
    uint32_t count = 0;
    if (!eloqstore::GetVarint32(&rest, &count))
    {
        std::fprintf(stderr, "oracle: mapping count missing\n");
        return kExitInvalid;
    }
    if (count != spec.value_li.size())
    {
        std::fprintf(stderr, "oracle: mapping count %u, expected %zu\n", count,
                     spec.value_li.size());
        return kExitInvalid;
    }
    for (size_t i = 0; i < spec.value_li.size(); i++)
    {
        uint64_t value = 0;
        if (!eloqstore::GetVarint64(&rest, &value) ||
            value != spec.value_li[i])
        {
            std::fprintf(stderr, "oracle: mapping value %zu mismatch\n", i);
            return kExitInvalid;
        }
    }
    if (!rest.empty())
    {
        std::fprintf(stderr, "oracle: mapping has trailing bytes\n");
        return kExitInvalid;
    }
    std::printf("oracle: verify mapping case=%.*s pages=%u ok\n",
                int(name.size()), name.data(), count);
    return 0;
}

int VerifyStore(std::string_view name, const std::string &blob,
                const eloqstore::KvOptions &options)
{
    const std::vector<StoreFile> expected_li = StoreFilesOf(name, options);
    std::string_view rest(blob);
    if (rest.size() < kStoreMagic.size() ||
        rest.substr(0, kStoreMagic.size()) != kStoreMagic)
    {
        std::fprintf(stderr, "oracle: store magic mismatch\n");
        return kExitInvalid;
    }
    rest.remove_prefix(kStoreMagic.size());
    uint32_t count = 0;
    if (!eloqstore::GetVarint32(&rest, &count))
    {
        std::fprintf(stderr, "oracle: store file count missing\n");
        return kExitInvalid;
    }
    if (count != expected_li.size())
    {
        std::fprintf(stderr, "oracle: store file count %u, expected %zu\n",
                     count, expected_li.size());
        return kExitInvalid;
    }
    for (size_t i = 0; i < expected_li.size(); i++)
    {
        uint32_t name_len = 0;
        if (!eloqstore::GetVarint32(&rest, &name_len) ||
            rest.size() < name_len)
        {
            std::fprintf(stderr, "oracle: store name %zu out of bounds\n", i);
            return kExitInvalid;
        }
        const std::string_view file_name = rest.substr(0, name_len);
        rest.remove_prefix(name_len);
        uint32_t data_len = 0;
        if (!eloqstore::GetVarint32(&rest, &data_len) ||
            rest.size() < data_len)
        {
            std::fprintf(stderr, "oracle: store data %zu out of bounds\n", i);
            return kExitInvalid;
        }
        const std::string_view data = rest.substr(0, data_len);
        rest.remove_prefix(data_len);
        const StoreFile &want = expected_li[i];
        if (file_name != want.name || data != want.data)
        {
            std::fprintf(stderr, "oracle: store file %zu mismatch\n", i);
            return kExitInvalid;
        }
        // 清单内容必须仍可被真实 parser 读回。
        if (file_name.starts_with("data_"))
        {
            if (data.size() != kPageSize ||
                !eloqstore::ValidateChecksum(data))
            {
                std::fprintf(stderr, "oracle: store data file %zu invalid\n",
                             i);
                return kExitInvalid;
            }
        }
        else if (file_name.starts_with("manifest_"))
        {
            if (!eloqstore::ManifestBuilder::ValidateChecksum(data))
            {
                std::fprintf(stderr, "oracle: store manifest %zu invalid\n",
                             i);
                return kExitInvalid;
            }
        }
        else if (file_name.starts_with("segment_") && data.empty())
        {
            std::fprintf(stderr, "oracle: store segment %zu empty\n", i);
            return kExitInvalid;
        }
    }
    if (!rest.empty())
    {
        std::fprintf(stderr, "oracle: store has trailing bytes\n");
        return kExitInvalid;
    }
    std::printf("oracle: verify store case=%.*s files=%u ok\n",
                int(name.size()), name.data(), count);
    return 0;
}

int VerifyCase(std::string_view kind, std::string_view name,
               const fs::path &case_dir)
{
    if (!KnownCase(kind, name))
    {
        std::fprintf(stderr, "oracle: unsupported verify kind/case `%.*s`/`%.*s`\n",
                     int(kind.size()), kind.data(), int(name.size()),
                     name.data());
        return kExitUnsupported;
    }
    std::string blob;
    if (const int code = ReadFile(case_dir / kPayloadName, &blob); code != 0)
    {
        return code;
    }
    if (kind == "page")
    {
        return VerifyPage(name, blob);
    }
    if (kind == "index")
    {
        return VerifyIndex(name, blob);
    }
    if (kind == "overflow")
    {
        return VerifyOverflow(blob);
    }
    if (kind == "compress")
    {
        return VerifyCompress(name, case_dir, blob);
    }
    if (kind == "mapping")
    {
        return VerifyMapping(name, blob);
    }
    if (kind == "store")
    {
        return VerifyStore(name, blob, eloqstore::KvOptions{});
    }
    return VerifyManifest(name, blob);
}

int Usage()
{
    std::fprintf(
        stderr,
        "usage:\n"
        "  fkv_oracle fixture --kind <K> --case <C> --out <DIR>\n"
        "  fkv_oracle verify  --case-dir <DIR> --kind <K> --case <C>\n"
        "kinds/cases:\n"
        "  page      empty single multi restart ttl large overflow\n"
        "  index     leaf branch\n"
        "  overflow  chain\n"
        "  manifest  empty checksum branch archive\n"
        "  compress  raw dict\n"
        "  mapping   empty single multi\n"
        "  store     empty single multi\n");
    return kExitUsage;
}

}  // namespace

int main(int argc, char **argv)
{
    std::string mode;
    std::string kind;
    std::string name;
    std::string out;
    std::string case_dir;
    for (int i = 1; i < argc; i++)
    {
        std::string arg = argv[i];
        const bool has_next = i + 1 < argc;
        if (arg == "fixture" || arg == "verify")
        {
            mode = arg;
        }
        else if (arg == "--kind" && has_next)
        {
            kind = argv[++i];
        }
        else if (arg == "--case" && has_next)
        {
            name = argv[++i];
        }
        else if (arg == "--out" && has_next)
        {
            out = argv[++i];
        }
        else if (arg == "--case-dir" && has_next)
        {
            case_dir = argv[++i];
        }
        else
        {
            std::fprintf(stderr, "oracle: unknown argument %s\n", arg.c_str());
            return Usage();
        }
    }

    if (mode == "fixture")
    {
        if (kind.empty() || name.empty() || out.empty())
        {
            return Usage();
        }
        return RunFixture(kind, name, fs::path(out));
    }
    if (mode == "verify")
    {
        if (case_dir.empty() || kind.empty() || name.empty())
        {
            return Usage();
        }
        return VerifyCase(kind, name, fs::path(case_dir));
    }
    return Usage();
}
