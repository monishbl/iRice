#!/usr/bin/env bash
# Builds tessera-events (native/tessera-events, Rust), the one process that
# follows the system for the shell (logind, udev, sound, network, badges…),
# and puts it where the shell starts it from. Without it the shell runs its
# older watchers instead (a monitor process and a wrapper for each).
#   tools/build-native.sh    needs rust (cargo); crates come from crates.io,
#                            at the versions in Cargo.lock
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# (iRice's folders, from before it became Tessera, move over first: never both)
python3 -I "$ROOT/shell/assets/scripts/migrate-names.py" || true
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/tessera/bin"
CRATE="$ROOT/native/tessera-events"
command -v cargo >/dev/null || {
    echo "cargo not found. On Arch it comes with the rust package."
    exit 1
}
cargo build --release --locked --quiet --manifest-path "$CRATE/Cargo.toml"
mkdir -p "$DEST"
# (a new file moved over the old: the running one keeps its copy)
cp "$CRATE/target/release/tessera-events" "$DEST/tessera-events.new"
mv "$DEST/tessera-events.new" "$DEST/tessera-events"
rm -f "$DEST/irice-events" # (built before iRice became Tessera: this one replaces it)
echo "Built $DEST/tessera-events ($("$DEST/tessera-events" --version))"
