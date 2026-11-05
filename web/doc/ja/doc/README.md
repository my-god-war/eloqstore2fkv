# eloqstore2fkv

eloqstore から fkv への移行および互換性検証ウェアハウス。

メインのfkvリポジトリは純粋なRustのままで、独立して構築されています。このリポジトリには、C++ リファレンス実装 Oracle、双方向バイト コーパス、
台帳、履歴監査記録を移行し、CI を検証します。

## コンテンツ

- C++ oracle: eloqstore フォーマット層をリンクし、page/index/overflow/manifest/ を生成します。
  コーパスを圧縮します。
- 双方向コーパス: C++ 書き込み → Rust 読み取り、Rust 書き込み → C++ 読み取り。
- `fkv_compat`: Rust 側の互換性テスト。fkv ウェアハウスのフォーマット クレートに依存します。
- `fkv_ledger`: eloqstore をスキャンし、移行台帳を記録し、コーパスを生成して検証します。
- パフォーマンスの比較: 同じワークロードの下で、fkv ベースラインと C++ ベースラインを比較します。

## 入り口

- [検証プロセス](workflow.md)
- [C++ oracle](oracle.md)
- [台帳の移行](ledger.md)
- [パフォーマンス](performance.md)
- [レビュー記録](issues.md)