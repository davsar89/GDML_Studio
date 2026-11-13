# gdml-cad-worker, against an OCCT built with occt-options.cmake
# (CMAKE_PREFIX_PATH=<OCCT install>). Same project on every platform.
cmake_minimum_required(VERSION 3.16)
project(gdml-cad-worker LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

file(STRINGS occt-source.txt occt_version REGEX "^version=")
string(REPLACE "version=" "" occt_version "${occt_version}")
find_package(OpenCASCADE ${occt_version} EXACT REQUIRED CONFIG)
find_package(Threads REQUIRED)

add_executable(gdml-cad-worker src/main.cpp src/read.cpp src/mesh.cpp src/interfere.cpp src/output.cpp
  src/recognize.cpp src/certify.cpp)
# The certificate and recognition on known solids: ctest.
add_executable(recognize-test test/recognize_test.cpp src/recognize.cpp src/certify.cpp)
enable_testing()
add_test(NAME recognize COMMAND recognize-test)
foreach(target gdml-cad-worker recognize-test)
  target_include_directories(${target} PRIVATE ${OpenCASCADE_INCLUDE_DIR})
  # OCC_CATCH_SIGNALS turns faults into exceptions only with this defined.
  target_compile_definitions(${target} PRIVATE OCC_CONVERT_SIGNALS)
  target_link_libraries(${target} PRIVATE
    TKDESTEP TKXCAF TKLCAF TKCAF TKXSBase TKDE TKMesh TKBO TKPrim TKShHealing TKTopAlgo
    TKGeomAlgo TKGeomBase TKBRep TKG3d TKG2d TKMath TKernel Threads::Threads)
endforeach()
# Installed next to its libraries' directory: cad/gdml-cad-worker, cad/lib/.
set_target_properties(gdml-cad-worker PROPERTIES INSTALL_RPATH "$ORIGIN/lib")
install(TARGETS gdml-cad-worker RUNTIME DESTINATION .)
