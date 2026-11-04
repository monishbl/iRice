#!/usr/bin/env bash
# Builds Tessera's Rust programs and puts them where the shell starts them
# from: tessera-events (native/tessera-events), the one process that follows
# the system for the shell (logind, udev, sound, network, badges…), and
# tessera-ui (native/tessera-ui), the interface in Rust. Without them the
# shell runs its older watchers and draws everything in QML.
#   tools/build-native.sh [--ui]   needs rust (cargo); crates come from
#                            crates.io, at the versions in Cargo.lock
#                            (--ui: tessera-ui too, even before Settings uses it)
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

# tessera-ui, the interface in Rust (native/tessera-ui): it draws only the
# surfaces Settings lists in rust.surfaces, so it's built when that list has
# any (or with --ui). Its first build takes several minutes (Slint, link-time
# optimisation); a failed build leaves the shell drawing everything in QML.
CFG="${XDG_CONFIG_HOME:-$HOME/.config}/tessera/config.json"
in_use=$(python3 -I -c 'import json,sys
try: print(len(json.load(open(sys.argv[1])).get("rust", {}).get("surfaces", [])))
except Exception: print(0)' "$CFG")
[[ " $* " == *" --ui "* || $in_use -gt 0 ]] || exit 0
UI="$ROOT/native/tessera-ui"
cargo build --release --locked --quiet --manifest-path "$UI/Cargo.toml" --bin tessera-ui || {
    echo "tessera-ui didn't build: the shell draws everything in QML meanwhile"
    exit 0
}
cp "$UI/target/release/tessera-ui" "$DEST/tessera-ui.new"
mv "$DEST/tessera-ui.new" "$DEST/tessera-ui"
echo "Built $DEST/tessera-ui ($("$DEST/tessera-ui" --version))"
