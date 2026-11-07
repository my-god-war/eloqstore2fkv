# Hiệu suất

Tải xuống `perf-latest` của `fkv_bench` từ kho lưu trữ fkv và chạy cùng một khối lượng công việc trên một máy cố định.
Và so sánh thông lượng, lượng tử độ trễ và RSS cao nhất với đường cơ sở C++.

Nguồn tham số phù hợp với fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

Đường cơ sở C++ được điều hành bởi môi trường oracle `simple_bench` và sau đó xuất ra theo cùng một lược đồ và được xuất bản lên
`perf-baseline` giải phóng nội dung. Các kết quả so sánh được ghi lại vào tài liệu kho và trang web trực tiếp hiển thị Markdown.
Không có lớp dữ liệu JSON nào được duy trì.

## Kết quả hiện tại

Đường cơ sở của lược đồ 2 cho 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, trang 4 KiB,
`data_append_mode=false`, `direct_io=true`. Khối lượng công việc cố định:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| dự án | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Đặt thông lượng | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Nhận thông lượng | 5,526 QPS | 7,293 QPS | 1.32x |
| Đặt băng thông logic | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Nhận băng thông hợp lý | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| RSS cao điểm | 175.6 MiB | 145.5 MiB | 0.83x |
| Mức sử dụng bộ nhớ | 269.0 MiB | 101.4 MiB | 0.38x |
| Lưu trữ / dữ liệu danh nghĩa | 2.20x | 0.83x | 0.38x |

Viết 1 triệu mục (10.000 đợt) tương ứng với
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, nghĩa là dữ liệu được đồng bộ hóa một lần mỗi đợt.
Chỉ số bổ sung của Rust: Quét `141,166 kv/s`, Quét toàn bộ `108,274 kv/s`, Khuếch đại ghi `2.57x`
(`proc_io_write` 329 MB / 128 MB ghi logic), `4.86` ghi lệnh gọi hệ thống mỗi đợt.
Tỷ lệ lưu trữ được tính toán dựa trên dữ liệu danh nghĩa 1M × 128B = 128 MB; C++ không kích hoạt GC/compact lần này,
Vì vậy, nó chứa nhiều trang lịch sử hơn. Nó không có cùng cỡ nòng với "khuếch đại không gian dữ liệu sinh tồn".
Độ trễ đuôi P99.9 của Rust vẫn kém hơn C++ (đặt 223,3 ms so với 86,0 ms, nhận 355,5 ms so với
48,2 ms), đường cơ sở duy nhất dao động rất lớn, vui lòng xem báo cáo hiệu suất nội bộ để biết chi tiết.

**Lưu ý**: `write-read` của C++ có nghĩa là luồng ghi và luồng đọc chạy cùng lúc và Rust hiện tại
“Viết xong, mở lại, đọc lại”, ở khâu đọc không có thi viết. Do đó, số nhân của `write-read` không thể được coi là
Kết luận đọc thuần túy / viết thuần túy; so sánh chặt chẽ nên xem `scripts/perf_local.sh --symmetric`
Hai cặp kết quả được đưa ra: chỉ ghi và chỉ đọc. Số chạy chung được chia sẻ duy nhất chỉ được sử dụng cho hồi quy
Xu hướng, bản phát hành được đánh giá bằng cách sử dụng mức trung vị của `--runs`.