cmake_minimum_required(VERSION 3.25)
project(native_mtproto_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
set(TELEGRAM_ROOT "" CACHE PATH "Read-only original Telegram")
find_package(Qt6 REQUIRED COMPONENTS Core Gui Network)
find_package(OpenSSL REQUIRED)
add_executable(native_crypto crypto.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_auth_key.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/details/mtproto_rsa_public_key.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_crypto PRIVATE ${TELEGRAM_ROOT}/SourceFiles
 ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_crypto PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_crypto PRIVATE -Wl,--gc-sections)
target_link_libraries(native_crypto PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
set(SCHEME_INCLUDE "" CACHE PATH "External generated native TL scheme include path")
target_include_directories(native_crypto PRIVATE ${SCHEME_INCLUDE})
target_sources(native_crypto PRIVATE
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_dc_options.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_config.cpp)
set(REFERENCE_DIR "" CACHE PATH "External generated capture source directory")
add_executable(native_tcp ${REFERENCE_DIR}/tcp.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_auth_key.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_tcp PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_tcp PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_tcp PRIVATE -Wl,--gc-sections)
target_link_libraries(native_tcp PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
target_sources(native_tcp PRIVATE ${TELEGRAM_ROOT}/lib_base/base/random.cpp ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp)
target_link_options(native_tcp PRIVATE -Wl,--wrap=RAND_bytes)
set(SCHEME_OBJECT "" CACHE FILEPATH "Original generated scheme.cpp object from the native TL build")
add_executable(native_tls ${REFERENCE_DIR}/tls.cpp ${SCHEME_OBJECT}
 ${TELEGRAM_ROOT}/lib_base/base/random.cpp
 ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_tls PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_tls PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_tls PRIVATE -Wl,--gc-sections -Wl,--wrap=RAND_bytes)
target_link_libraries(native_tls PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
add_executable(native_socks socks.cpp)
target_link_libraries(native_socks PRIVATE Qt6::Core Qt6::Network)
add_executable(native_connect connect.cpp)
target_link_libraries(native_connect PRIVATE Qt6::Core Qt6::Network)
add_executable(native_proxy proxy.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_proxy PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_proxy PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_proxy PRIVATE -Wl,--gc-sections)
target_link_libraries(native_proxy PRIVATE Qt6::Core Qt6::Gui Qt6::Network)
add_executable(native_http http.cpp)
target_link_libraries(native_http PRIVATE Qt6::Core Qt6::Network)
add_executable(native_session ${REFERENCE_DIR}/session.cpp ${SCHEME_OBJECT}
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_auth_key.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/details/mtproto_serialized_request.cpp
 ${TELEGRAM_ROOT}/lib_base/base/random.cpp
 ${TELEGRAM_ROOT}/lib_base/base/bytes.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_session PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_session PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_session PRIVATE -Wl,--gc-sections -Wl,--wrap=RAND_bytes)
target_link_libraries(native_session PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
add_executable(native_session_receive ${REFERENCE_DIR}/session_receive.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/mtproto_auth_key.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_session_receive PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_session_receive PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_session_receive PRIVATE -Wl,--gc-sections)
target_link_libraries(native_session_receive PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
add_executable(native_simple_config ${REFERENCE_DIR}/simple_config.cpp
 ${TELEGRAM_ROOT}/SourceFiles/mtproto/details/mtproto_rsa_public_key.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp)
target_include_directories(native_simple_config PRIVATE ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl)
target_compile_options(native_simple_config PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_simple_config PRIVATE -Wl,--gc-sections)
target_link_libraries(native_simple_config PRIVATE Qt6::Core Qt6::Gui Qt6::Network OpenSSL::Crypto)
target_sources(native_simple_config PRIVATE ${SCHEME_OBJECT})
