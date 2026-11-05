#!/usr/bin/env bash
# Screenshots of the sidebar, QML and Rust in the same states, in the hidden
# sandbox arena 9 (never the real screen), for review/sidebar/:
#   tools/sidebar-shots.sh <glass|material> <qml|rust> [wallpaper]
# This checkout's shell runs in the arena (its .sandbox linked to the main
# checkout's), so the Rust sidebar opens as it would for anyone: Settings'
# rust.surfaces, RustUi.qml, Panels. The AI and the Anime tab talk to
# tools/sidebar-mock.py on 127.0.0.1:47391 only (canned answers and
# failures; no real model, no key): the Rust one's Anime tab through
# TESSERA_BOORU_MOCK, the QML one's never opened (it can't be
# pointed at a mock).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export IRICE_ARENA=9
. "$HERE/../../site/film/tessera/capture/lib.sh"
# (the film's photos stay in the main checkout: not in git)
[[ -f $WALLS/wall-alpine-lake.jpg ]] || WALLS=/home/user/Projects/Personal/iRice/site/film/tessera/capture/seed/photos
look=$1 which=$2 wall=${3:-$WALLS/wall-alpine-lake.jpg}
OUT="$HERE/review/sidebar"
PORT=47391
mkdir -p "$OUT"
shot() { sleep "${2:-1}"; dev shot "$OUT/$which-$look-$1.png" >/dev/null && echo "shot $which-$look-$1"; }

# The mock, on the host (the arena shares its network), stopped on exit
python3 -I "$HERE/tools/sidebar-mock.py" $PORT "$WALLS" & MOCK=$!
trap 'kill $MOCK 2>/dev/null' EXIT

setup "$look" "$wall" 1

# tessera-ui for the arena: this checkout's build, the Anime tab on the mock
BIN="$HOMEDIR/.local/share/tessera/bin/tessera-ui"
mkdir -p "$(dirname "$BIN")"
rm -f "$BIN"
printf '#!/bin/sh\nTESSERA_BOORU_MOCK=http://127.0.0.1:%s exec %s "$@"\n' $PORT "$HERE/target/${PROFILE:-release}/tessera-ui" >"$BIN"
chmod +x "$BIN"

# Settings for a take (the shell reads config.json live)
cfg() {
    python3 -I - "$HOMEDIR/.config/irice/config.json" "$which" "$1" $PORT <<'EOF'
import json, sys
path, which, model, port = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
def m(id, name, endpoint, auth):
    d = {"id": id, "name": name, "provider": "custom", "endpoint": endpoint, "project": "", "location": "", "auth": auth,
         "thinking": False, "maxTokens": 0, "vision": True}
    d["key"] = "|".join(["custom", id, endpoint, auth, "", ""]).replace(":", "%3A").replace("/", "%2F")
    return d
models = {"atlas": m("atlas-8b", "Atlas 8B", f"http://127.0.0.1:{port}/v1", "none"),
          "keyless": m("orbit-2", "Orbit 2", f"http://127.0.0.1:{port}/v1", "key"),
          "offline": m("atlas-8b", "Atlas 8B", "http://127.0.0.1:47399/v1", "none")}
c = json.load(open(path))
c.setdefault("rust", {})["surfaces"] = ["sidebar"] if which == "rust" else []
c.setdefault("sidebar", {}).update(booru=True, booruProvider="yandere", booruNsfw=False)
ai = c.setdefault("ai", {})
if model == "none":
    ai.update(models=[], model="")
else:
    ai.update(models=[json.dumps(models[model])], model=models[model]["key"], keepHistory=True)
json.dump(c, open(path, "w"), indent=4)
EOF
}

# The header's buttons: past chats and New chat (screen points, at 1x)
if [[ $look == material ]]; then HDR="${HDR:-358 108 400 108}"; else HDR="${HDR:-362 104 400 104}"; fi
read -r HX HY NX NY <<<"$HDR"
# (a new chat: the composer's /new, whatever the header shows)
new_chat() { inbox 'key "/new"; key -k Return'; sleep 1; }

cfg atlas
sleep 6
open_sb() { inbox "ipc panels tab ${1:-0}; ipc panels open sidebar IRICE-TEST"; sleep 2; }
close_sb() { inbox 'ipc panels close'; sleep 1.5; }

open_sb 0
shot empty 1
inbox 'key "Plan a slow weekend in Lisbon"; key -k Return'
shot streaming 4
shot answer 22
inbox 'key "This one will fail"; key -k Return'
shot error 3
# Past chats: the header's clock (the header's right end, left of New chat)
inbox "P move $HX $HY click"
shot history 2
inbox 'key -k Down; key -k Down'
shot history-keys 1
inbox 'key -k Escape'
close_sb
# A screenshot for the chat (Screen tools: it comes back attached)
inbox 'ipc screen ask'
sleep 4
shot attached 1
close_sb

cfg keyless
sleep 3
open_sb 0
new_chat
shot needs-key 1
close_sb

cfg offline
sleep 3
open_sb 0
new_chat
inbox 'key "Hello there"; key -k Return'
shot offline 4
close_sb

cfg none
sleep 3
open_sb 0
new_chat
shot no-model 1
close_sb

cfg atlas
sleep 2
open_sb 1
inbox 'key "Good morning"'
shot translate 5
close_sb

if [[ $which == rust ]]; then
    open_sb 2
    shot anime 5
    inbox 'key "nothing at all"; key -k Return'
    shot anime-none 3
    close_sb
fi

# (this arena's tessera-ui: its log, then off again)
inbox 'tail -n 15 $XDG_RUNTIME_DIR/quickshell.log 2>/dev/null | grep -i tessera-ui'
python3 -I - "$HOMEDIR/.config/irice/config.json" <<'EOF'
import json, sys
c = json.load(open(sys.argv[1])); c.setdefault("rust", {})["surfaces"] = []
json.dump(c, open(sys.argv[1], "w"), indent=4)
EOF
