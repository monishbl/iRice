#!/usr/bin/env bash
# Starts the Tessera login screen in the greeter's compositor (hypr/greeter.lua
# runs it as greetd's "greeter" user, from ./install.sh --greeter's copy in
# /usr/share/tessera). Its look comes from greeter.json there (Settings → Lock
# Screen → Login screen writes it through tessera-root login-look): the style,
#   look   the look you last used: Glass (shell/) or Material (its root,
#          assembled here from shell/ and material/)
#   text   ly's: a box of fields in monospace, keyboard first
#   plain  black, your picture, the field and the fingerprint
# and the colours and clock that look needs, as a config.json of its own
# (the greeter can't read your home). A login screen that doesn't start
# (quits with an error within 10 s) gives way to the text one, so there's
# always a way in. Returns when the greeter quits (you logged in).
#   greeter-start.sh            (TESSERA_SHARE: another copy, for tests)
set -u
SHARE="${TESSERA_SHARE:-/usr/share/tessera}"
QS="${TESSERA_QS:-qs}"
T="$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/tessera-greeter.XXXXXX")" || T=""
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
    os.makedirs(os.path.join(out, "config", "tessera"), exist_ok=True)
    with open(os.path.join(out, "config", "tessera", "config.json"), "w") as f:
        json.dump(config, f)
print(style, look, "1" if g.get("clock24") is True else "0")
PY
)

# Which folder runs it
ROOT="$SHARE/shell"
if [[ $STYLE == look && $LOOK == material && -f "$SHARE/material/shell.qml" && -n $T ]]; then
    ROOT=$(python3 -I "$SHARE/shell/assets/scripts/material-root.py" "$SHARE/shell" "$SHARE/material" "$T/material-shell" 2>/dev/null) || ROOT="$SHARE/shell"
fi

genv() { # style
    local env=(TESSERA_GREETER_STYLE="$1" TESSERA_GREETER_WALLPAPER="$SHARE/wallpaper.jpg" TESSERA_GREETER_CLOCK24="${CLOCK24:-0}")
    [[ -n $T ]] && env+=(XDG_CONFIG_HOME="$T/config" XDG_CACHE_HOME="$T/cache")
    printf '%s\0' "${env[@]}"
}
run() { # style, root: the QML login screen
    local env
    mapfile -d '' env < <(genv "$1")
    env "${env[@]}" "$QS" -p "$2/greeter.qml"
}
# The login screen in Rust (tessera-ui --greeter), when install.sh copied
# tessera-ui here (TESSERA_GREETER_UI: another one, for tests)
UI="${TESSERA_GREETER_UI:-$SHARE/bin/tessera-ui}"
run_rust() { # style
    local env
    mapfile -d '' env < <(genv "$1")
    env "${env[@]}" "$UI" --greeter --style "$1" --look "$LOOK"
}
start=$SECONDS
if [[ -x $UI ]]; then
    run_rust "$STYLE"
    code=$?
    # (one that quits with an error at once gives way to the QML one)
    [[ $code == 0 || $((SECONDS - start)) -ge 10 ]] && exit "$code"
    echo "greeter-start: the Rust login screen quit ($code); the QML one instead" >&2
    start=$SECONDS
fi
run "$STYLE" "$ROOT"
code=$?
if [[ $code != 0 && $STYLE != text && $((SECONDS - start)) -lt 10 ]]; then
    echo "greeter-start: the $STYLE login screen quit ($code); the text one instead" >&2
    run text "$SHARE/shell"
    code=$?
fi
exit "$code"
