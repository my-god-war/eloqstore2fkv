# C++ oracle

`cpp/oracle` ಬೈಟ್ ಕಾರ್ಪಸ್ ಅನ್ನು ಉತ್ಪಾದಿಸಲು eloqstore ನ ನಿಜವಾದ ಬಿಲ್ಡರ್ / ಪಾರ್ಸರ್ ಅನ್ನು ಬಳಸಿ.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | ಖಾಲಿ / ಏಕ / ಬಹು (ಪ್ಲೇಸ್‌ಹೋಲ್ಡರ್) |
| store | ಖಾಲಿ / ಏಕ / ಬಹು (ಪ್ಲೇಸ್‌ಹೋಲ್ಡರ್) |

## ಇಂಟರ್ಫೇಸ್

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 ರ ನಿರ್ಗಮನ ಕೋಡ್ ಯಶಸ್ವಿಯಾಗಿದೆ ಎಂದು ಪರಿಗಣಿಸಲಾಗಿದೆ. `verify` ಅನ್ನು ಸ್ಪಷ್ಟವಾಗಿ `kind` / `case` ರವಾನಿಸಬೇಕು.

## ಅವಲಂಬನೆಗಳನ್ನು ನಿರ್ಮಿಸಿ

Linux ಕಂಟೈನರ್‌ಗಳಿಗೆ `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`, ಅಗತ್ಯವಿದೆ
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. ಒರಾಕಲ್ ಸ್ಥಾಯೀ ಗ್ರಂಥಾಲಯವನ್ನು ನಿರ್ಮಿಸಲು eloqstore ಪಿನ್‌ನಲ್ಲಿ ಸಂಗ್ರಹವಾಗಿರುವ abseil ಮೂಲ ಕೋಡ್ ಅನ್ನು ಬಳಸುತ್ತದೆ.