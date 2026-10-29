# iRice's native helpers (built by build.sh into build/):
#   IRice.Native  QML module: GlassSync, the compositor glass shapes kept in
#                 step from C++ (see glasssync.h)
#   glass-ipc     the request helper (C version of assets/scripts/glass-ipc.py)
#   goa-signin    adds a Google account to GNOME Online Accounts from iRice's
#                 Settings (see goa-signin.c); built when GOA's backend
#                 library is installed
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
    SOURCES glasssync.h glasssync.cpp glasssender.hpp
)
target_link_libraries(irice_native PRIVATE Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick)

add_executable(glass-ipc glass-ipc.c)
set_target_properties(glass-ipc PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})

# Optional: without GNOME Online Accounts' backend, Settings falls back to
# GNOME's own Online Accounts window
find_package(PkgConfig)
if(PkgConfig_FOUND)
    pkg_check_modules(GOA_BACKEND IMPORTED_TARGET goa-backend-1.0 json-glib-1.0)
    if(GOA_BACKEND_FOUND)
        add_executable(goa-signin goa-signin.c)
        target_link_libraries(goa-signin PRIVATE PkgConfig::GOA_BACKEND)
        set_target_properties(goa-signin PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
    endif()
endif()
