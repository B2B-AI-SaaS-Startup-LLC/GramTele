cmake_minimum_required(VERSION 3.25)
project(data_chat_channel_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR} ${SCHEME_INCLUDE} COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_groups main.cpp ${CMAKE_CURRENT_BINARY_DIR}/scheme_rights.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_groups PRIVATE ${CMAKE_CURRENT_SOURCE_DIR} ${CMAKE_CURRENT_BINARY_DIR} ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_groups PRIVATE -O2 -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_groups PRIVATE -Wl,--gc-sections)
target_link_libraries(native_groups PRIVATE Qt6::Core)
