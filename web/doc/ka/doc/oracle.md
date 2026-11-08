# C++ oracle

`cpp/oracle` გამოიყენეთ eloqstore-ის რეალური მშენებელი/პარსერი ბაიტის კორპუსის გენერირებისთვის.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ცარიელი / ერთჯერადი / მრავალჯერადი (placeholder) |
| store | ცარიელი / ერთჯერადი / მრავალჯერადი (placeholder) |

## ინტერფეისი

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

გასასვლელი კოდი 0 ითვლება წარმატებულად. `verify` უნდა გადაეცეს ცალსახად `kind` / `case`.

## შექმენით დამოკიდებულებები

Linux კონტეინერებს სჭირდებათ `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`,
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. Oracle გამოიყენებს abseil-ის წყაროს კოდს, რომელიც ინახება eloqstore pin-ში სტატიკური ბიბლიოთეკის ასაშენებლად.