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

## Resultados atuais

linha de base do esquema 2 para 2026-11-06T04:25Z,`github-ubuntu-latest-x86_64`, página de 4 KiB,
`data_append_mode=false`, `direct_io=true`. Carga de trabalho fixa:
`rows/max_key=1000000`,`batch_size=100`,`write_batchs=10000`,
`read_per_part=4`,`partitions=1`,`kv_size=128`,`rng=cpp`
(`key_interval=4` / `del_ratio=4`), `test_secs=30`.

| projeto | C++ EloqStore | Rust fkv | Rust / C++ |
|---|---:|---:|---:|
| Colocar rendimento | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Obtenha rendimento | 5,526 QPS | 7,293 QPS | 1.32x |
| Coloque largura de banda lógica | 2.06 MiB/s | 2.61 MiB/s | 1.27x |
| Obtenha largura de banda lógica | 0.67 MiB/s | 0.89 MiB/s | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pico de RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de armazenamento | 269.0 MiB | 101.4 MiB | 0.38x |
| Armazenamento/dados nominais | 2.20x | 0.83x | 0.38x |

Escrever 1 milhão de entradas (10.000 lotes) corresponde a
`io_write_at_num=48,625`, `io_data_sync_num=10,000`, ou seja, os dados são sincronizados uma vez por lote.
Métricas adicionais de ferrugem: varredura `141,166 kv/s`, varredura completa `108,274 kv/s`, amplificação de gravação `2.57x`
(`proc_io_write` 329 MB/128 MB de gravações lógicas), `4.86` grava chamadas de sistema por lote.
A taxa de armazenamento é calculada com base em 1M × 128B = 128 MB de dados nominais; C++ não aciona GC/compact desta vez,
Portanto, contém mais páginas de história. Não é o mesmo número de calibre que “amplificação do espaço de dados de sobrevivência”.
A latência final P99.9 do Rust ainda é pior que a do C++ (coloque 223,3 ms vs 86,0 ms, obtenha 355,5 ms vs.
48,2 ms), a linha de base única flutua muito. Consulte o relatório de desempenho interno para obter detalhes.

**Nota**: `write-read` do C++ significa que o thread de escrita e o thread de leitura são executados ao mesmo tempo, e Rust atualmente
“Terminar a redação, reabrir e ler novamente”, não há concurso de redação na etapa de leitura. Portanto o multiplicador de `write-read` não pode ser tratado como
Conclusão de leitura pura/escrita pura; comparação estrita deve ver `scripts/perf_local.sh --symmetric`
Dois pares de resultados são gerados: somente gravação e somente leitura. Números de executores compartilhados únicos são usados ​​apenas para regressão
Tendências e lançamentos são avaliados usando a mediana de `--runs`.