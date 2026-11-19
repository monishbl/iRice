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
#   build       built here, for this processor's level (x86-64-v3 or generic)
#   buildTuned  built here, for this processor only (-C target-cpu=native)
#
#   tools/build-native.sh [--ui]   as rust.uiSource says (--ui: when built
#                                  here, tessera-ui too, even before Settings
#                                  uses it; it takes several minutes)
#   tools/build-native.sh --build  build here, for this processor's level
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
    # Where cargo works: TESSERA_CARGO_TARGET (native-install.py: the build
    # cache, one folder per variant, outside the checkout, kept between
    # updates), else each crate's own target folder. The profile:
    # TESSERA_CARGO_PROFILE ("local": thin LTO, minutes instead of the
    # release profile's long link) when the crate has it, else release.
    # Tuned builds get their own target folder, so switching back and forth
    # doesn't rebuild everything each time
    sfx=""
    if [[ $TUNED == 1 ]]; then
        export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=native"
        sfx="/tuned"
    fi
    profile_of() { # crate dir -> the profile to build with
        local p="${TESSERA_CARGO_PROFILE:-release}"
        [[ $p == release ]] || grep -q "^\[profile\.$p\]" "$1/Cargo.toml" || p=release
        echo "$p"
    }
    target_of() { # crate dir -> its target folder
        if [[ -n "${TESSERA_CARGO_TARGET:-}" ]]; then echo "$TESSERA_CARGO_TARGET"; else echo "$1/target$sfx"; fi
    }
    mkdir -p "$STAGE"
    # Each build through tools/build-progress.py: how far it is, the crate
    # it's on, the time (and how long is left, after a first build), live;
    # $XDG_STATE_HOME/tessera/build-progress.json and build.log for the
    # installer and Software Update. Plain cargo when python3 isn't there.
    cargo_build() { # label title final cargo-args…
        local label="$1" title="$2" final="$3"
        shift 3
        if command -v python3 >/dev/null && [[ -f "$ROOT/tools/build-progress.py" ]]; then
            python3 -I "$ROOT/tools/build-progress.py" cargo --label "$label" --title "$title" --final "$final" -- cargo build "$@"
        else
            cargo build --quiet "$@"
        fi
    }
    CRATE="$ROOT/native/tessera-events"
    P=$(profile_of "$CRATE") T=$(target_of "$CRATE")
    cargo_build tessera-events tessera-events tessera-events --profile "$P" --locked --manifest-path "$CRATE/Cargo.toml" --target-dir "$T"
    cp "$T/$P/tessera-events" "$STAGE/tessera-events"
    [[ $UI == 1 ]] || exit 0
    # tessera-ui: its first build takes several minutes (Slint, link-time
    # optimisation). A failed one still installs tessera-events
    UI_CRATE="$ROOT/native/tessera-ui"
    P=$(profile_of "$UI_CRATE") T=$(target_of "$UI_CRATE")
    if cargo_build tessera-ui "the shell" tessera-ui --profile "$P" --locked --manifest-path "$UI_CRATE/Cargo.toml" --target-dir "$T" --bin tessera-ui; then
        cp "$T/$P/tessera-ui" "$STAGE/tessera-ui"
    else
        echo "tessera-ui didn't build"
    fi
    exit 0
fi
# (folders under Tessera's old name move over first, never both: migrate-names.py)
python3 -I "$ROOT/shell/assets/scripts/migrate-names.py" --no-checkout || true
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
