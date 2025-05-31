# c_demo

cmake_minimum_required(VERSION 3.5)

set(MACOS TRUE) # Build for macOS Darwin ARM
set(WINDOWS FALSE) # Build for Windows x64
set(LINUX FALSE) # Build for Linus x64

set(CMAKE_C_COMPILER "gcc")
set(CMAKE_CXX_COMPILER "g++")
set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED True)

project(xcplib VERSION 6.0 LANGUAGES C)

# xcplib
set(xcplib_SOURCES ./src/xcpAppl.c ./src/xcpLite.c ./src/xcpEthServer.c ./src/xcpEthTl.c  ./src/xcpQueue.c ./src/a2l.c ./src/platform.c   )
set_source_files_properties(${xcplib_SOURCES} PROPERTIES LANGUAGE C)
add_library(xcplib ${xcplib_SOURCES})
target_include_directories(xcplib PUBLIC "${PROJECT_SOURCE_DIR}/src" )
target_compile_options(xcplib
    PRIVATE
        -Wall
        -Wextra
        -Wconversion
        -Werror
        -pedantic
        -Wshadow
)

# Example hello_xcp
set(hello_xcp_SOURCES examples/hello_xcp/src/main.c ${xcplib_SOURCES} )
add_executable(hello_xcp ${hello_xcp_SOURCES})
target_include_directories(hello_xcp PUBLIC "${PROJECT_SOURCE_DIR}/src" )
target_link_libraries( hello_xcp PRIVATE xcplib)

# Example c_demo
set(c_demo_SOURCES examples/c_demo/src/main.c ${xcplib_SOURCES} )
add_executable(c_demo ${c_demo_SOURCES})
target_include_directories(c_demo PUBLIC "${PROJECT_SOURCE_DIR}/src" )
target_link_libraries(c_demo PRIVATE xcplib)

if (WINDOWS)

else ()

  #set(THREADS_PREFER_PTHREAD_FLAG ON)
  #find_package(Threads REQUIRED)
  #target_link_libraries(c_demo PRIVATE Threads::Threads)

  #target_link_libraries(c_demo PRIVATE m)

  set_target_properties(c_demo PROPERTIES SUFFIX ".out")
  set_target_properties(hello_xcp PROPERTIES SUFFIX ".out")

endif ()
