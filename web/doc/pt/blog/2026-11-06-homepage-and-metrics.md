---
date: 2026-11-06
summary: Comparação de desempenho da página inicial, expansão do indicador de avaliação, páginas GitHub e blogs multilíngues online
---

# 06/11/2026: Tabela de desempenho da página inicial, expansão de indicadores e páginas

A atualização de hoje se concentra em análises de desempenho e lançamentos de sites.

## Comparação de desempenho da página inicial

- A página inicial é alterada para prioridade de desempenho e a tabela de comparação C++/Rust é exibida diretamente na primeira tela.
- Três novos conjuntos de indicadores foram adicionados: largura de banda lógica, atraso de cauda P99.9 e amplificação espacial.
- Adicionados detalhes de execução do Rust: varredura, varredura completa, amplificação de gravação, número de chamadas de sistema de gravação e sincronização de dados por lote.
- Marque explicitamente `write-read` como calibre misto e vincule à documentação de desempenho.

Linha de base atual da mesma máquina:

| índice | C++ | Rust | Rust / C++ |
|---|---:|---:|---:|
| Put | 16,866 kv/s | 21,377 kv/s | 1.27x |
| Get | 5,526 QPS | 7,293 QPS | 1.32x |
| Put P99 | 51.2 ms | 35.1 ms | 0.69x |
| Get P99 | 31.2 ms | 10.0 ms | 0.32x |
| Put P99.9 | 86.0 ms | 223.3 ms | 2.60x |
| Get P99.9 | 48.2 ms | 355.5 ms | 7.37x |
| Pico de RSS | 175.6 MiB | 145.5 MiB | 0.83x |
| Uso de armazenamento | 269.0 MiB | 101.4 MiB | 0.38x |

## Indicadores de avaliação

Referindo-me à saída comum de RocksDB, LevelDB, YCSB, WiredTiger e outros benchmarks, adicionei:

- Largura de banda lógica de leitura e gravação MiB/s;
- Atraso de cauda P99.9;
- Amplificação espacial, amplificação de escrita;
- Número de chamadas de sistema de gravação e sincronização de dados em cada lote;
- Taxa de transferência de digitalização/fullscan;
- CPU, falhas de página, alternâncias de contexto e E/S de processo continuam a ser coletadas pelo arquivo secundário de recursos.

Aposentado na mesma época`gxhash_4096B_gib_s`. É um microbenchmark de hash de bloco único de 4 KiB que não está no caminho do mecanismo KV e não possui contrapartida em C++.

## Correção e correções de engenharia

- Corrija o espaço de chave de `--rng cpp` leitura/scan: a detecção de leitura não é mais multiplicada por `key_interval` para evitar que 75% das leituras caiam em chaves não escritas.
- Foram corrigidas novas tentativas de espera causadas pelo erro de julgamento do macOS `openrsync` em manifestos com o mesmo conteúdo.
- Corrigido um problema de hardware em que os discos virtuais QEMU eram mal interpretados como SSDs locais.

## Sites e publicação

- Páginas do GitHub habilitadas: <https://my-god-war.github.io/eloqstore2fkv/>
- O roteamento de site oferece suporte a subcaminhos `/eloqstore2fkv/`.
- O documento é lido de `raw.githubusercontent.com` em vez disso, e o 404 causado pela falta de `web/` no caminho jsDelivr é corrigido.
- A navegação do blog e `bun run tran` a tradução multilíngue estão integradas; este artigo foi traduzido automaticamente da fonte chinesa para outros idiomas.

Os changelogs subsequentes continuarão a documentar alterações de compatibilidade, desempenho e conjunto de ferramentas.