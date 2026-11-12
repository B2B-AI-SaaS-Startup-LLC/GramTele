cmake_minimum_required(VERSION 3.25)
project(tdata_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core Gui Network)
find_package(OpenSSL REQUIRED)
add_executable(native_tdata main.cpp ${CAPTURE}/utilities.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_auth_key.cpp
 ${TELEGRAM_ROOT}/lib_base/base/random.cpp
 ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
 ${TELEGRAM_ROOT}/SourceFiles/storage/serialize_common.cpp)
target_include_directories(native_tdata PRIVATE ${CAPTURE} ${SCHEME_INCLUDE}
 ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl
 ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_tdata PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_tdata PRIVATE -Wl,--gc-sections -Wl,--wrap=RAND_bytes)
target_link_libraries(native_tdata PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
