#!/usr/bin/env bash
# File copies in progress, for the island's transfer activity. Every 2 s
# (1 s while something is copying) looks for cp / mv / rsync / dd / install
# / pv, and for each, the file it is reading and how far it got (from
# /proc/<pid>/fdinfo). Prints a JSON array when the set changes or progresses:
#   [{"pid":123,"cmd":"cp","file":"/path/big.iso","pos":1048576,"size":4194304}]
# and [] once when copying ends. Exits when stdin closes (the shell quit).
prev=""
while :; do
    items=()
    for p in $(pgrep -x 'cp|mv|rsync|dd|install|pv|gcp' 2>/dev/null); do
        cmd=$(cat "/proc/$p/comm" 2>/dev/null) || continue
        best="" bpos=0 bsize=0
        for fd in /proc/"$p"/fd/*; do
            f=$(readlink "$fd" 2>/dev/null) || continue
            [[ -f "$f" ]] || continue
            info=$(<"/proc/$p/fdinfo/${fd##*/}") 2>/dev/null || continue
            flags=$(sed -n 's/^flags:[[:space:]]*//p' <<<"$info")
            (( (8#${flags:-0} & 3) == 0 )) || continue # the file being read
            pos=$(sed -n 's/^pos:[[:space:]]*//p' <<<"$info")
            size=$(stat -c %s "$f" 2>/dev/null) || continue
            (( size > bsize )) && { best=$f; bpos=$pos; bsize=$size; }
        done
        [[ -n "$best" ]] || continue
        f=${best//\\/\\\\}; f=${f//\"/\\\"}
        items+=("{\"pid\":$p,\"cmd\":\"$cmd\",\"file\":\"$f\",\"pos\":${bpos:-0},\"size\":$bsize}")
    done
    out="[$(IFS=,; echo "${items[*]}")]"
    if [[ "$out" != "$prev" ]]; then
        echo "$out"
        prev=$out
    fi
    # Stop with the shell
    read -t "$( ((${#items[@]})) && echo 1 || echo 2 )" _ || [[ $? -gt 128 ]] || exit 0
done
