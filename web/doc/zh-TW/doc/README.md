# eloqstore2fkv

eloqstore 到 fkv 的遷移與相容驗證倉庫。

fkv 主倉保持純 Rust、獨立建置；本倉保留 C++ 參考實作 oracle、雙向位元組語料、
遷移帳本、歷史審查記錄與驗證 CI。

## 內容

- C++ oracle：連結 eloqstore 格式層，產生 page / index / overflow / manifest /
  compress 語料。
- 雙向語料：C++ 寫 → Rust 讀，Rust 寫 → C++ 讀。
- `fkv_compat`：Rust 側相容測試，依賴 fkv 倉的 format crate。
- `fkv_ledger`：掃描 eloqstore、記錄遷移帳本、產生與校驗語料。
- 效能比較：把 fkv 與 C++ 基準放到同一套 workload 下比較。

## 入口

- [驗證流程](workflow.md)
- [C++ oracle](oracle.md)
- [遷移帳本](ledger.md)
- [效能](performance.md)
- [審查記錄](issues.md)