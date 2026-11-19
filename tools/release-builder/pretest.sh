#!/usr/bin/env bash
# The release builder's test before a set is offered (tessera-release runs it
# for each variant it built; docs/releases.md, "Tested before it's offered"):
# the new tessera-ui in a hidden sandbox arena on server1 (site/tools/arena.sh:
# never a real screen, demo data, the sandbox shims keep Wi-Fi and Bluetooth
# untouched), and it must
#   1. start, and answer IPC within 30 s,
#   2. draw: the wallpaper and the bar up ("shell ready"),
#   3. lock and unlock and still answer (the lock is what a restart into an
#      update leans on),
# else the set isn't made ready, and why is the last line.
#
#   tools/release-builder/pretest.sh <tessera-ui binary> <checkout of its commit> [arena number]
#
# Exit 0: passed. Exit 1: failed (the last line says what). Afterwards the
# host's Wi-Fi and Bluetooth power are compared with before (a difference
# fails it too: a test must never touch the real radios).
set -uo pipefail
BIN=$(realpath "$1") SRC=$(realpath "$2") N="${3:-71}"
OUT=$(mktemp -d "${TMPDIR:-/tmp}/tessera-pretest.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
radios() { echo "wifi=$(nmcli -t -f WIFI general 2>/dev/null) bt=$(bluetoothctl show 2>/dev/null | grep -m1 Powered | xargs)"; }
before=$(radios)
cd "$SRC" || { echo "pretest: no checkout at $SRC"; exit 1; }
[[ -x site/tools/arena.sh ]] || { echo "pretest: this version has no site/tools/arena.sh, so it can't be tested here"; exit 1; }
# (each check writes its verdict; the arena's own output goes to the log)
# (q: the tested tessera-ui itself, over its own socket, with the answer.
# Not shell-ipc.sh: when tessera-ui doesn't start the sandbox falls back to
# the QML shell, which answers too, and a broken build passed)
CHECKS='
ok() { echo "PRETEST $1" >>"'"$OUT"'/verdict"; }
q() { [ -S "$XDG_RUNTIME_DIR/tessera/ipc.sock" ] && "$XDG_DATA_HOME/tessera/bin/tessera-ui" ipc "$@" 2>/dev/null; }
up=0
for _ in $(seq 60); do q call shell ready >/dev/null && { up=1; break; }; sleep 0.5; done
[ $up = 1 ] && ok answers || { ok "fail: it did not answer IPC within 30 s"; exit 0; }
drawn=0
for _ in $(seq 40); do [ "$(q call shell ready)" = yes ] && { drawn=1; break; }; sleep 0.5; done
[ $drawn = 1 ] && ok draws || { ok "fail: it answered but never drew (no wallpaper and bar)"; exit 0; }
q call lock lock >/dev/null; sleep 2
q call shell ready >/dev/null || { ok "fail: it stopped answering once locked"; exit 0; }
q call lock unlock >/dev/null; sleep 2
q call shell ready >/dev/null && [ "$(q call shell ready)" = yes ] && ok locks || ok "fail: it did not come back from the lock"
'
WALL=$(ls site/film/tessera/capture/seed/photos 2>/dev/null | grep -m1 -iE '[.](jpe?g|png)$')
TESSERA_ARENA=$N timeout 240 site/tools/arena.sh "$BIN" "$OUT" up glass "$WALL" 1 dark sh "$CHECKS" down \
    >"$OUT/arena.log" 2>&1
after=$(radios)
v=$(cat "$OUT/verdict" 2>/dev/null)
if [[ "$before" != "$after" ]]; then
    echo "pretest: the host's radios changed ($before → $after): the test must never touch them"
    exit 1
fi
if grep -q "^PRETEST fail" <<<"$v" || ! grep -q "^PRETEST locks" <<<"$v"; then
    tail -5 "$OUT/arena.log" | sed 's/^/  | /'
    why=$(grep -m1 "^PRETEST fail" <<<"$v" | sed 's/^PRETEST fail: //')
    echo "pretest: failed: ${why:-the arena did not run the checks (see above)}"
    exit 1
fi
echo "pretest: passed (starts, answers in 30 s, draws, locks and unlocks)"
