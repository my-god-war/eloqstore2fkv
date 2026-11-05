# Desempenho

Baixe `perf-latest` de `fkv_bench` do repositório fkv e execute a mesma carga de trabalho em uma máquina fixa.
E compare a taxa de transferência, os quantis de latência e o pico de RSS com uma linha de base C++.

A origem do parâmetro é consistente com fkv:

- `PERF_ROWS`
- `PERF_BATCH_SIZE`
- `PERF_WORKLOAD`
- `PERF_BENCH_ROOT`

A linha de base C++ é executada pelo ambiente oracle `simple_bench` e então gerada de acordo com o mesmo esquema e publicada em
`perf-baseline` liberar recurso. Os resultados da comparação são gravados no documento do warehouse e o site renderiza diretamente o Markdown.
Nenhuma camada de dados JSON é mantida.