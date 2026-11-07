# eloqstore2fkv

Kho xác minh di chuyển và tương thích từ eloqstore sang fkv.

Kho lưu trữ fkv chính vẫn là Rust thuần túy và được xây dựng độc lập; kho lưu trữ này giữ lại lời tiên tri triển khai tham chiếu C++, kho lưu trữ byte hai chiều,
Di chuyển sổ cái, hồ sơ kiểm toán lịch sử và xác minh CI.

## nội dung

- C++ oracle: liên kết lớp định dạng eloqstore và tạo trang/index/overflow/manifest/
  nén tử thi.
- Kho dữ liệu hai chiều: C++ viết → Rust đọc, Rust viết → C++ đọc.
- `fkv_compat`: Kiểm tra khả năng tương thích bên rỉ sét, dựa vào thùng định dạng của kho fkv.
- `fkv_ledger`: Quét eloqstore, ghi lại sổ cái di chuyển, tạo và xác minh kho văn bản.
- So sánh hiệu suất: So sánh đường cơ sở của fkv và C++ trong cùng một khối lượng công việc.

## Cổng vào

- [Quá trình xác minh](workflow.md)
- [C++ oracle](oracle.md)
- [Di chuyển sổ cái](ledger.md)
- [hiệu suất](performance.md)
- [Xem xét hồ sơ](issues.md)