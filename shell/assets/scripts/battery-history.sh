#!/usr/bin/env bash
# The laptop battery's charge over the last 24 hours, from UPower's history
# (the largest history file that isn't a Bluetooth or USB device's), one
# sample every 5 minutes at most: "<unix time> <percent> <c|d>" per line.
for d in /var/lib/upower /run/host/var/lib/upower; do
    [[ -d "$d" ]] && break
done
f=$(ls -S "$d"/history-charge-*.dat 2>/dev/null | grep -v -e ':' -e 'generic_id' | head -1)
[[ -r "$f" ]] || exit 0
since=$(( $(date +%s) - 86400 ))
awk -v s="$since" '$1 >= s && $1 - last >= 300 { last = $1; printf "%d %d %s\n", $1, $2, ($3 == "charging" || $3 == "fully-charged") ? "c" : "d" }' "$f"
