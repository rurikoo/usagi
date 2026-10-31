cmake_minimum_required(VERSION 3.16)

project(usagi LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)

set(JNIVM_ENABLE_TESTS OFF CACHE BOOL "" FORCE)

add_subdirectory(third_party/libjnivm)

add_library(usagi_bridge STATIC
    cpp/bridge.cpp
)

set_target_properties(usagi_bridge PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
)

target_include_directories(usagi_bridge
    PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include"
)

target_link_libraries(usagi_bridge
    PUBLIC jnivm
)

install(
    TARGETS usagi_bridge jnivm
    ARCHIVE DESTINATION lib
)

install(
    DIRECTORY include/
    DESTINATION include
)

install(
    DIRECTORY third_party/libjnivm/include/
    DESTINATION include
)
