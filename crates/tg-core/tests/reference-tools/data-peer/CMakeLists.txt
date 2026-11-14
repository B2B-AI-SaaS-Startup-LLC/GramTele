cmake_minimum_required(VERSION 3.25)
project(data_peer_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR} ${SCHEME_INCLUDE} COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_data_peer main.cpp ${CMAKE_CURRENT_BINARY_DIR}/peer_scheme.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
 ${TELEGRAM_ROOT}/SourceFiles/data/data_user_names.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp
 ${TELEGRAM_ROOT}/lib_base/base/crc32hash.cpp)
target_include_directories(native_data_peer PRIVATE ${CMAKE_CURRENT_SOURCE_DIR} ${CMAKE_CURRENT_BINARY_DIR}
 ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_data_peer PRIVATE -O2 -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_data_peer PRIVATE -Wl,--gc-sections -Wl,--wrap=_ZN5QDate11currentDateEv)
target_link_libraries(native_data_peer PRIVATE Qt6::Core)
