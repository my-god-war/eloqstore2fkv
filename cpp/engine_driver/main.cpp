// 最小 C++ 引擎驱动器：用真实 EloqStore 写入/校验一个表，供 Rust 侧交叉读取。
//
// 用法：
//   engine_driver write        <root> <table> <prefix> <count> <value>
//   engine_driver verify       <root> <table> <prefix> <count> <value>
//   engine_driver write_repeat  <root> <table> <prefix> <count> <size> <byte>
//   engine_driver verify_repeat <root> <table> <prefix> <count> <size> <byte>
//
// key 固定为 `<prefix>%04d`；write/verify 的 value 固定为 `<value>`，
// write_repeat/verify_repeat 的 value 为 `<size>` 个 `<byte>`（用于 overflow 页）。
// Rust 侧用同一规则。只用于兼容性测试，不进入生产路径。

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <algorithm>
#include <fstream>
#include <string>
#include <string_view>
#include <vector>

#include "eloq_store.h"
#include "kv_options.h"

namespace
{

constexpr uint16_t kPageSize = 4 * 1024;

std::string KeyOf(std::string_view prefix, uint64_t index)
{
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%.*s%04llu",
                  static_cast<int>(prefix.size()), prefix.data(),
                  static_cast<unsigned long long>(index));
    return buf;
}

uint64_t ParseU64(const char *text)
{
    char *end = nullptr;
    unsigned long long value = std::strtoull(text, &end, 10);
    if (end == text || *end != '\0')
    {
        std::fprintf(stderr, "invalid number: %s\n", text);
        std::exit(2);
    }
    return static_cast<uint64_t>(value);
}

std::pair<std::string, uint64_t> ParseTable(const char *text)
{
    std::string_view view(text);
    size_t dot = view.rfind('.');
    if (dot == std::string_view::npos || dot == 0 || dot + 1 == view.size())
    {
        std::fprintf(stderr, "table must be <name>.<shard>: %s\n", text);
        std::exit(2);
    }
    std::string name(view.substr(0, dot));
    uint64_t shard = ParseU64(std::string(view.substr(dot + 1)).c_str());
    return {name, shard};
}

int Fail(const char *what, eloqstore::KvError err)
{
    std::fprintf(stderr, "%s failed: err=%d\n", what,
                 static_cast<int>(err));
    return 1;
}

std::string Hex(std::string_view bytes)
{
    static constexpr char kDigits[] = "0123456789abcdef";
    std::string out;
    out.reserve(bytes.size() * 2);
    for (unsigned char byte : bytes)
    {
        out.push_back(kDigits[byte >> 4]);
        out.push_back(kDigits[byte & 0xf]);
    }
    return out;
}

struct Op
{
    bool del{false};
    std::string key;
    std::string value;
    uint64_t ts{0};
};

std::vector<Op> ReadOps(const char *path)
{
    std::ifstream in(path);
    if (!in)
    {
        std::fprintf(stderr, "cannot open oplog: %s\n", path);
        std::exit(2);
    }
    std::vector<Op> ops;
    std::string line;
    while (std::getline(in, line))
    {
        if (line.empty() || line[0] == '#')
        {
            continue;
        }
        std::string_view view(line);
        auto next = [&view]() {
            size_t pos = view.find(' ');
            std::string token(view.substr(0, pos));
            view.remove_prefix(pos == std::string_view::npos ? view.size() : pos + 1);
            return token;
        };
        std::string op = next();
        Op entry;
        entry.del = (op == "del" || op == "delete");
        entry.key = next();
        if (entry.del)
        {
            entry.ts = ParseU64(next().c_str());
        }
        else
        {
            entry.value = next();
            entry.ts = ParseU64(next().c_str());
        }
        ops.push_back(std::move(entry));
    }
    return ops;
}

}  // namespace

int main(int argc, char **argv)
{
    if (argc < 2)
    {
        std::fprintf(stderr,
                     "usage: %s write|verify|write_repeat|verify_repeat|apply|dump"
                     " ...\n",
                     argv[0]);
        return 2;
    }
    std::string mode = argv[1];
    bool oplog_mode = (mode == "apply" || mode == "dump");
    bool repeat_mode = (mode == "write_repeat" || mode == "verify_repeat");
    // apply/dump: <root> <table> <file>；write/verify: <root> <table> <prefix>
    // <count> <value>；*_repeat 多一个 <size> <byte>。
    const int expected_argc = oplog_mode ? 5 : (repeat_mode ? 8 : 7);
    if (argc != expected_argc)
    {
        std::fprintf(stderr, "invalid argument count for %s\n", mode.c_str());
        return 2;
    }
    std::string root = argv[2];
    auto [table, shard] = ParseTable(argv[3]);

    eloqstore::KvOptions opts;
    opts.store_path = {root};
    opts.num_threads = 1;
    opts.data_page_size = kPageSize;
    // 其余字段保持 KvOptions 默认值：data_append_mode=false（PooledFilePages）。
    // ENGINE_APPEND_MODE=1 时切到 append 分配（与 Rust 侧 FKV_ENGINE_APPEND_MODE 对齐），
    // 让同一份引擎交叉同时覆盖两种分配模式。
    if (const char *append = std::getenv("ENGINE_APPEND_MODE");
        append != nullptr && append[0] != '\0' && append[0] != '0')
    {
        opts.data_append_mode = true;
    }

    eloqstore::TableIdent tbl(table, shard);
    eloqstore::EloqStore store(opts);
    eloqstore::KvError err = store.Start("main", 0);
    if (err != eloqstore::KvError::NoError)
    {
        return Fail("start", err);
    }

    int rc = 0;
    if (mode == "apply")
    {
        for (const Op &op : ReadOps(argv[4]))
        {
            eloqstore::BatchWriteRequest req;
            std::vector<eloqstore::WriteDataEntry> entries;
            entries.emplace_back(op.key, op.value, op.ts,
                                 op.del ? eloqstore::WriteOp::Delete
                                        : eloqstore::WriteOp::Upsert);
            req.SetArgs(tbl, std::move(entries));
            store.ExecSync(&req);
            if (req.Error() != eloqstore::KvError::NoError)
            {
                rc = Fail("apply", req.Error());
                break;
            }
        }
    }
    else if (mode == "dump")
    {
        eloqstore::ScanRequest req;
        req.SetArgs(tbl, "", "");
        store.ExecSync(&req);
        if (req.Error() != eloqstore::KvError::NoError)
        {
            rc = Fail("scan", req.Error());
        }
        else
        {
            auto span = req.Entries();
            std::vector<eloqstore::KvEntry> entries(span.begin(), span.end());
            std::sort(entries.begin(), entries.end(),
                      [](const eloqstore::KvEntry &a, const eloqstore::KvEntry &b) {
                          return a.key_ < b.key_;
                      });
            std::ofstream out(argv[4]);
            for (const auto &entry : entries)
            {
                out << Hex(entry.key_) << '\t' << Hex(entry.value_) << '\n';
            }
        }
    }
    else if (mode == "write")
    {
        std::string prefix = argv[4];
        uint64_t count = ParseU64(argv[5]);
        std::string value = argv[6];
        std::vector<eloqstore::WriteDataEntry> entries;
        entries.reserve(count);
        for (uint64_t i = 0; i < count; ++i)
        {
            entries.emplace_back(KeyOf(prefix, i), value, 1,
                                 eloqstore::WriteOp::Upsert);
        }
        eloqstore::BatchWriteRequest req;
        req.SetArgs(tbl, std::move(entries));
        store.ExecSync(&req);
        if (req.Error() != eloqstore::KvError::NoError)
        {
            rc = Fail("write", req.Error());
        }
    }
    else if (mode == "verify")
    {
        std::string prefix = argv[4];
        uint64_t count = ParseU64(argv[5]);
        std::string value = argv[6];
        for (uint64_t i = 0; i < count; ++i)
        {
            std::string key = KeyOf(prefix, i);
            eloqstore::ReadRequest req;
            req.SetArgs(tbl, key);
            store.ExecSync(&req);
            if (req.Error() != eloqstore::KvError::NoError)
            {
                rc = Fail(("read " + key).c_str(), req.Error());
                break;
            }
            if (req.value_ != value)
            {
                std::fprintf(stderr, "value mismatch key=%s got=%s want=%s\n",
                             key.c_str(), req.value_.c_str(), value.c_str());
                rc = 1;
                break;
            }
        }
    }
    else if (mode == "write_repeat")
    {
        std::string prefix = argv[4];
        uint64_t count = ParseU64(argv[5]);
        uint64_t size = ParseU64(argv[6]);
        std::string byte_arg = argv[7];
        if (byte_arg.size() != 1)
        {
            std::fprintf(stderr, "<byte> must be a single character: %s\n",
                         byte_arg.c_str());
            rc = 2;
        }
        else
        {
            // 重复同一字节的大 value：超过单页容量时由引擎写入 overflow 页链。
            std::string value(size, byte_arg[0]);
            std::vector<eloqstore::WriteDataEntry> entries;
            entries.reserve(count);
            for (uint64_t i = 0; i < count; ++i)
            {
                entries.emplace_back(KeyOf(prefix, i), value, 1,
                                     eloqstore::WriteOp::Upsert);
            }
            eloqstore::BatchWriteRequest req;
            req.SetArgs(tbl, std::move(entries));
            store.ExecSync(&req);
            if (req.Error() != eloqstore::KvError::NoError)
            {
                rc = Fail("write_repeat", req.Error());
            }
        }
    }
    else if (mode == "verify_repeat")
    {
        std::string prefix = argv[4];
        uint64_t count = ParseU64(argv[5]);
        uint64_t size = ParseU64(argv[6]);
        std::string byte_arg = argv[7];
        if (byte_arg.size() != 1)
        {
            std::fprintf(stderr, "<byte> must be a single character: %s\n",
                         byte_arg.c_str());
            rc = 2;
        }
        else
        {
            const char byte = byte_arg[0];
            for (uint64_t i = 0; i < count; ++i)
            {
                std::string key = KeyOf(prefix, i);
                eloqstore::ReadRequest req;
                req.SetArgs(tbl, key);
                store.ExecSync(&req);
                if (req.Error() != eloqstore::KvError::NoError)
                {
                    rc = Fail(("read " + key).c_str(), req.Error());
                    break;
                }
                // 只报告长度 / 内容判定，避免把 100 KiB 的 value 打进日志。
                if (req.value_.size() != size)
                {
                    std::fprintf(stderr,
                                 "value size mismatch key=%s got=%zu want=%llu\n",
                                 key.c_str(), req.value_.size(),
                                 static_cast<unsigned long long>(size));
                    rc = 1;
                    break;
                }
                if (!std::all_of(req.value_.begin(), req.value_.end(),
                                 [byte](char got) { return got == byte; }))
                {
                    std::fprintf(stderr, "value content mismatch key=%s\n",
                                 key.c_str());
                    rc = 1;
                    break;
                }
            }
        }
    }
    else
    {
        std::fprintf(stderr, "unknown mode: %s\n", mode.c_str());
        rc = 2;
    }

    store.Stop();
    return rc;
}
