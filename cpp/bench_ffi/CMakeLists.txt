cmake_minimum_required(VERSION 3.20)
project(fkv_bench_ffi CXX)

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

add_library(fkv_bench_ffi SHARED bench_ffi.cpp bench_ffi.h)
target_include_directories(fkv_bench_ffi PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}"
  "${ELOQSTORE_SRC}/include"
)
target_link_libraries(fkv_bench_ffi PRIVATE eloqstore ZLIB::ZLIB)

# 只导出 fkv_bench_* 的 C ABI（header 里标了 default visibility）。
set_target_properties(fkv_bench_ffi PROPERTIES
  CXX_VISIBILITY_PRESET hidden
  VISIBILITY_INLINES_HIDDEN YES
)

# Ubuntu 26.04 的 libcurl.so 只在 DT_NEEDED 里带 libz，而默认链接器
# （--no-copy-dt-needed-entries）不会递归解析；显式允许，避免
# `undefined reference to inflateEnd`。macOS 的 ld 不认识这个选项。
if(UNIX AND NOT APPLE)
  target_link_options(fkv_bench_ffi PRIVATE "-Wl,--copy-dt-needed-entries")
endif()

# ---------------------------------------------------------------------------
# RocksDB 参考引擎：同一个 C ABI（bench_ffi.h），矩阵用 `--rocksdb-lib` 加载。
#
# 依赖系统 RocksDB（Ubuntu 的 `librocksdb-dev`）；找不到就跳过，矩阵会退化成
# 两列（fkv / EloqStore），不会让整个 perf 流程失败。
# ---------------------------------------------------------------------------
find_path(ROCKSDB_INCLUDE_DIR rocksdb/db.h)
find_library(ROCKSDB_LIBRARY NAMES rocksdb)
if(ROCKSDB_INCLUDE_DIR AND ROCKSDB_LIBRARY)
  add_library(rocksdb_bench_ffi SHARED rocksdb_ffi.cpp bench_ffi.h)
  target_include_directories(rocksdb_bench_ffi PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${ROCKSDB_INCLUDE_DIR}"
  )
  target_link_libraries(rocksdb_bench_ffi PRIVATE "${ROCKSDB_LIBRARY}")
  set_target_properties(rocksdb_bench_ffi PROPERTIES
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN YES
  )
  message(STATUS "rocksdb_bench_ffi: ${ROCKSDB_LIBRARY}")
else()
  message(STATUS "rocksdb_bench_ffi: 跳过（未找到 rocksdb/db.h 或 librocksdb）")
endif()
