cmake_minimum_required(VERSION 3.5)

# Use 'where clang' to find dir to clang and change CMAKE_C_COMPILER
set(CMAKE_C_COMPILER             "/usr/bin/clang")
set(CMAKE_C_FLAGS                "-Wall -std=c99")
set(CMAKE_C_FLAGS_DEBUG          "-g")
set(CMAKE_C_FLAGS_MINSIZEREL     "-Os -DNDEBUG")
set(CMAKE_C_FLAGS_RELEASE        "-O4 -DNDEBUG")
set(CMAKE_C_FLAGS_RELWITHDEBINFO "-O2 -g")

# Use 'where clang++' to find dir to clang++ and change CMAKE_CXX_COMPILER
set(CMAKE_CXX_COMPILER             "/usr/bin/clang++")
set(CMAKE_CXX_FLAGS                "${CMAKE_CXX_FLAGS} -O3 -std=c++11")
set(CMAKE_CXX_FLAGS_DEBUG          "-g")
set(CMAKE_CXX_FLAGS_MINSIZEREL     "-Os -DNDEBUG")
set(CMAKE_CXX_FLAGS_RELEASE        "-O4 -DNDEBUG")
set(CMAKE_CXX_FLAGS_RELWITHDEBINFO "-O2 -g")
set(CMAKE_CXX_STANDARD 14)

# Like before to modify llvm - bins
set(CMAKE_AR      "/usr/bin/llvm-ar")
set(CMAKE_LINKER  "/usr/bin/llvm-link")
set(CMAKE_NM      "/usr/bin/llvm-nm")
set(CMAKE_OBJDUMP "/usr/bin/llvm-objdump")
set(CMAKE_RANLIB  "/usr/bin/llvm-ranlib")

#your project name
project(rlc_phase_llvm)

find_package(LLVM REQUIRED CONFIG)

message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
message(STATUS "Using LLVMConfig.cmake in: ${LLVM_DIR}")

# Set your project compile flags.
# E.g. if using the C++ header files you will need to enable C++11 support
# for your compiler.
include_directories(${LLVM_INCLUDE_DIRS})
separate_arguments(LLVM_DEFINITIONS_LIST NATIVE_COMMAND ${LLVM_DEFINITIONS})
add_definitions(${LLVM_DEFINITIONS_LIST})

# Now build our tools
# Need to modify the path to main.cpp
add_executable(${PROJECT_NAME} phase_llvm/main.cpp phase_llvm/test.h phase_llvm/test.cpp)

# Find the libraries that correspond to the LLVM components
# that we wish to use
llvm_map_components_to_libnames(llvm_libs support core irreader)

# Link against LLVM libraries
target_link_libraries(${PROJECT_NAME} ${llvm_libs})


# Tries to find an install of the Z3 library and header files
#
# Once done this will define
#  Z3_FOUND - BOOL: System has the Z3 library installed
#  Z3_INCLUDE_DIRS - LIST:The GMP include directories
#  Z3_LIBRARIES - LIST:The libraries needed to use Z3
include(FindPackageHandleStandardArgs)

# Try to find libraries
find_library(Z3_LIBRARIES
        NAMES z3
        DOC "Z3 libraries"
        )
if (Z3_LIBRARIES)
    message(STATUS "Found Z3 libraries: \"${Z3_LIBRARIES}\"")
else()
    message(STATUS "Could not find Z3 libraries")
endif()

# Try to find headers
find_path(Z3_INCLUDE_DIRS
        NAMES z3.h
        # For distributions that keep the header files in a `z3` folder,
        # for example Fedora's `z3-devel` package at `/usr/include/z3/z3.h`
        PATH_SUFFIXES z3
        DOC "Z3 C header"
        )
if (Z3_INCLUDE_DIRS)
    message(STATUS "Found Z3 include path: \"${Z3_INCLUDE_DIRS}\"")
else()
    message(STATUS "Could not find Z3 include path")
endif()

# Handle QUIET and REQUIRED and check the necessary variables were set and if so
# set ``Z3_FOUND``
find_package_handle_standard_args(Z3 DEFAULT_MSG Z3_INCLUDE_DIRS Z3_LIBRARIES)