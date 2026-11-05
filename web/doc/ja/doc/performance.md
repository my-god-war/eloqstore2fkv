# パフォーマンス

fkv リポジトリから `perf-latest` または `fkv_bench` をダウンロードし、固定マシンで同じワークロードを実行します。
また、スループット、レイテンシ分位数、およびピーク RSS を C++ ベースラインと比較します。

パラメータ ソースは fkv と一致しています。

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ ベースラインは Oracle 環境`simple_bench`によって実行され、同じスキーマに従って出力され、
`perf-baseline` アセットをリリースします。比較結果はウェアハウス ドキュメントに書き戻され、サイトは Markdown を直接レンダリングします。
JSON データ層は維持されません。