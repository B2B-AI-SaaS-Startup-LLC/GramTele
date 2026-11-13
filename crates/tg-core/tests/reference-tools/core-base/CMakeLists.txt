cmake_minimum_required(VERSION 3.25)
project(native_core_base LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
add_executable(native_lists lists.cpp ${TELEGRAM_ROOT}/SourceFiles/storage/storage_sparse_ids_list.cpp ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp)
target_compile_options(native_lists PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_lists PRIVATE -Wl,--gc-sections)
target_include_directories(native_lists PRIVATE ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_link_libraries(native_lists PRIVATE Qt6::Core)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract_plurals.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR}/plurals.cpp COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_plurals ${CMAKE_CURRENT_BINARY_DIR}/plurals.cpp)
target_link_libraries(native_plurals PRIVATE Qt6::Core)
target_sources(native_lists PRIVATE ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_sources(native_lists PRIVATE ${TELEGRAM_ROOT}/SourceFiles/storage/storage_user_photos.cpp)
add_executable(native_storage_trace trace.cpp
 ${TELEGRAM_ROOT}/SourceFiles/storage/storage_sparse_ids_list.cpp
 ${TELEGRAM_ROOT}/SourceFiles/storage/storage_shared_media.cpp
 ${TELEGRAM_ROOT}/SourceFiles/storage/storage_user_photos.cpp
 ${TELEGRAM_ROOT}/SourceFiles/storage/storage_facade.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_compile_options(native_storage_trace PRIVATE -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_storage_trace PRIVATE -Wl,--gc-sections)
target_include_directories(native_storage_trace PRIVATE
 ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base
 ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_link_libraries(native_storage_trace PRIVATE Qt6::Core)
