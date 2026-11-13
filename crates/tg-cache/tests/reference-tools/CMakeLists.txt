cmake_minimum_required(VERSION 3.25)
project(cache_file_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(OpenSSL REQUIRED)
add_executable(native_cache_file main.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_encryption.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_encrypted_file.cpp
 ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp
 ${TELEGRAM_ROOT}/lib_base/base/random.cpp
 ${TELEGRAM_ROOT}/lib_base/base/file_lock_posix.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_cache_file PRIVATE
 ${TELEGRAM_ROOT}/lib_storage ${TELEGRAM_ROOT}/lib_base
 ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_cache_file PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp
 -ffunction-sections -fdata-sections)
target_link_options(native_cache_file PRIVATE -Wl,--gc-sections -Wl,--wrap=RAND_bytes)
target_link_libraries(native_cache_file PRIVATE Qt6::Core OpenSSL::Crypto)
