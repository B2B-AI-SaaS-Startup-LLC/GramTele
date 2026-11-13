cmake_minimum_required(VERSION 3.25)
project(cache_database_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(OpenSSL REQUIRED)
file(GLOB cache_sources ${TELEGRAM_ROOT}/lib_storage/storage/cache/*.cpp)
list(FILTER cache_sources EXCLUDE REGEX "_tests\\.cpp$")
add_executable(native_cache_database main.cpp ${cache_sources}
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_encryption.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_encrypted_file.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_databases.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_clear_legacy.cpp
 ${TELEGRAM_ROOT}/lib_storage/storage/storage_clear_legacy_posix.cpp
 ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp
 ${TELEGRAM_ROOT}/lib_base/base/random.cpp
 ${TELEGRAM_ROOT}/lib_base/base/file_lock_posix.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_base/base/concurrent_timer.cpp
 ${TELEGRAM_ROOT}/lib_crl/crl/common/crl_common_queue.cpp
 ${TELEGRAM_ROOT}/lib_crl/crl/common/crl_common_list.cpp
 ${TELEGRAM_ROOT}/lib_crl/crl/common/crl_common_on_main.cpp
 ${TELEGRAM_ROOT}/lib_crl/crl/crl_time.cpp
 ${TELEGRAM_ROOT}/lib_crl/crl/linux/crl_linux_time.cpp)
target_include_directories(native_cache_database PRIVATE
 ${TELEGRAM_ROOT}/lib_storage ${TELEGRAM_ROOT}/lib_base
 ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl
 ${GSL_INCLUDE} ${RANGE_INCLUDE} ${XXHASH_INCLUDE})
target_compile_definitions(native_cache_database PRIVATE CRL_USE_QT XXH_INLINE_ALL)
target_compile_definitions(native_cache_database PRIVATE DATABASE_FIXTURES="${CMAKE_CURRENT_SOURCE_DIR}/../fixtures/database")
target_compile_options(native_cache_database PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_cache_database PRIVATE -Wl,--gc-sections -Wl,--wrap=RAND_bytes -Wl,--wrap=time)
target_link_libraries(native_cache_database PRIVATE Qt6::Core OpenSSL::Crypto)
