#!/usr/bin/env bash
# parity-run.sh: the deterministic parity test. Plays every step file
# (native/tessera-ui/parity/steps/*.steps, the DSL below) on the QML shell and
# on tessera-ui --standalone, in both looks, in one hidden sandbox arena set up
# with YOUR settings (config.json copied in, accounts and keys left out).
# No AI drives anything: it moves the virtual pointer, types, runs IPC, takes
# paired screenshots and recordings, times animations and checks regions, and
# writes one report (report.md + side-by-side pictures) for a person or one
# reviewing agent to judge.
#
#   tools/parity-run.sh --bin TESSERA_UI [--steps DIR]... [--only GLOB]
#                       [--looks material,glass] [--arena N] [--out DIR]
#
#   --bin     the tessera-ui build to test (e.g. the integration build)
#   --steps   a folder of .steps files (repeatable; default: every worktree's
#             and this checkout's native/tessera-ui/parity/steps)
#   --only    run only step files whose name matches this glob (e.g. 'Island*')
#
# The step DSL, one action per line (# comments):
#   look material|glass       the lines after it run only in that look (until
#                             the next look line); lines before any run in both
#   setup <cmd>               a shell command inside the arena
#   ipc <args>                the shell's IPC (QML: qs ipc, Rust: tessera-ui ipc)
#   ptr <path>                one virtual-pointer call: move X Y sleep MS glide X Y MS click down up scroll DX DY
#   key <args>                wtype -d 30 <args>
#   wait <s>
#   shot <name>               a screenshot (paired QML | Rust in the report)
#   rec <name> <s> <step>     record while running one ipc/ptr/key step; timed
#   static <name> x y w h     the region didn't change in recording <name>
#   note <text>               what to check, for the reviewer
#
# Never your screen: the arena's headless IRICE-TEST only. Takes a while: run
# it when you're not using the laptop for anything heavy.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PX="" PY="" ARENA=7 BIN="" ONLY="*" LOOKS="material,glass" STEPDIRS=()
OUT="$HOME/tessera-verify/parity-$(date +%Y%m%d-%H%M)"
USERCFG="$HOME/iRice/.sandbox/home/.config/tessera/config.json"
while (($#)); do
    case "$1" in
        --bin) BIN=$2; shift ;;
        --steps) STEPDIRS+=("$2"); shift ;;
        --only) ONLY=$2; shift ;;
        --looks) LOOKS=$2; shift ;;
        --arena) ARENA=$2; shift ;;
        --out) OUT=$2; shift ;;
        --config) USERCFG=$2; shift ;;
        -h|--help) sed -n 2,34p "$0"; exit 0 ;;
        *) echo "unknown option $1 (--help)"; exit 2 ;;
    esac
    shift
done
[[ -x $BIN ]] || { echo "--bin: a tessera-ui build is needed"; exit 2; }
[[ $ARENA =~ ^[2-9]$ ]] || { echo "--arena must be 2–9"; exit 2; }
if ((${#STEPDIRS[@]} == 0)); then
    for d in "$ROOT"/native/tessera-ui/parity/steps "$ROOT"/.claude/worktrees/*/native/tessera-ui/parity/steps; do
        [[ -d $d && $d != *"/worktrees/int/"* ]] && STEPDIRS+=("$d")
    done
fi
export IRICE_ARENA=$ARENA
# shellcheck source=/dev/null
. "$ROOT/site/film/tessera/capture/lib.sh"
CFG="$HOMEDIR/.config/tessera/config.json"
mkdir -p "$OUT/shots" "$OUT/recs"
STEPLOG="$OUT/steps.log"   # (every step and what it printed: a setup that failed shows here)
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$OUT/run.log"; }
trap 'dev down >/dev/null 2>&1' EXIT
# (step files by name; the same name in two folders: the first folder wins)
declare -A FILES
for d in "${STEPDIRS[@]}"; do
    for f in "$d"/*.steps; do
        [[ -f $f ]] || continue
        n=$(basename "$f" .steps)
        # shellcheck disable=SC2053
        [[ $n == $ONLY ]] || continue
        [[ -n ${FILES[$n]:-} ]] || FILES[$n]=$f
    done
done
mapfile -t NAMES < <(printf '%s\n' "${!FILES[@]}" | sort)
log "${#NAMES[@]} step files, looks $LOOKS, arena $ARENA, build $BIN"

use_config() { # look standalone(0|1)
    python3 -I - "$USERCFG" "$CFG" "$1" "$2" <<'PY'
import json, sys
src, dst, look, sa = sys.argv[1:]
c = json.load(open(src))
c.setdefault("appearance", {})["look"] = look
r = c.setdefault("rust", {})
r["standalone"] = sa == "1"
r["surfaces"] = []
for k in ("ai", "google", "cloud", "phone"):   # (accounts and keys stay out of the arena)
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
PY
}
start_shell() { # look pass
    use_config "$1" "$([[ $2 == rust ]] && echo 1 || echo 0)"
    cp "$CFG" "$OUT/.config-base.json"
    install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"   # (the launcher's 3-crash fallback)
    dev reload </dev/null >/dev/null 2>&1
    sleep 12
    inbox 'ipc lock unlock; ipc panels close' </dev/null >/dev/null 2>&1
    sleep 2
}
fresh() { # between files: settings as they were, no panels, no lock, no test windows, pointer parked
    cmp -s "$OUT/.config-base.json" "$CFG" || { cp "$OUT/.config-base.json" "$CFG"; sleep 1; }
    inbox 'ipc panels close; ipc lock unlock; wtype -k Escape; wtype -k Escape
        for a in $(hyprctl clients -j | jq -r ".[].address"); do hyprctl dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; done
        pkill -x fakeplayer.py 2>/dev/null; true' </dev/null >/dev/null 2>&1
    dev pointer -o IRICE-TEST move 1500 600 </dev/null >/dev/null 2>&1
    PX=1500 PY=600
    sleep 1
}
do_step() { # verb args… (ipc/ptr/key/setup: one action)
    local verb=$1; shift
    case "$verb" in
        ipc) inbox "ipc $*" </dev/null >>"$STEPLOG" 2>&1 ;;
        ptr) # (each vptr call starts at the screen centre: start where the last one ended)
            local a=("$@") i
            [[ -n $PX ]] && a=(move "$PX" "$PY" "${a[@]}")
            dev pointer -o IRICE-TEST "${a[@]}" </dev/null >/dev/null 2>&1
            for ((i = 0; i < ${#a[@]}; i++)); do
                case "${a[i]}" in move|glide) PX=${a[i+1]} PY=${a[i+2]} ;; esac
            done ;;
        key) dev run wtype -d 30 "$@" </dev/null >>"$STEPLOG" 2>&1 ;;
        setup) inbox "$*" </dev/null >>"$STEPLOG" 2>&1 || echo "  (exit $?)" >>"$STEPLOG" ;;
    esac
}
run_file() { # name look pass
    local name=$1 look=$2 pass=$3 f=${FILES[$1]} line verb rest n s section=both
    fresh
    while IFS= read -r line || [[ -n $line ]]; do
        line="${line%%$'\r'}"
        line="${line//\$TESSERA_BIN/\$HOME/.local/share/tessera/bin/tessera-ui}"
        [[ -z ${line// } || $line == \#* ]] && continue
        read -r verb rest <<<"$line"
        if [[ $verb == look ]]; then section=$rest; continue; fi
        [[ $section == both || $section == "$look" ]] || continue
        printf '%s %s/%s %s: %s\n' "$(date +%T)" "$look" "$pass" "$name" "${line:0:160}" >>"$STEPLOG"
        case "$verb" in
            note) ;;
            setup|ipc|key) do_step "$verb" $rest ;;
            ptr) # shellcheck disable=SC2086
                do_step ptr $rest ;;
            wait) sleep "$rest" ;;
            shot) dev shot "$OUT/shots/$look-$name-$rest-$pass.png" </dev/null >/dev/null 2>&1 ;;
            rec)
                read -r n s rest <<<"$rest"
                local mp4="/tmp/parity-rec-$n.mp4"
                inbox "rm -f $mp4; wf-recorder -o IRICE-TEST -r 60 -f $mp4 >/dev/null 2>&1 & echo \$! > /tmp/parity-rec.pid" </dev/null >/dev/null 2>&1
                sleep 0.6
                # shellcheck disable=SC2086
                do_step $rest
                sleep "$s"
                inbox "kill -INT \$(cat /tmp/parity-rec.pid) 2>/dev/null; sleep 0.5; cp $mp4 '$OUT/recs/$look-$name-$n-$pass.mp4' 2>/dev/null" </dev/null >/dev/null 2>&1
                python3 "$ROOT/tools/anim-timing.py" "$OUT/recs/$look-$name-$n-$pass.mp4" >"$OUT/recs/$look-$name-$n-$pass.txt" 2>&1
                ;;
            static)
                read -r n rest <<<"$rest"
                # shellcheck disable=SC2086
                python3 "$ROOT/tools/region-changes.py" "$OUT/recs/$look-$name-$n-$pass.mp4" $rest >"$OUT/recs/$look-$name-$n-$pass.static.txt" 2>&1
                ;;
            *) echo "$name: unknown step '$verb'" >>"$OUT/run.log" ;;
        esac
    done <"$f"
}

log "arena setup (your settings from $USERCFG)"
setup material "$WALLS/wall-tiles.jpg" 1
for look in ${LOOKS//,/ }; do
    for pass in qml rust; do
        log "[$look] $pass pass"
        start_shell "$look" "$pass"
        for name in "${NAMES[@]}"; do
            # (a file with look sections but none for this look: skipped)
            if grep -qE '^look ' "${FILES[$name]}" && ! grep -qE "^look $look\b" "${FILES[$name]}"; then
                [[ -n $(sed -n '/^look /q;/^[a-z]/p' "${FILES[$name]}") ]] || continue
            fi
            run_file "$name" "$look" "$pass"
        done
    done
done

log "pairing and report"
python3 -I - "$OUT" "${!FILES[@]}" <<'PY'
import os, re, subprocess, sys
out = sys.argv[1]
shots, recs = os.path.join(out, "shots"), os.path.join(out, "recs")
lines = ["# Parity run", "", f"Folder: {out}", "",
         "Each shot is paired QML | Rust (`*-pair.jpg`). Recordings: when the change starts, how long it lasts, frames, longest gap between frames. `static`: changed frames in a region that must not change (0 = pass).", ""]
def txt(p):
    try:
        return open(p).read().strip().splitlines()[-1]
    except (OSError, IndexError):
        return "—"
names = sorted(set(sys.argv[2:]))
for name in names:
    rows = []
    for f in sorted(os.listdir(shots)):
        m = re.match(rf"(material|glass)-{re.escape(name)}-(.+)-qml\.png$", f)
        if not m:
            continue
        look, shot = m.groups()
        q, r = os.path.join(shots, f), os.path.join(shots, f.replace("-qml.png", "-rust.png"))
        pair = os.path.join(shots, f.replace("-qml.png", "-pair.jpg"))
        if os.path.exists(r):
            subprocess.run(["magick", q, r, "+append", "-resize", "2400x", pair], capture_output=True)
            d = subprocess.run(["magick", "compare", "-metric", "RMSE", q, r, "null:"], capture_output=True, text=True).stderr
            m2 = re.search(r"\(([\d.]+)\)", d)
            rows.append(f"| {look} | shot `{shot}` | {float(m2.group(1)) * 100:.1f}% different | `{os.path.relpath(pair, out)}` |" if m2 else f"| {look} | shot `{shot}` | ? | `{os.path.relpath(pair, out)}` |")
        else:
            rows.append(f"| {look} | shot `{shot}` | **no Rust shot** | |")
    for f in sorted(os.listdir(recs)):
        m = re.match(rf"(material|glass)-{re.escape(name)}-(.+)-qml\.txt$", f)
        if not m or f.endswith(".static.txt"):
            continue
        look, rec = m.groups()
        q, r = txt(os.path.join(recs, f)), txt(os.path.join(recs, f.replace("-qml.txt", "-rust.txt")))
        rows.append(f"| {look} | rec `{rec}` | QML: {q.split(': ', 1)[-1]} | Rust: {r.split(': ', 1)[-1]} |")
    for f in sorted(os.listdir(recs)):
        m = re.match(rf"(material|glass)-{re.escape(name)}-(.+)-(qml|rust)\.static\.txt$", f)
        if m:
            look, rec, pass_ = m.groups()
            t = txt(os.path.join(recs, f))
            n = re.search(r": (\d+) changed frames", t)
            ok = n and n.group(1) == "0"
            rows.append(f"| {look} | static `{rec}` ({pass_}) | {'PASS' if ok else '**FAIL**'} | {t.split(':', 1)[-1].strip()[:80]} |")
    if rows:
        lines += [f"## {name}", "", "| look | what | result | detail |", "|---|---|---|---|", *rows, ""]
lines += ["", "Every step and its output: `steps.log`. `note` lines and `# MANUAL:` comments in the step files say what to look for."]
open(os.path.join(out, "report.md"), "w").write("\n".join(lines) + "\n")
PY
# (the recordings are big: keep only their timings)
rm -f "$OUT"/recs/*.mp4
log "done: $OUT/report.md"
