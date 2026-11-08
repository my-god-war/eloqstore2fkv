# C++ oracle

`cpp/oracle` eloqstore의 실제 빌더/파서를 사용하여 바이트 코퍼스를 생성합니다.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | 비어 있음 / 단일 / 다중(자리 표시자) |
| store | 비어 있음 / 단일 / 다중(자리 표시자) |

## 인터페이스

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

종료 코드 0은 성공한 것으로 간주됩니다. `verify`은 명시적으로 `kind` / `case` 전달되어야 합니다.

## 의존성 구축

Linux 컨테이너에는 `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`가 필요합니다.
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle은 eloqstore 핀에 저장된 Abseil 소스 코드를 사용하여 정적 라이브러리를 구축합니다.