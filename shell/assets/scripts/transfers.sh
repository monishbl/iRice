#!/usr/bin/env bash
# File copies in progress, for the island's transfer activity, run under
# watch.sh. Every 10 s (2 s while something is copying) looks for cp / mv /
# rsync / dd / install / pv, and for each, the file it is reading and how far
# it got (from /proc/<pid>/fdinfo). Prints a JSON array when the set changes
# or progresses:
#   [{"pid":123,"cmd":"cp","file":"/path/big.iso","pos":1048576,"size":4194304}]
# and [] once when copying ends. It ends when the shell that started it is
# gone. (Finding the copiers reads each process's name with shell built-ins,
# about 10 ms, a third of what pgrep costs.)
parent=$PPID
cps=()
copiers() { # (into cps: no subshell)
    local f c
    cps=()
    for f in /proc/[0-9]*/comm; do
        read -r c 2>/dev/null <"$f" || continue
        case $c in cp | mv | rsync | dd | install | pv | gcp) f=${f%/comm}; cps+=("${f#/proc/}") ;; esac
    done
}
prev=""
while kill -0 "$parent" 2>/dev/null; do
    items=()
    copiers
    for p in "${cps[@]}"; do
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
        echo "$out" || exit 0
        prev=$out
    fi
    if ((${#items[@]})); then sleep 2; else sleep 10; fi
done
