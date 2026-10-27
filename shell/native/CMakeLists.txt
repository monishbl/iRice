# iRice's native helpers (built by build.sh into build/):
#   IRice.Native  QML module: GlassSync, the compositor glass shapes kept in
#                 step from C++ (see glasssync.h)
#   glass-ipc     the request helper (C version of assets/scripts/glass-ipc.py)
# The shell works without them (QML and Python fall-backs); launch-shell.sh
# puts build/qml on the import path when it's there.
cmake_minimum_required(VERSION 3.21)
project(irice-native LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Gui Qml Quick)
qt_standard_project_setup(REQUIRES 6.5)

qt_add_qml_module(irice_native
    URI IRice.Native
    VERSION 1.0
    PLUGIN_TARGET irice_native
    OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/qml/IRice/Native
    SOURCES glasssync.h glasssync.cpp
)
target_link_libraries(irice_native PRIVATE Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick)

add_executable(glass-ipc glass-ipc.c)
set_target_properties(glass-ipc PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
