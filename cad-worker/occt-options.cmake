# The OCCT build for gdml-cad-worker, the one owner of its configuration on every
# platform: `cmake -C occt-options.cmake -S <OCCT source> ...`.
# Shared libraries (shipped unmodified under OCCT's LGPL 2.1 with its exception),
# only the toolkits the worker uses, no third-party products.
set(CMAKE_BUILD_TYPE Release CACHE STRING "")
set(BUILD_LIBRARY_TYPE Shared CACHE STRING "")
set(BUILD_CPP_STANDARD C++17 CACHE STRING "")
foreach(module FoundationClasses ModelingData ModelingAlgorithms Visualization
               ApplicationFramework DataExchange Draw)
  set(BUILD_MODULE_${module} OFF CACHE BOOL "")
endforeach()
# OCCT adds the toolkits these depend on (TKXCAF brings TKV3d and TKService).
set(BUILD_ADDITIONAL_TOOLKITS "TKDESTEP TKXCAF TKMesh TKBO TKBool TKPrim TKShHealing" CACHE STRING "")
foreach(product FREETYPE FREEIMAGE FFMPEG OPENVR RAPIDJSON DRACO TBB EIGEN VTK TK XLIB OPENGL GLES2 D3D)
  set(USE_${product} OFF CACHE BOOL "")
endforeach()
set(BUILD_DOC_Overview OFF CACHE BOOL "")
set(BUILD_DOC_RefMan OFF CACHE BOOL "")
set(BUILD_GTEST OFF CACHE BOOL "")
set(BUILD_RESOURCES OFF CACHE BOOL "")
set(INSTALL_TEST_CASES OFF CACHE BOOL "")
# Keep OCCT's range checks in Release: a malformed file then raises a catchable
# Standard_Failure for that part instead of reading out of bounds.
set(BUILD_RELEASE_DISABLE_EXCEPTIONS OFF CACHE BOOL "")
# Installed libraries find each other in their own directory: a RUNPATH applies only
# to the object that carries it, not to the libraries it loads. Ignored on Windows.
set(CMAKE_INSTALL_RPATH "$ORIGIN" CACHE STRING "")
# No entry in the user's CMake package registry (~/.cmake, or the Windows registry).
set(CMAKE_EXPORT_NO_PACKAGE_REGISTRY ON CACHE BOOL "")
