cmake_minimum_required(VERSION 3.25)
project(data_changes_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
foreach(mode IN ITEMS O0 O2)
 add_executable(native_data_changes_${mode} main.cpp
  ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
  ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
 target_include_directories(native_data_changes_${mode} PRIVATE ${SCHEME_INCLUDE}
  ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl
  ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${TELEGRAM_ROOT}/lib_ui
  ${GSL_INCLUDE} ${RANGE_INCLUDE})
 target_compile_definitions(native_data_changes_${mode} PRIVATE CRL_USE_QT)
 target_compile_options(native_data_changes_${mode} PRIVATE -${mode}
  -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
 target_link_options(native_data_changes_${mode} PRIVATE -Wl,--gc-sections)
 target_link_libraries(native_data_changes_${mode} PRIVATE Qt6::Core)
endforeach()
