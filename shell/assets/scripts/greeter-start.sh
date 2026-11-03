#!/usr/bin/env bash
# Starts the iRice login screen in the greeter's compositor (hypr/greeter.lua
# runs it as greetd's "greeter" user, from ./install.sh --greeter's copy in
# /usr/share/irice). Its look comes from greeter.json there (Settings → Lock
# Screen → Login screen writes it through irice-root login-look): the style,
#   look   the look you last used: Glass (shell/) or Material (its root,
#          assembled here from shell/ and material/)
#   text   ly's: a box of fields in monospace, keyboard first
#   plain  black, your picture, the field and the fingerprint
# and the colours and clock that look needs, as a config.json of its own
# (the greeter can't read your home). A login screen that doesn't start
# (quits with an error within 10 s) gives way to the text one, so there's
# always a way in. Returns when the greeter quits (you logged in).
#   greeter-start.sh            (IRICE_SHARE: another copy, for tests)
set -u
SHARE="${IRICE_SHARE:-/usr/share/irice}"
QS="${IRICE_QS:-qs}"
T="$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/irice-greeter.XXXXXX")" || T=""
[[ -n $T ]] && trap 'rm -rf "$T"' EXIT

# The look, and a config.json for it (unknown or missing: the text one)
read -r STYLE LOOK CLOCK24 < <(python3 -I - "$SHARE/greeter.json" "${T:-/nonexistent}" "$SHARE/wallpaper.jpg" <<'PY' 2>/dev/null || echo "text glass 0"
import json, os, sys
src, out, wall = sys.argv[1:]
try:
    g = json.load(open(src))
except (OSError, ValueError):
    g = {}
style = g.get("style") if g.get("style") in ("look", "text", "plain") else "look"
look = g.get("look") if g.get("look") in ("glass", "material") else "glass"
config = {
    "appearance": {**{k: v for k, v in (("look", look), ("mode", g.get("mode")), ("materialSeed", g.get("seed")), ("materialStyle", g.get("materialStyle"))) if isinstance(v, str)}, **({"clock24": g["clock24"]} if isinstance(g.get("clock24"), bool) else {})},
    "lock": {k: g[k] for k in ("clockFace", "clockWeight", "clockWidth", "clockRound") if k in g},
    "wallpaper": {"path": wall},
}
if os.path.isdir(out):
    os.makedirs(os.path.join(out, "config", "irice"), exist_ok=True)
    with open(os.path.join(out, "config", "irice", "config.json"), "w") as f:
        json.dump(config, f)
print(style, look, "1" if g.get("clock24") is True else "0")
PY
)

# Which folder runs it
ROOT="$SHARE/shell"
if [[ $STYLE == look && $LOOK == material && -f "$SHARE/material/shell.qml" && -n $T ]]; then
    ROOT=$(python3 -I "$SHARE/shell/assets/scripts/material-root.py" "$SHARE/shell" "$SHARE/material" "$T/material-shell" 2>/dev/null) || ROOT="$SHARE/shell"
fi

run() { # style, root
    local env=(IRICE_GREETER_STYLE="$1" IRICE_GREETER_WALLPAPER="$SHARE/wallpaper.jpg" IRICE_GREETER_CLOCK24="${CLOCK24:-0}")
    [[ -n $T ]] && env+=(XDG_CONFIG_HOME="$T/config" XDG_CACHE_HOME="$T/cache")
    env "${env[@]}" "$QS" -p "$2/greeter.qml"
}
start=$SECONDS
run "$STYLE" "$ROOT"
code=$?
if [[ $code != 0 && $STYLE != text && $((SECONDS - start)) -lt 10 ]]; then
    echo "greeter-start: the $STYLE login screen quit ($code); the text one instead" >&2
    run text "$SHARE/shell"
    code=$?
fi
exit "$code"
