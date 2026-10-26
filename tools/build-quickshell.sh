#!/usr/bin/env bash
# Builds iRice its own Quickshell (a release, against your current Qt) into
# ~/.local/share/irice/quickshell, and iRice uses it from then on. For when the
# system's Quickshell is pinned by another setup (end4's dots pin an older
# commit) and is out of step with Qt after an update ("built against Qt X but
# the system has Qt Y ... likely to cause crashes"). Nothing system-wide
# changes; delete that folder to go back. Run again after big Qt updates.
#   tools/build-quickshell.sh [version]     (default v0.3.2)
set -euo pipefail
VER="${1:-v0.3.2}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}/irice"
SRC="$DATA/build/quickshell"
PREFIX="$DATA/quickshell"
for c in git cmake ninja; do command -v "$c" >/dev/null || { echo "Needs $c (sudo pacman -S --needed git cmake ninja)"; exit 1; }; done
mkdir -p "$DATA/build"
# Reuse the download when it's already this version (and its build, so a rerun
# only compiles what changed)
if [[ "$(git -C "$SRC" describe --tags --exact-match 2>/dev/null)" != "$VER" ]]; then
    rm -rf "$SRC"
    git clone --depth 1 --branch "$VER" https://git.outfoxxed.me/quickshell/quickshell.git "$SRC"
fi
cmake -S "$SRC" -B "$SRC/build" -GNinja -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" -DINSTALL_QML_PREFIX=lib/qt6/qml \
    -DDISTRIBUTOR="iRice (local build)" -DDISTRIBUTOR_DEBUGINFO_AVAILABLE=NO
nice -n 10 cmake --build "$SRC/build"
# Install into a staging copy of the real path (Quickshell's install writes
# absolute paths, so --prefix can't redirect it), then swap it in whole: the
# running shell keeps its files until it restarts
STAGE="$DATA/build/stage"
rm -rf "$STAGE"
DESTDIR="$STAGE" cmake --install "$SRC/build"
"$STAGE$PREFIX/bin/quickshell" --version >/dev/null || { echo "The new Quickshell doesn't start; keeping the current one."; exit 1; }
rm -rf "$PREFIX.old"
[[ -d "$PREFIX" ]] && mv "$PREFIX" "$PREFIX.old"
mv "$STAGE$PREFIX" "$PREFIX"
rm -rf "$PREFIX.old" "$STAGE"
[[ -x "$PREFIX/bin/qs" && -d "$PREFIX/lib/qt6/qml/Quickshell" ]] || { echo "Install incomplete in $PREFIX"; exit 1; }
echo "iRice now uses $("$PREFIX/bin/qs" --version | head -1) from $PREFIX (takes effect when the shell restarts: Super+Ctrl+Escape)."
