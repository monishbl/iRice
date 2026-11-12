#!/usr/bin/env bash
# visual-compare.sh: the last QML version before the Rust rewrite (342a6ac,
# 4 Nov) beside today's Rust shell, screen by screen, in two hidden sandbox
# arenas at once, with the same settings (your config.json, accounts and keys
# left out), the same wallpaper and the same look. Then one HTML page to
# compare them: a slider (drag the divider), side by side, and a diff.
#
#   tools/visual-compare.sh --bin TESSERA_UI [--looks material,glass]
#                           [--qml-ref DIR] [--arenas QML,RUST] [--out DIR]
#                           [--only GLOB] [--wall FILE]
#
#   --bin      the tessera-ui to show (the Rust side)
#   --qml-ref  a checkout of the pre-Rust version (default ~/tessera-verify/qml-ref;
#              made with: git worktree add --detach ~/tessera-verify/qml-ref 342a6ac,
#              its glass plugin built with: make -C hypr/plugin/tessera-glass)
#   --sets N   N pairs of arenas at once (1–4; default 4), each taking every
#              Nth screen: QML in 2..5, Rust in 6..9 (never 1, yours)
#   --only     only the screens whose name matches (e.g. 'settings-*')
#
# The screens are tools/visual-scenes.txt (name | commands run in the
# sandbox; "-" for none). Open OUT/index.html when it's done.
# Never your screen: the arenas' hidden IRICE-TEST only.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="" LOOKS="material,glass" REF="$HOME/tessera-verify/qml-ref" SETS=4 SHARD="0/1" ARENA="" ONLY="*"
OUT="$HOME/tessera-verify/visual-$(date +%m%d-%H%M)" SIDE=""
USERCFG="$HOME/iRice/.sandbox/home/.config/tessera/config.json"
WALL=""
while (($#)); do
    case "$1" in
        --bin) BIN=$2; shift ;;
        --looks) LOOKS=$2; shift ;;
        --qml-ref) REF=$2; shift ;;
        --sets) SETS=$2; shift ;;
        --shard) SHARD=$2; shift ;;   # (internal)
        --arena) ARENA=$2; shift ;;   # (internal)
        --out) OUT=$2; shift ;;
        --only) ONLY=$2; shift ;;
        --wall) WALL=$2; shift ;;
        --side) SIDE=$2; shift ;;   # (internal: one side's run)
        -h|--help) sed -n 2,20p "$0"; exit 0 ;;
        *) echo "unknown option $1"; exit 2 ;;
    esac
    shift
done
SCENES="$ROOT/tools/visual-scenes.txt"

# ── One side: its own sandbox, its own copy of the code ─────────────────────
if [[ -n $SIDE ]]; then
    if [[ $SIDE == qml ]]; then CODE=$REF; else CODE=$ROOT; fi
    export IRICE_ARENA=$ARENA
    # shellcheck source=/dev/null
    . "$CODE/site/film/tessera/capture/lib.sh"
    CFG="$HOMEDIR/.config/tessera/config.json"
    LOG="$OUT/$SIDE-${SHARD%/*}.log"
    mkdir -p "$OUT/$SIDE"
    log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >>"$LOG"; }
    trap 'dev down >/dev/null 2>&1' EXIT
    use_config() { # look
        python3 -I - "$USERCFG" "$CFG" "$1" "$SIDE" "$OUT/wall.jpg" <<'PY'
import json, sys
src, dst, look, side, wall = sys.argv[1:]
c = json.load(open(src))
c.setdefault("appearance", {})["look"] = look
w = c.setdefault("wallpaper", {})
w.update(path=wall, darkPath=wall, lightPath=wall, dynamic="off", live="")
r = c.setdefault("rust", {})
r["standalone"] = side == "rust"
r["surfaces"] = []
for k in ("ai", "google", "cloud", "phone"):   # (accounts and keys stay out of the arena)
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
PY
    }
    reset() {
        inbox 'ipc panels close; ipc lock demo off; ipc lock unlock; ipc emoji close; ipc shotEditor close; ipc island dismissNewest; ipc island dismissNewest; wtype -k Escape; wtype -k Escape
            for a in $(hyprctl clients -j | jq -r ".[].address"); do hyprctl dispatch "hl.dsp.window.close({ window = \"address:$a\" })" >/dev/null; done; true' </dev/null >/dev/null 2>&1
        dev pointer -o IRICE-TEST move 1500 700 </dev/null >/dev/null 2>&1
        sleep 1
    }
    log "setup: arena $IRICE_ARENA, code $CODE"
    setup material "$OUT/wall.jpg" 1 >/dev/null 2>&1
    for look in ${LOOKS//,/ }; do
        log "[$look] starting the shell"
        use_config "$look"
        if [[ $SIDE == rust ]]; then
            install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
            rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"
        fi
        dev reload </dev/null >/dev/null 2>&1
        sleep 18
        reset
        k=-1
        while IFS='|' read -r name cmds; do
            name=$(echo "$name" | xargs)
            [[ -z $name || $name == \#* ]] && continue
            k=$((k + 1))
            (( k % ${SHARD#*/} == ${SHARD%/*} )) || continue
            # shellcheck disable=SC2053
            [[ $name == $ONLY ]] || continue
            cmds=$(echo "$cmds" | sed 's/^ *//')
            log "[$look] $name"
            [[ $cmds != "-" && -n $cmds ]] && inbox "$cmds" </dev/null >/dev/null 2>&1
            sleep 2.5
            dev shot "$OUT/$SIDE/$look-$name.png" </dev/null >/dev/null 2>&1
            reset
        done <"$SCENES"
    done
    log "done"
    exit 0
fi

# ── Both sides at once, then the page ───────────────────────────────────────
[[ -x $BIN ]] || { echo "--bin: a tessera-ui build is needed"; exit 2; }
[[ -f $REF/site/film/tessera/capture/lib.sh ]] || { echo "--qml-ref: no pre-Rust checkout at $REF"; exit 2; }
[[ $SETS =~ ^[1-4]$ ]] || { echo "--sets: 1 to 4"; exit 2; }
mkdir -p "$OUT"
# (one wallpaper for both: yours, dark; or --wall)
if [[ -z $WALL ]]; then
    WALL=$(python3 -c "import json;c=json.load(open('$USERCFG'));w=c.get('wallpaper',{});print(w.get('darkPath') or w.get('path') or '')")
fi
[[ -f $WALL ]] || WALL="$ROOT/site/film/tessera/capture/seed/photos/wall-sunset-lake.jpg"
cp "$WALL" "$OUT/wall.jpg"
echo "$SETS sets: QML (342a6ac) in arenas 2..$((1 + SETS)), Rust ($BIN) in 6..$((5 + SETS)); looks $LOOKS; out $OUT"
args=(--bin "$BIN" --looks "$LOOKS" --qml-ref "$REF" --out "$OUT" --only "$ONLY")
pids=()
for ((i = 0; i < SETS; i++)); do
    "$0" "${args[@]}" --side qml --arena $((2 + i)) --shard "$i/$SETS" </dev/null >/dev/null 2>&1 &
    pids+=($!)
    sleep 12   # (the arenas start one after another)
    "$0" "${args[@]}" --side rust --arena $((6 + i)) --shard "$i/$SETS" </dev/null >/dev/null 2>&1 &
    pids+=($!)
    sleep 12
done
for p in "${pids[@]}"; do wait "$p"; done
python3 -I "$ROOT/tools/visual-report.py" "$OUT" "$SCENES"
echo "done: $OUT/index.html"
