#!/usr/bin/env bash
# Builds irice-events (native/irice-events, Rust), the one process that
# follows the system for the shell (logind, udev, sound, network, badges…),
# and puts it where the shell starts it from. Without it the shell runs its
# older watchers instead (a monitor process and a wrapper for each).
#   tools/build-native.sh    needs rust (cargo); crates come from crates.io,
#                            at the versions in Cargo.lock
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/irice/bin"
CRATE="$ROOT/native/irice-events"
command -v cargo >/dev/null || {
    echo "cargo not found. On Arch it comes with the rust package."
    exit 1
}
cargo build --release --locked --quiet --manifest-path "$CRATE/Cargo.toml"
mkdir -p "$DEST"
# (a new file moved over the old: the running one keeps its copy)
cp "$CRATE/target/release/irice-events" "$DEST/irice-events.new"
mv "$DEST/irice-events.new" "$DEST/irice-events"
echo "Built $DEST/irice-events ($("$DEST/irice-events" --version))"
