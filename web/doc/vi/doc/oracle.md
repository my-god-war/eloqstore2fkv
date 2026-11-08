# C++ oracle

`cpp/oracle` Sử dụng trình tạo/phân tích cú pháp thực sự của eloqstore để tạo kho dữ liệu byte.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | trống/đơn/đa (giữ chỗ) |
| store | trống/đơn/đa (giữ chỗ) |

## giao diện

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Mã thoát bằng 0 được coi là thành công. `verify` phải được thông qua một cách rõ ràng `kind` / `case`.

## Xây dựng phần phụ thuộc

Vùng chứa Linux yêu cầu `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle sẽ sử dụng mã nguồn abseil được lưu trữ trong mã pin eloqstore để xây dựng một thư viện tĩnh.