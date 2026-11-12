#!/usr/bin/env bash
# parity-par.sh: the parity test (tools/parity-run.sh) split across several
# hidden sandbox arenas at once, then one report. Each arena takes every Nth
# step file and runs it on both shells in both looks.
#
#   tools/parity-par.sh --bin TESSERA_UI [--arenas 2,3,4,5,7,8] [--max-wait 8]
#                       [--steps DIR] [--looks material,glass] [--out DIR]
#
# Watch it: python3 ~/.local/share/tessera-port/prog.py live
# Never arena 1 (yours). Each arena is ~1–1.5 GB and about a core.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARENAS="2,3,4,5,7,8" MAXWAIT=8 BIN="" LOOKS="material,glass"
STEPS="$ROOT/.claude/worktrees/int/native/tessera-ui/parity/steps"
OUT="$HOME/tessera-verify/parity-full-$(date +%m%d-%H%M)"
while (($#)); do
    case "$1" in
        --bin) BIN=$2; shift ;;
        --arenas) ARENAS=$2; shift ;;
        --max-wait) MAXWAIT=$2; shift ;;
        --steps) STEPS=$2; shift ;;
        --looks) LOOKS=$2; shift ;;
        --out) OUT=$2; shift ;;
        -h|--help) sed -n 2,11p "$0"; exit 0 ;;
        *) echo "unknown option $1"; exit 2 ;;
    esac
    shift
done
[[ -x $BIN ]] || { echo "--bin: a tessera-ui build is needed"; exit 2; }
IFS=, read -ra A <<<"$ARENAS"
for a in "${A[@]}"; do [[ $a =~ ^[2-9]$ ]] || { echo "arena $a: only 2–9"; exit 2; }; done
mkdir -p "$OUT"
n=${#A[@]}
echo "$n arenas (${A[*]}), waits capped at ${MAXWAIT}s, out $OUT"
pids=()
for i in "${!A[@]}"; do
    "$ROOT/tools/parity-run.sh" --bin "$BIN" --arena "${A[i]}" --shard "$i/$n" --max-wait "$MAXWAIT" \
        --steps "$STEPS" --looks "$LOOKS" --out "$OUT" >/dev/null 2>&1 </dev/null &
    pids+=($!)
    sleep 20   # (the arenas start one after another: less load at once)
done
for p in "${pids[@]}"; do wait "$p"; done
"$ROOT/tools/parity-run.sh" --report-only --steps "$STEPS" --looks "$LOOKS" --out "$OUT" >/dev/null 2>&1 </dev/null
echo "[$(date +%H:%M:%S)] done: $OUT/report.md" | tee -a "$OUT/run-all.log"
