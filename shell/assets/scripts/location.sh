#!/usr/bin/env bash
# Which apps are using your location (services/Privacy.qml, the blue dot),
# run under watch.sh. Prints "[app1,app2]" whenever the set changes ("[]"
# when none; "?" for an app GeoClue can't name), once at the start too.
#
# Event-driven: `gdbus monitor` follows GeoClue on the system bus (it coming
# and going, and its property changes, such as a client starting) and only
# then are GeoClue's clients asked, a second after things settle. GeoClue
# is D-Bus activated: only looked at while it's already running, so asking
# never starts it. Without gdbus, a look every 15 s. Either way, one look a
# minute as a safety net.
#
# It ends when the shell that started it is gone.
parent=$PPID
prev=x

g() { busctl --system get-property org.freedesktop.GeoClue2 "$1" "org.freedesktop.GeoClue2.$2" "$3" 2>/dev/null; }
check() {
    local cur=""
    if busctl --system list 2>/dev/null | awk '$1 == "org.freedesktop.GeoClue2" && $2 != "-" { f = 1 } END { exit !f }' &&
        g /org/freedesktop/GeoClue2/Manager Manager InUse | grep -q true; then
        cur=$(for c in $(busctl --system tree --list org.freedesktop.GeoClue2 2>/dev/null | grep /Client/); do
            g "$c" Client Active | grep -q true && g "$c" Client DesktopId | cut -d'"' -f2
        done | sort -u | paste -sd,)
        [ -z "$cur" ] && cur="?"
    fi
    if [ "$cur" != "$prev" ]; then
        echo "[$cur]" || exit 0
        prev=$cur
    fi
}

check
if command -v gdbus >/dev/null 2>&1; then
    # (dies with this script: as part of a pipeline it would otherwise wait
    # for GeoClue's next message before noticing nobody reads it any more)
    pd=(); command -v setpriv >/dev/null 2>&1 && pd=(setpriv --pdeathsig TERM --)
    "${pd[@]}" gdbus monitor --system --dest org.freedesktop.GeoClue2 2>/dev/null | {
        last=$SECONDS
        dirty=0
        while kill -0 "$parent" 2>/dev/null; do
            if IFS= read -r -t 1 _; then
                dirty=1 # (something changed: look once it's quiet)
                continue
            elif [ $? -le 128 ]; then
                break # gdbus ended: fall back to polling below
            fi
            if [ $dirty = 1 ] || [ $((SECONDS - last)) -ge 60 ]; then
                check
                dirty=0
                last=$SECONDS
            fi
        done
    }
fi
while kill -0 "$parent" 2>/dev/null; do
    sleep 15
    check
done
