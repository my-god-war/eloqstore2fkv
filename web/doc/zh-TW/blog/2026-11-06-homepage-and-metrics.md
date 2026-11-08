---
date: 2026-11-06
summary: 首頁效能比較、評測指標擴充、GitHub Pages 與多語言部落格上線
---

# 2026-11-06：首頁效能表、指標擴充與 Pages

今天的更新集中在效能評測和網站發布。

## 首頁效能對比

- 首頁改成效能優先，第一畫面直接展示 C++ / Rust 對比表。
- 新增邏輯頻寬、P99.9 尾延遲、空間放大三組指標。
- 增加 Rust 運作細節：Scan、Fullscan、寫入放大、每批寫系統呼叫與資料同步次數。
- 明確標註 `write-read` 為混合口徑，並連結至效能文件。

目前同機基線：

| 指標 | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| 峰值 RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| 儲存佔用 | 269.0 MiB | 101.4 MiB | 0.38x |

## 評測指標

參考 RocksDB、LevelDB、YCSB、WiredTiger 等基準的常見輸出，補充了：

- 邏輯讀寫頻寬 MiB/s；
- P99.9 尾延遲；
- 空間放大、寫放大；
- 每批寫系統呼叫與資料同步次數；
- Scan / Fullscan 吞吐；
- CPU、頁錯誤、上下文切換與進程 IO 繼續由資源側車採集。

同時退役了 `gxhash_4096B_gib_s`。它是 4 KiB 單塊哈希微基準，不在 KV 引擎路徑上，也沒有 C++ 對應指標。

## 正確性與工程修復

- 修復 `--rng cpp` 下讀 / scan 的 key 空間：讀取探測不再乘 `key_interval`，避免 75% 的讀落到未寫入的 key。
- 修正 macOS `openrsync` 同內容 manifest 誤判導致的 standby 重試。
- 修正 QEMU 虛擬磁碟被誤判為本地 SSD 的硬體門問題。

## 網站與發布

- GitHub Pages 已啟用：<https://my-god-war.github.io/eloqstore2fkv/>
- 站點路由支援 `/eloqstore2fkv/` 子路徑。
- 文件改為從 `raw.githubusercontent.com` 讀取，修復 jsDelivr 路徑缺少 `web/` 導致的 404。
- 部落格導航和 `bun run tran` 多語言翻譯已存取；本文由中文來源自動翻譯到其他語言。

後續更新日誌會繼續記錄相容性、效能和工具鏈變更。