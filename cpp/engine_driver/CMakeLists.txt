cmake_minimum_required(VERSION 3.20)
project(fkv_engine_driver CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if(NOT ELOQSTORE_SRC)
  message(FATAL_ERROR "ELOQSTORE_SRC 未设置；先跑 scripts/fetch_eloqstore.sh")
endif()

# 只构建引擎库，不构建 benchmark / tests / 示例。
set(SKIP_CREATE_BUCKET ON CACHE BOOL "" FORCE)
set(WITH_BENCHMARK OFF CACHE BOOL "" FORCE)
set(WITH_UNIT_TESTS OFF CACHE BOOL "" FORCE)
set(WITH_EXAMPLE OFF CACHE BOOL "" FORCE)
set(WITH_DB_STRESS OFF CACHE BOOL "" FORCE)
set(CMAKE_POLICY_VERSION_MINIMUM 3.5 CACHE STRING "" FORCE)

add_subdirectory("${ELOQSTORE_SRC}" eloqstore_build)

# Ubuntu 26.04 的 libcurl.so 不再把 zlib 作为可传递依赖带入链接行，
# eloqstore 的 target_link_libraries 只列了 CURL_LIBRARIES；
# 显式补 ZLIB，避免 `undefined reference to inflateEnd`。
find_package(ZLIB REQUIRED)

# Ubuntu 26.04 的静态 libcrypto.a 引用 jitterentropy 符号（OpenSSL 3.5+）。
# 放到 CMAKE_CXX_STANDARD_LIBRARIES（链接行末尾），保证在 libcrypto.a 之后解析。
find_library(JITTERENTROPY_LIB NAMES jitterentropy)
if(JITTERENTROPY_LIB)
  set(CMAKE_CXX_STANDARD_LIBRARIES
      "${CMAKE_CXX_STANDARD_LIBRARIES} ${JITTERENTROPY_LIB}")
endif()

add_executable(engine_driver main.cpp)
target_link_libraries(engine_driver PRIVATE eloqstore ZLIB::ZLIB)
# Ubuntu 26.04 的 libcurl.so 只在 DT_NEEDED 里带 libz，而默认链接器
# （--no-copy-dt-needed-entries）不会递归解析；显式允许，避免
# `undefined reference to inflateEnd`。
target_link_options(engine_driver PRIVATE "-Wl,--copy-dt-needed-entries")
