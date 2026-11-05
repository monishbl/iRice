#!/usr/bin/env bash
# Screenshots of the login screen, QML and Rust driven the same way, in the
# hidden sandbox arena 9 (never the real screen, never the real greetd):
#   tools/greeter-shots.sh <glass|material> <qml|rust> [look|text|plain]
#   tools/greeter-shots.sh demo <glass|material> [look|text|plain]   (Rust's --demo states)
# Both greeters get the same made-up people (a passwd file of their own: the
# QML one through a copy of GreeterAuth.qml pointed at it), sessions, host
# name and picture, and the same config.json (as greeter-start.sh writes
# one). A preview first (the password "tessera"), then against
# tools/fake-greetd.py for the fingerprint reader and a real login
# conversation. Power is never confirmed, and `systemctl` is a stub on PATH
# besides (the Rust one also runs --dry). Shots: review/greeter/.
# Every greeter here runs inside the arena (`dev run`), never with this
# shell's WAYLAND_DISPLAY.
#
# The full pass, both looks (12 drive runs, 3 demo runs):
#   for look in glass material; do for s in look plain text; do
#     for w in qml rust; do tools/greeter-shots.sh $look $w $s; done; done; done
#   for look in glass material; do tools/greeter-shots.sh demo $look look; done
#   tools/greeter-shots.sh demo glass text
# Then compare qml-*/rust-* pairs state by state: users, typing, busy, wrong
# (the shake has settled), session, power (Restart asked), next-user,
# finger, finger-miss, greetd-wrong. Each "started:" line must name
# tessera-session for user grace (the login went through the fake greetd).
# Each "power stub:" line must say "never called". Then IRICE_ARENA=9 ./dev.sh down.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
export IRICE_ARENA=9 TESSERA_ARENA=9
ROOT="$REPO"
. "$REPO/sandbox/lib.sh"
dev() { "$REPO/dev.sh" "$@"; }
P() { dev pointer -o IRICE-TEST "$@" >/dev/null 2>&1; }
key() { dev run wtype "$@" 2>/dev/null; }

#   FACE=analog tools/greeter-shots.sh still <glass|material> <qml|rust>   (one preview shot: the clock faces)
case $1 in
    demo) mode=demo look=$2 which=rust style=${3:-look} ;;
    still) mode=still look=$2 which=$3 style=${4:-look} ;;
    *) mode=drive look=$1 which=$2 style=${3:-look} ;;
esac
OUT="$HERE/review/greeter"
G="$TESSERA_HOME/greeter-test"
# (the socket in the arena's runtime folder: a path under its home is longer
# than the 108 bytes a socket's path may have)
SOCK="$TESSERA_RT/fake-greetd.sock"
BIN="$HERE/target/${PROFILE:-release}/tessera-ui"
mkdir -p "$OUT" "$G"
tag="$which-$look-$style${FACE:+-$FACE}"
shot() { sleep "${2:-1}"; dev shot "$OUT/$tag-$1.png" >/dev/null && echo "shot $tag-$1"; }

# ── The made-up world ───────────────────────────────────────────────────────
mkdir -p "$G/sessions" "$G/icons" "$G/bin" "$G/config/tessera"
printf '%s\n' 'root:x:0:0::/root:/bin/bash' 'ada:x:1000:1000:Ada Lovelace:/home/ada:/bin/bash' \
    'grace:x:1001:1001:Grace Hopper,,,:/home/grace:/bin/zsh' 'katherine:x:1002:1002:Katherine Johnson:/home/k:/bin/bash' \
    'daemon:x:1003:1003::/:/usr/bin/nologin' >"$G/passwd"
echo tessera >"$G/hostname"
printf '[Desktop Entry]\nName=Tessera\nExec=tessera-session\n' >"$G/sessions/tessera.desktop"
printf '[Desktop Entry]\nName=Hyprland\nExec=Hyprland\n' >"$G/sessions/hyprland.desktop"
printf '[Desktop Entry]\nName=Sway\nExec=sway\n' >"$G/sessions/sway.desktop"
cp "$REPO/shell/assets/wallpapers/tahoe-dawn.jpg" "$G/icons/ada"
printf '#!/bin/sh\necho "systemctl stub: $*" >>"%s/power.log"\n' "$G" >"$G/bin/systemctl"
chmod +x "$G/bin/systemctl"
python3 -I - "$G/config/tessera/config.json" "$look" "${FACE:-dynamic}" <<'EOF'
import json, sys
json.dump({"appearance": {"look": sys.argv[2], "mode": "dark", "materialSeed": "#6750A4", "materialStyle": "tonalSpot", "clock24": False},
           "lock": {"clockFace": sys.argv[3], "clockWeight": 640, "clockWidth": 112, "clockRound": True},
           "wallpaper": {"path": ""}}, open(sys.argv[1], "w"))
EOF
# The QML greeter's root (the Material one assembled as greeter-start.sh
# does), its GreeterAuth.qml reading the made-up files
QROOT="$G/root-$look"
rm -rf "$QROOT"
python3 -I "$REPO/shell/assets/scripts/material-root.py" "$REPO/shell" "$REPO/$([[ $look == material ]] && echo material || echo shell)" "$QROOT" >/dev/null
auth="$QROOT/modules/greeter/GreeterAuth.qml"
src=$(readlink -f "$auth")
rm -f "$auth"
sed -e "s#\"/etc/passwd\"#\"$G/passwd\"#" -e "s#\"/etc/hostname\"#\"$G/hostname\"#" \
    -e "s#/usr/share/wayland-sessions/#$G/sessions/#" -e "s#file:///var/lib/AccountsService/icons/#file://$G/icons/#" "$src" >"$auth"

# ── The arena: up, one 1920×1080 screen, no shell ───────────────────────────
dev status | grep -q running || { dev up >/dev/null 2>&1; sleep 12; }
dev hyprctl eval 'hl.monitor({ output = "IRICE-TEST", mode = "1920x1080@60", position = "20000x20000", scale = 1 })' >/dev/null
dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
dev hyprctl dispatch 'hl.dsp.focus({ monitor = "IRICE-TEST" })' >/dev/null
stop_sandbox_shells

stop_greeter() {
    local p
    for p in $(sandbox_pids qs; sandbox_pids quickshell; sandbox_pids tessera-ui); do kill "$p" 2>/dev/null; done
    sleep 0.5
}
# start <mock|greetd> <first user> [demo state]
start() {
    stop_greeter
    rm -rf "$G/cache-$which"
    mkdir -p "$G/cache-$which"
    printf '{"user":"%s","session":"Tessera"}' "$2" >"$G/cache-$which/tessera-greeter.json"
    local env=(XDG_CONFIG_HOME="$G/config" XDG_CACHE_HOME="$G/cache-$which" PATH="$G/bin:/usr/bin:/bin"
        TESSERA_GREETER_STYLE="$style" TESSERA_GREETER_CLOCK24=0)
    [[ $1 == mock ]] && env+=(TESSERA_GREETER_MOCK=1) || env+=(GREETD_SOCK="$SOCK")
    local cmd
    if [[ $which == qml ]]; then
        cmd="qs -p '$QROOT/greeter.qml'"
    else
        env+=(TESSERA_GREETER_PASSWD="$G/passwd" TESSERA_GREETER_SESSIONS="$G/sessions" TESSERA_GREETER_HOSTNAME="$G/hostname" TESSERA_GREETER_ICONS="$G/icons")
        cmd="'$BIN' --greeter --dry${3:+ --demo $3}"
    fi
    dev run bash -c "setsid -f env ${env[*]} $cmd >'$G/$which.log' 2>&1 </dev/null"
    sleep 3
}
# Restart, as each style offers it
restart_click() {
    case "$style" in
        text) key -k F2 ;;
        plain) P move 1822 1034 click ;;
        look) if [[ $look == material ]]; then P move 1822 1034 click; else P move 960 1005 click; fi ;;
    esac
}
next_session() {
    if [[ $style == text ]]; then key -k Up -k Up -k Right; else key -k Tab; fi
}

if [[ $mode == still ]]; then
    start mock ada
    shot still 2
    stop_greeter
    exit 0
fi
if [[ $mode == demo ]]; then
    for s in ${DEMO_STATES:-users typing wrong busy finger finger-miss session power other none single}; do
        start mock ada "$s"
        shot "demo-$s" 1
    done
    stop_greeter
    exit 0
fi

# ── A preview (the mock): typing, a wrong password, the session, power ────
start mock ada
shot users 1
key "secret12"
shot typing 1
key -k Return
shot busy 0.2
shot wrong 1.2
next_session
shot session 1
restart_click
shot power 1
key -k Escape
key -k Down
shot next-user 1

# ── Against the fake greetd: the reader, then a real conversation ─────────
python3 -I "$HERE/tools/fake-greetd.py" "$SOCK" --finger 7 --log "$G/started-$which.jsonl" 2>"$G/fake-$which.log" &
FAKE=$!
trap 'kill $FAKE 2>/dev/null; stop_greeter' EXIT
rm -f "$G/started-$which.jsonl"
sleep 1
start greetd grace
shot finger 2
shot finger-miss 7
key "nottheone"
key -k Return
shot greetd-wrong 2
key "tessera"
key -k Return
# (the reader listens again first: the password waits for PAM to ask)
sleep 10
echo "started: $(cat "$G/started-$which.jsonl" 2>/dev/null || echo nothing)"
echo "power stub: $(cat "$G/power.log" 2>/dev/null || echo never called)"
