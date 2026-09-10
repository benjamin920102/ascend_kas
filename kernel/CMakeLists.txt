cmake_minimum_required(VERSION 3.16)
set(CMAKE_ASC_ARCHITECTURES "dav-2201" CACHE STRING "NPU architecture")
find_package(ASC REQUIRED)
project(ascend_kas_kernel LANGUAGES ASC CXX)

set(ASCEND_KAS_BATCH "4096" CACHE STRING "nonces per scan batch")
set(ASCEND_KAS_VEC_GRID "40" CACHE STRING "vector blocks")
set(ASCEND_KAS_CUBE_GRID "20" CACHE STRING "cube blocks")
set(ASCEND_KAS_CUBE_SINGLE_N "224" CACHE STRING "N elements handled by one cube core; use a multiple of 32 for INT8")

add_library(ascend_kas SHARED
    ascend_kas_runtime.asc
    ascend_kas_vec.asc)

set_target_properties(ascend_kas PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_link_libraries(ascend_kas PRIVATE tiling_api register platform m dl pthread)
target_compile_options(ascend_kas PRIVATE
    $<$<COMPILE_LANGUAGE:ASC>:-O3>
    $<$<COMPILE_LANGUAGE:ASC>:--npu-arch=${CMAKE_ASC_ARCHITECTURES}>
    $<$<COMPILE_LANGUAGE:ASC>:-DASCEND_KAS_BATCH=${ASCEND_KAS_BATCH}>
    $<$<COMPILE_LANGUAGE:ASC>:-DASCEND_KAS_VEC_GRID=${ASCEND_KAS_VEC_GRID}>
    $<$<COMPILE_LANGUAGE:ASC>:-DASCEND_KAS_CUBE_GRID=${ASCEND_KAS_CUBE_GRID}>
    $<$<COMPILE_LANGUAGE:ASC>:-DASCEND_KAS_CUBE_SINGLE_N=${ASCEND_KAS_CUBE_SINGLE_N}>
    $<$<COMPILE_LANGUAGE:ASC>:-DASCENDC_DUMP=0>
)

