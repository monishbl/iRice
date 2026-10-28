#!/usr/bin/env bash
# Screen sharing as PipeWire sees it (services/Privacy.qml), run under
# watch.sh. A share through the screen-sharing portal is a video stream node
# that xdg-desktop-portal-hyprland makes ("xdph-streaming-N"), linked to the
# app that receives it. The shell's own window previews never go through
# PipeWire, so they can't look like a share.
#
# Asking PipeWire for its node list (pw-cli) costs ~15 ms, so it's only done
# when something may have changed: the screen-sharing portal (xdph) opens
# files for every stream it makes, so a change in how many it has open (a
# free look, no process started) is the cue; and once a minute anyway. No
# xdph found: every 10 s. Only while an xdph stream exists does it take the
# graph (pw-dump), every 2 s,
# and keep just what Privacy.readGraph reads: the xdph nodes, the links out
# of them, the nodes at the other end and their clients, each reduced to
# id, type, link state and ends, and the four props it uses. That's printed
# as one line of JSON only when it changed (the full dump is ~180 KB and
# changes every time); once when the last stream goes it prints "[]". With
# no share going on it prints nothing and costs next to nothing.
#
# It ends when the shell that started it is gone, even if watch.sh couldn't
# end it (copies left by restarts used to pile up).
parent=$PPID
had=0
last=""
xdph=""
xfds=-1
checked=-60

# The portal's process: found again only when it's gone (a restart)
find_xdph() {
    local f c
    xdph=""
    for f in /proc/[0-9]*/comm; do
        read -r c 2>/dev/null <"$f" || continue
        case $c in xdg-desktop-por*) [[ $(tr '\0' ' ' <"${f%comm}cmdline" 2>/dev/null) == *hyprland* ]] && { f=${f%/comm}; xdph=${f#/proc/}; return; } ;; esac
    done
}
# Did anything change worth asking PipeWire about?
changed() {
    [ -n "$xdph" ] && [ -d "/proc/$xdph" ] || find_xdph
    if [ -z "$xdph" ]; then
        [ $((SECONDS - checked)) -ge 10 ]
        return
    fi
    local fds=(/proc/"$xdph"/fd/*)
    if [ ${#fds[@]} != "$xfds" ] || [ $((SECONDS - checked)) -ge 60 ]; then
        xfds=${#fds[@]}
        return 0
    fi
    return 1
}

# The share-relevant part of a pw-dump, as compact JSON
read -r -d '' JQ <<'EOF'
def slim: {id, type, info: {state: .info.state, "output-node-id": .info["output-node-id"], "input-node-id": .info["input-node-id"],
    props: ((.info.props // {}) | {"node.name": .["node.name"], "client.id": .["client.id"], "application.name": .["application.name"], "application.process.binary": .["application.process.binary"]})}};
[.[] | select((.type // "") | endswith(":Node")) | select((.info.props["node.name"] // "") | startswith("xdph-streaming-"))] as $x
| ($x | map(.id)) as $xi
| [.[] | select((.type // "") | endswith(":Link")) | select(.info["output-node-id"] as $o | $xi | index($o))] as $l
| ($l | map(.info["input-node-id"])) as $pi
| [.[] | select((.type // "") | endswith(":Node")) | select(.id as $i | $pi | index($i))] as $p
| ($p | map(.info.props["client.id"])) as $ci
| [.[] | select((.type // "") | endswith(":Client")) | select(.id as $i | $ci | index($i))] as $c
| [$x[], $l[], $p[], $c[]] | map(slim)
EOF
read -r -d '' PY <<'EOF'
import json, sys
g = json.load(sys.stdin)
P = lambda o: (o.get("info") or {}).get("props") or {}
T = lambda o, t: (o.get("type") or "").endswith(t)
x = [o for o in g if T(o, ":Node") and (P(o).get("node.name") or "").startswith("xdph-streaming-")]
xi = {o["id"] for o in x}
l = [o for o in g if T(o, ":Link") and (o.get("info") or {}).get("output-node-id") in xi]
pi = {o["info"].get("input-node-id") for o in l}
p = [o for o in g if T(o, ":Node") and o["id"] in pi]
ci = {P(o).get("client.id") for o in p}
c = [o for o in g if T(o, ":Client") and o["id"] in ci]
keys = ("node.name", "client.id", "application.name", "application.process.binary")
def slim(o):
    i = o.get("info") or {}
    return {"id": o.get("id"), "type": o.get("type"), "info": {"state": i.get("state"), "output-node-id": i.get("output-node-id"),
            "input-node-id": i.get("input-node-id"), "props": {k: P(o).get(k) for k in keys}}}
print(json.dumps([slim(o) for o in x + l + p + c], separators=(",", ":")))
EOF
slim() {
    if command -v jq >/dev/null 2>&1; then jq -c "$JQ"; else python3 -I -c "$PY"; fi
}

while kill -0 "$parent" 2>/dev/null; do
    if [ "$had" = 0 ] && ! changed; then
        sleep 3
        continue
    fi
    checked=$SECONDS
    if pw-cli ls Node 2>/dev/null | grep -q 'xdph-streaming-'; then
        out=$(pw-dump --no-colors 2>/dev/null | slim 2>/dev/null)
        if [ -n "$out" ] && [ "$out" != "$last" ]; then
            printf '%s\n' "$out" || exit 0
            last=$out
        fi
        had=1
        sleep 2
    else
        if [ "$had" = 1 ]; then
            echo '[]' || exit 0
            had=0
            last=""
        fi
        sleep 2
    fi
done
