# C++ oracle

`cpp/oracle` Use o construtor/analisador real do eloqstore para gerar corpus de bytes.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | vazio / único / múltiplo (espaço reservado) |
| store | vazio / único / múltiplo (espaço reservado) |

## interface

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

Um código de saída 0 é considerado bem-sucedido. `verify` deve ser passado explicitamente `kind` / `case`.

## Construir dependências

Os contêineres Linux requerem `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
<bid="1">. A Oracle usará o código-fonte do rapel armazenado no pin eloqstore para construir uma biblioteca estática.