#!/usr/bin/env bash
# Tessera's Rust programs, where the shell starts them from
# ($XDG_DATA_HOME/tessera/bin): tessera-events (native/tessera-events), the
# one process that follows the system for the shell (logind, udev, sound,
# network, badges…), and tessera-ui (native/tessera-ui), the interface in
# Rust. Without them the shell runs its older watchers and draws everything
# in QML.
#
# How they get there is Settings' rust.uiSource, through one entry point
# (tools/native-install.py, which the updater uses too; docs/releases.md):
#   auto        (the default) the signed ready-made build for this exact
#               version, for this processor; built here when there's none
#               (your own edits, no network, a version not built yet)
#   build       built here, for any x86-64
#   buildTuned  built here, for this processor only (-C target-cpu=native)
#
#   tools/build-native.sh [--ui]   as rust.uiSource says (--ui: when built
#                                  here, tessera-ui too, even before Settings
#                                  uses it; it takes several minutes)
#   tools/build-native.sh --build  build here, for any x86-64
#   tools/build-native.sh --tuned  build here, for this processor only
#   tools/build-native.sh --stage DIR [--tuned] [--ui]
#       native-install.py's builder: builds and copies the programs into DIR
#       (installs nothing). Needs rust (cargo); crates come from crates.io, at
#       the versions in Cargo.lock.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${1:-}" == --stage ]]; then
    STAGE="${2:?--stage needs a folder}"
    shift 2
    TUNED=0 UI=0
    for a in "$@"; do
        case "$a" in
            --tuned) TUNED=1 ;;
            --ui) UI=1 ;;
        esac
    done
    command -v cargo >/dev/null || {
        echo "cargo not found. On Arch it comes with the rust package."
        exit 1
    }
    # Tuned builds get their own target folder, so switching back and forth
    # doesn't rebuild everything each time
    sfx=""
    if [[ $TUNED == 1 ]]; then
        export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=native"
        sfx="/tuned"
    fi
    mkdir -p "$STAGE"
    CRATE="$ROOT/native/tessera-events"
    cargo build --release --locked --quiet --manifest-path "$CRATE/Cargo.toml" --target-dir "$CRATE/target$sfx"
    cp "$CRATE/target$sfx/release/tessera-events" "$STAGE/tessera-events"
    [[ $UI == 1 ]] || exit 0
    # tessera-ui: its first build takes several minutes (Slint, link-time
    # optimisation). A failed one still installs tessera-events
    UI_CRATE="$ROOT/native/tessera-ui"
    if cargo build --release --locked --quiet --manifest-path "$UI_CRATE/Cargo.toml" --target-dir "$UI_CRATE/target$sfx" --bin tessera-ui; then
        cp "$UI_CRATE/target$sfx/release/tessera-ui" "$STAGE/tessera-ui"
    else
        echo "tessera-ui didn't build"
    fi
    exit 0
fi

# (iRice's folders, from before it became Tessera, move over first: never both)
python3 -I "$ROOT/shell/assets/scripts/migrate-names.py" || true
args=()
for a in "$@"; do
    case "$a" in
        --build) args+=(--mode build) ;;
        --tuned) args+=(--mode buildTuned) ;;
        --source=auto|--source=build|--source=buildTuned) args+=(--mode "${a#*=}") ;;
        --ui) args+=(--ui) ;;
        *) echo "Unknown option: $a (see the top of $0)"; exit 2 ;;
    esac
done
exec python3 -I "$ROOT/tools/native-install.py" install "${args[@]}"
