#!/usr/bin/env bash
# Screen sharing as PipeWire sees it (services/Privacy.qml), run under
# watch.sh. A share through the screen-sharing portal is a video stream node
# that xdg-desktop-portal-hyprland makes ("xdph-streaming-N"), linked to the
# app that receives it. The shell's own window previews never go through
# PipeWire, so they can't look like a share.
#
# Watches the graph (pw-dump --monitor, event-driven), and half a second
# after it settles prints the whole graph as one line of JSON, but only while
# an xdph stream exists, plus once when the last one goes ("[]"). With no
# share going on it prints nothing at all.
had=0
dirty=1
pw-dump --monitor --no-colors 2>/dev/null | while :; do
    IFS= read -r -t 0.5 _
    rc=$?
    if [ $rc = 0 ]; then
        dirty=1
        continue
    fi
    [ $rc -gt 128 ] || exit 0 # pw-dump ended (PipeWire restarted): Privacy.qml starts this again
    [ "$dirty" = 1 ] || continue
    dirty=0
    out=$(pw-dump --no-colors 2>/dev/null) || continue
    if [[ "$out" == *xdph-streaming-* ]]; then
        printf '%s\n' "$out" | tr -d '\n'
        echo
        had=1
    elif [ "$had" = 1 ]; then
        echo '[]'
        had=0
    fi
done
