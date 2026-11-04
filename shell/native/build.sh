#!/usr/bin/env bash
# Builds Tessera's native helpers into build/ (see CMakeLists.txt). Needs cmake,
# a C/C++ compiler and Qt 6 development files (qt6-declarative). Run again
# after a Qt update; the shell falls back to QML and Python without them.
set -euo pipefail
cd "$(dirname "$0")"
gen=(); command -v ninja >/dev/null && gen=(-G Ninja)
cmake -S . -B build "${gen[@]}" -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build build --parallel
# (the module's name while Tessera was called iRice: replaced by this build)
rm -rf build/qml/IRice
# Remember which Qt it was built for (launch-shell.sh skips a stale build)
{ /usr/lib/qt6/bin/qtpaths --qt-version || qmake6 -query QT_VERSION; } 2>/dev/null >build/built-for-qt || true
echo "Built: $(pwd)/build/qml (Tessera.Native) and $(pwd)/build/glass-ipc"
