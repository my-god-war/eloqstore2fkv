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