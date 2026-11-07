---
date: 2026-11-06
summary: 首页性能对比、评测指标扩展、GitHub Pages 与多语言博客上线
---

# 2026-11-06：首页性能表、指标扩展与 Pages

今天的更新集中在性能评测和站点发布。

## 首页性能对比

- 首页改成性能优先，第一屏直接展示 C++ / Rust 对比表。
- 新增逻辑带宽、P99.9 尾延迟、空间放大三组指标。
- 增加 Rust 运行细节：Scan、Fullscan、写放大、每批写系统调用与数据同步次数。
- 明确标注 `write-read` 为混合口径，并链接到性能文档。

当前同机基线：

| 指标 | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| 峰值 RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| 存储占用 | 269.0 MiB | 101.4 MiB | 0.38x |

## 评测指标

参考 RocksDB、LevelDB、YCSB、WiredTiger 等基准的常见输出，补了：

- 逻辑读写带宽 MiB/s；
- P99.9 尾延迟；
- 空间放大、写放大；
- 每批写系统调用与数据同步次数；
- Scan / Fullscan 吞吐；
- CPU、页错误、上下文切换与进程 IO 继续由资源侧车采集。

同时退役了 `gxhash_4096B_gib_s`。它是 4 KiB 单块哈希微基准，不在 KV 引擎路径上，也没有 C++ 对应指标。

## 正确性与工程修复

- 修复 `--rng cpp` 下读 / scan 的 key 空间：读探测不再乘 `key_interval`，避免 75% 的读落到未写入的 key。
- 修复 macOS `openrsync` 对同内容 manifest 误判导致的 standby 重试。
- 修复 QEMU 虚拟盘被误判为本地 SSD 的硬件门问题。

## 站点与发布

- GitHub Pages 已启用：<https://my-god-war.github.io/eloqstore2fkv/>
- 站点路由支持 `/eloqstore2fkv/` 子路径。
- 文档改为从 `raw.githubusercontent.com` 读取，修复 jsDelivr 路径缺少 `web/` 导致的 404。
- 博客导航和 `bun run tran` 多语言翻译已接入；本文由中文源自动翻译到其他语言。

后续更新日志会继续记录兼容性、性能和工具链变更。
