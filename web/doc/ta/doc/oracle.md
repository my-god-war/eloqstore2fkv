# C++ oracle

`cpp/oracle` பைட் கார்பஸை உருவாக்க eloqstore இன் உண்மையான பில்டர் / பாகுபடுத்தியைப் பயன்படுத்தவும்.

## kind / case

| kind | case |
|---|---|
| page | empty / single / multi / restart / ttl / large / overflow |
| index | leaf / branch |
| overflow | chain |
| manifest | empty / branch / checksum / archive |
| compress | raw / dict |
| mapping | வெற்று / ஒற்றை / பல ( ஒதுக்கிட) |
| store | வெற்று / ஒற்றை / பல ( ஒதுக்கிட) |

## இடைமுகம்

```text
<oracle> fixture --kind K --case C --out DIR
<oracle> verify  --case-dir DIR --kind K --case C
```

0 இன் வெளியேறும் குறியீடு வெற்றிகரமாக கருதப்படுகிறது. `verify` வெளிப்படையாக `kind` / `case` அனுப்பப்பட வேண்டும்.

## சார்புகளை உருவாக்குங்கள்

Linux கொள்கலன்களுக்கு `cmake`,`g++ >= 13`,`libzstd-dev`,`libgoogle-glog-dev`, தேவை
`libgflags-dev`,`libjsoncpp-dev`,`liburing-dev`,`libcurl4-openssl-dev`,
`libboost-all-dev`. நிலையான நூலகத்தை உருவாக்க, eloqstore பின்னில் சேமிக்கப்பட்டுள்ள abseil மூலக் குறியீட்டை Oracle பயன்படுத்தும்.