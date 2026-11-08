---
date: 2026-11-06
summary: So sánh hiệu suất trang chủ, mở rộng chỉ số đánh giá, Trang GitHub và blog đa ngôn ngữ trực tuyến
---

# 2026-11-06: Bảng hiệu suất trang chủ, mở rộng chỉ báo và Trang

Bản cập nhật hôm nay tập trung vào đánh giá hiệu suất và ra mắt trang web.

## So sánh hiệu suất trang chủ

- Trang chủ được thay đổi thành ưu tiên hiệu suất và bảng so sánh C++/Rust được hiển thị trực tiếp trên màn hình đầu tiên.
- Ba bộ chỉ báo mới được thêm vào: băng thông logic, độ trễ đuôi P99.9 và khuếch đại không gian.
- Đã thêm chi tiết chạy Rust: Quét, Quét toàn bộ, khuếch đại ghi, số lượng lệnh gọi hệ thống ghi và đồng bộ hóa dữ liệu mỗi đợt.
- Đánh dấu rõ ràng `write-read` là cỡ nòng hỗn hợp và liên kết đến tài liệu hiệu suất.

Đường cơ sở của cùng một máy hiện tại:

| chỉ mục | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS cao điểm | 175.6 MiB | 145.5 MiB | 0.83x |
| Mức sử dụng bộ nhớ | 269.0 MiB | 101.4 MiB | 0.38x |

## Chỉ số đánh giá

Đề cập đến kết quả chung của RocksDB, LevelDB, YCSB, WiredTiger và các điểm chuẩn khác, tôi đã thêm:

- Băng thông đọc và ghi logic MiB/s;
- Độ trễ đuôi P99.9;
- Khuếch đại không gian, khuếch đại văn bản;
- Số lần ghi lệnh gọi hệ thống và đồng bộ hóa dữ liệu trong mỗi đợt;
- Thông lượng quét / quét toàn bộ;
- CPU, lỗi trang, chuyển ngữ cảnh và tiến trình IO tiếp tục được thu thập bởi sidecar tài nguyên.

Đã nghỉ hưu cùng lúc`gxhash_4096B_gib_s`. Đó là điểm chuẩn vi mô băm đơn khối 4 KiB không nằm trên đường dẫn công cụ KV và không có đối tác C++.

## Sửa lỗi chính xác và kỹ thuật

- Sửa không gian khóa của `--rng cpp` đọc/quét: tính năng phát hiện đọc không còn được nhân với `key_interval` để tránh 75% số lần đọc rơi vào các khóa chưa được ghi.
- Đã sửa lỗi thử lại ở chế độ chờ do macOS `openrsync` đánh giá sai các tệp kê khai có cùng nội dung.
- Đã khắc phục sự cố cổng phần cứng trong đó ổ đĩa ảo QEMU bị đánh giá sai là ổ SSD cục bộ.

## Trang web và xuất bản

- Đã bật trang GitHub: <https://my-god-war.github.io/eloqstore2fkv/>
- Định tuyến trang web hỗ trợ các đường dẫn phụ `/eloqstore2fkv/`.
- Thay vào đó, tài liệu được đọc từ `raw.githubusercontent.com` và lỗi 404 do thiếu `web/` trong đường dẫn jsDelivr đã được sửa.
- Điều hướng blog và `bun run tran` dịch đa ngôn ngữ được tích hợp; bài viết này được dịch tự động từ nguồn tiếng Trung sang các ngôn ngữ khác.

Các nhật ký thay đổi tiếp theo sẽ tiếp tục ghi lại các thay đổi về khả năng tương thích, hiệu suất và chuỗi công cụ.