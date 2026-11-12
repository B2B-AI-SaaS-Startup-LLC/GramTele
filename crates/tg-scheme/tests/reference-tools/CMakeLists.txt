cmake_minimum_required(VERSION 3.25)
project(native_tl_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
set(TELEGRAM_ROOT "" CACHE PATH "Read-only original Telegram directory")
set(REFERENCE_DIR "" CACHE PATH "External directory containing generated scheme and builders")
find_package(Qt6 REQUIRED COMPONENTS Core Gui)
add_executable(native_tl_reference
    ${REFERENCE_DIR}/builders.cpp
    ${REFERENCE_DIR}/gen/scheme.cpp
    ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
    ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
    ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_tl_reference PRIVATE
    ${REFERENCE_DIR}/gen ${TELEGRAM_ROOT}/SourceFiles
    ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl)
target_link_libraries(native_tl_reference PRIVATE Qt6::Core Qt6::Gui)
# Remove unused integration functions whose OS dependencies are outside this target.
set_source_files_properties(${TELEGRAM_ROOT}/lib_base/base/integration.cpp
    PROPERTIES COMPILE_OPTIONS "-ffunction-sections;-fdata-sections")
target_link_options(native_tl_reference PRIVATE -Wl,--gc-sections)
add_executable(native_tl_vectors vectors.cpp
    ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
    ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
    ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_tl_vectors PRIVATE
    ${REFERENCE_DIR}/gen ${TELEGRAM_ROOT}/SourceFiles
    ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_rpl)
target_link_libraries(native_tl_vectors PRIVATE Qt6::Core Qt6::Gui)
target_link_options(native_tl_vectors PRIVATE -Wl,--gc-sections)
