# 性能

从 fkv 仓下载 `perf-latest` 的 `fkv_bench`，在固定机器上跑同一 workload，
并与 C++ 基线比较吞吐、延迟分位与峰值 RSS。

参数来源与 fkv 保持一致：

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

C++ 基线由 oracle 环境跑 `simple_bench` 后按同一 schema 产出，发布到
`perf-baseline` release asset。对比结果写回本仓文档，站点直接渲染 Markdown，
不维护 JSON 数据层。
