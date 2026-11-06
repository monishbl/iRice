#!/usr/bin/env bash
# verify-rust-ui.sh: the deterministic check of tessera-ui against the QML
# shell. One hidden, silent sandbox arena (never your screen): every surface
# state is captured twice with the same seed, wallpaper and settings, first
# drawn by the QML shell, then by tessera-ui, in both looks; each pair gets a
# difference figure, a side-by-side and a difference picture. Then every
# crate's tests and the Python suite. Everything lands in one folder with
# report.md (read it, or hand it to Claude) and report.json.
#
#   tools/verify-rust-ui.sh [--arena N] [--looks glass,material] [--only a,b]
#                           [--standalone] [--skip-tests] [--skip-build]
#                           [--with-network] [--out DIR]
#
#   --standalone     the second pass is tessera-ui as the whole shell (wave 4:
#                    rust.standalone, no Quickshell at all) instead of Rust
#                    surfaces beside the QML; adds the standalone checks (it
#                    started alone, its IPC, calls nobody answered, crashes,
#                    memory) and the host crates' tests
#
#   --arena N        the sandbox arena (2–9, default 6; 1 is your own and refused)
#   --only a,b       just these surfaces (default: all below)
#   --with-network   also the Wi-Fi menu: it lists the networks around you, so
#                    it's off by default (the pictures stay on this computer)
#
# Takes about 30–60 minutes (builds and tests run at the lowest priority,
# memory-capped, through ~/.local/share/tessera-port/slot.sh when present).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARENA=6 LOOKS="glass,material" ONLY="" TESTS=1 BUILD=1 NETWORK=0 STANDALONE=0 BUILD_FAILED=0
OUT="$HOME/tessera-verify/$(date +%Y%m%d-%H%M)"
while (($#)); do
    case "$1" in
        --arena) ARENA=$2; shift ;;
        --looks) LOOKS=$2; shift ;;
        --only) ONLY=$2; shift ;;
        --skip-tests) TESTS=0 ;;
        --skip-build) BUILD=0 ;;
        --with-network) NETWORK=1 ;;
        --standalone) STANDALONE=1 ;;
        --out) OUT=$2; shift ;;
        -h|--help) sed -n 2,26p "$0"; exit 0 ;;
        *) echo "unknown option $1 (--help)"; exit 2 ;;
    esac
    shift
done
[[ $ARENA =~ ^[2-9]$ ]] || { echo "--arena must be 2–9 (1 is your own session's sandbox)"; exit 2; }
export IRICE_ARENA=$ARENA
# shellcheck source=/dev/null
. "$ROOT/site/film/tessera/capture/lib.sh"   # setup, inbox, dev (this arena)
CFG="$HOMEDIR/.config/tessera/config.json"
BIN="$ROOT/native/tessera-ui/target/release/tessera-ui"
GATE="$HOME/.local/share/tessera-port/slot.sh"
mkdir -p "$OUT/shots" "$OUT/logs"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$OUT/run.log"; }
cleanup() { log "bringing arena $ARENA down"; dev down >/dev/null 2>&1; }
trap cleanup EXIT

# Builds and tests never see a display and never take the machine
run_cargo() {
    if [[ -x $GATE ]]; then SLOT_JOBS=2 SLOT_MEM_HIGH=5G SLOT_MEM_MAX=6G RUST_MIN_STACK=67108864 "$GATE" "$@"
    else env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DISPLAY CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 nice -n 19 "$@"; fi
}

# ── The states: surface | name | what to do in the sandbox | seconds to wait ──
# (panels open through the shell as a shortcut would; every state is closed
# again before the next: Esc, panels close, lock unlock)
SCREEN=IRICE-TEST
STATES=$(cat <<EOF
menubar|idle|:|1.5
menubar|system-menu|ipc panels open systemMenu $SCREEN|1.5
menubar|window-menu|ipc panels open windowMenu $SCREEN|1.5
menubar|battery-menu|ipc panels open batteryMenu $SCREEN|1.5
dock|idle|:|1
osd|volume|ipc osd preview volume|1
osd|brightness|ipc osd preview brightness|1
emoji|open|ipc emoji open|2.5
clipboard|open|ipc panels open clipboard $SCREEN|2
spotlight|open|ipc panels open spotlight $SCREEN|2
spotlight|calculator|ipc panels open spotlight $SCREEN; sleep 1; key '2*3'|2
controlcenter|open|ipc panels open controlCenter $SCREEN|2
sidebar|open|ipc panels open sidebar $SCREEN|2.5
notifications|banner|notify-send -a Parcel 'Your order is on its way' 'It arrives on Thursday'|1.5
notifications|center|ipc panels open notificationCenter $SCREEN|2
today|open|ipc today preview bloom|3.5
wallpaper|picker|ipc panels open wallpaper $SCREEN|2
desktop|widgets|ipc panels widgets true|2
screentools|toolbar|ipc panels open screenTools $SCREEN|2
quicklook|picture|ipc panels quickLook $WALLS/wall-tiles.jpg|2.5
session|menu|ipc panels open session $SCREEN|2
polkit|request|ipc polkit preview|2
lock|idle|ipc lock lock|3
lock|typing|ipc lock lock; sleep 2; ipc lock demo typing|2
lock|wrong|ipc lock lock; sleep 2; ipc lock demo wrong|2
lock|fingerprint|ipc lock lock; sleep 2; ipc lock demo finger|2
lock|bedside|ipc lock lock; sleep 2; ipc lock demo standby|3
lock|login-intro|ipc loginIntro play|0.5
overlay|open|ipc panels open overlay $SCREEN|2
resources|menu|ipc panels open resourcesMenu $SCREEN|2
cheatsheet|open|ipc panels open cheatsheet $SCREEN|2
osk|open|ipc panels open keyboard $SCREEN|2
share|ask|ipc sharingdemo ask|2
share|sending|ipc sharingdemo sending 40|2
shelf|open|ipc shelf open|2
captions|demo|ipc captions demo 'Hello there' 'This is a test of live captions'|2
notes|open|ipc panels open quickNote $SCREEN|2
network|wifi-menu|ipc panels open wifiMenu $SCREEN|2.5
settings|appearance|ipc panels settings appearance|3
appstore|discover|ipc panels appStore|4
overview|open|ipc panels open overview $SCREEN|2.5|w
missioncontrol|open|ipc panels open missionControl $SCREEN|2.5|w
switcher|open|ipc panels open switcher $SCREEN|1.5|w
launchpad|open|ipc panels open launchpad $SCREEN|2.5|w
welcome|open|ipc welcome open|2.5
findpointer|ring|ipc findpointer find|0.6|s
spaceswitch|cue|ipc cues space $SCREEN glow|0.8|s
displayconnect|extend|ipc displays preview extend right|1.2|s
displayoverlay|identify|ipc cues identify|1.5|s
displayoverlay|confirm|ipc cues confirm|1.5|s
EOF
)
# (a 5th field: w = needs test windows open; s = standalone only, wave 4b's
# surfaces with no QML command to show them: shot in the standalone pass only,
# listed as "new (no QML)")
# (behaviour without a picture of its own: hotcorners, tiling, decorations,
# stagemanager, tabs, background (it's in every shot); the greeter has its own
# script: native/tessera-ui/tools/greeter-shots.sh)

want() { # surface → wanted?
    [[ $1 == network && $NETWORK == 0 ]] && return 1
    [[ -z $ONLY ]] || [[ ",$ONLY," == *",$1,"* ]]
}
surfaces_in_use() {
    printf '%s\n' "$STATES" | while IFS='|' read -r s _; do want "$s" && echo "$s"; done | sort -u | paste -sd, -
}
set_rust() { # comma list (empty: everything QML) [standalone 0|1]
    python3 -I - "$CFG" "$1" "${2:-0}" <<'PY'
import json, sys
p, names = sys.argv[1], [n for n in sys.argv[2].split(",") if n]
try:
    c = json.load(open(p))
except FileNotFoundError:
    c = {}
c.setdefault("rust", {})["surfaces"] = names
c["rust"]["standalone"] = sys.argv[3] == "1"  # (wave 4: tessera-ui as the whole shell)
# (one Material seed and accent for both passes: the sandbox has no matugen,
# so the QML would fall back to its rough pick while Rust computes matugen's)
a = c.setdefault("appearance", {})
a["materialSeed"] = "#3b6ea5"
a["accent"] = "blue"
json.dump(c, open(p, "w"), indent=4)
PY
}
quiet() { inbox </dev/null "ipc island holdAlerts true; ipc panels close; key -k Escape; key -k Escape; ipc lock unlock; ipc shelf close; ipc captions off; ipc today close; ipc panels widgets false; makoctl dismiss -a 2>/dev/null; true" >/dev/null 2>&1; sleep 0.8; }

# ── 1. Build ────────────────────────────────────────────────────────────────
{
    echo "commit $(git -C "$ROOT" rev-parse --short HEAD) ($(git -C "$ROOT" log -1 --format=%s | cut -c1-80))"
    echo "dirty files: $(git -C "$ROOT" status --porcelain | grep -vc '^??')"
    echo "date $(date -Is), arena $ARENA, looks $LOOKS, mode $( ((STANDALONE)) && echo standalone || echo 'surfaces beside QML')"
    echo "rustc $(rustc --version 2>/dev/null | cut -d' ' -f2)"
} > "$OUT/env.txt"
if ((BUILD)); then
    log "building tessera-ui (release, low priority)"
    avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
    ((avail < 4500)) && log "  only $avail MB free: the build waits for 4000 MB (it says so in logs/build.log each minute). Close a big app, or run with MIN_FREE_MB=3000"
    (cd "$ROOT/native/tessera-ui" && run_cargo cargo build --release --keep-going --bin tessera-ui) > "$OUT/logs/build.log" 2>&1 \
        || { log "build FAILED: every error is in report.md (full log: logs/build.log)"; BUILD_FAILED=1; }
fi
# (--keep-going: every crate's errors at once; not --locked: new crates add
# their lines to Cargo.lock)
[[ -x $BIN ]] || { log "no tessera-ui binary: nothing to compare"; BUILD_FAILED=1; }
# (a failed build never compares an older binary: straight to the report)
((BUILD_FAILED)) && TESTS=0

# ── 2. Capture ──────────────────────────────────────────────────────────────
USED=$(surfaces_in_use)
PIDF=/tmp/tessera-verify-$ARENA.windows   # (the test windows' pids: closed by pid, never by name)
capture() { # look pass(qml|rust)
    local look=$1 pass=$2
    printf '%s\n' "$STATES" | while IFS='|' read -r s name cmd wait flag; do
        want "$s" || continue
        [[ $flag == *s* && ( $pass == qml || $STANDALONE == 0 ) ]] && continue
        quiet
        if [[ $flag == *w* && ! -s $PIDF ]]; then   # (windows for Overview & co., from here on)
            inbox "foot >/dev/null 2>&1 & echo \$! >> $PIDF; foot >/dev/null 2>&1 & echo \$! >> $PIDF" </dev/null >/dev/null 2>&1
            sleep 2
        fi
        if [[ $pass == rust && $STANDALONE == 1 ]]; then
            # (standalone: what each command answered, and its exit code)
            out=$(inbox "$cmd" </dev/null 2>&1); rc=$?
            printf '%s|%s|%s|%s|%s\n' "$s" "$name" "$rc" "$cmd" "$(printf '%s' "$out" | tr '\n' ' ' | cut -c1-300)" >> "$OUT/logs/$look-standalone-ipc.txt"
        else
            inbox "$cmd" </dev/null >/dev/null 2>&1
        fi
        sleep "$wait"
        dev shot "$OUT/shots/$look-$s-$name-$pass.png" </dev/null >/dev/null 2>&1 || log "  no shot: $look $s $name ($pass)"
        # (Settings and the App Store are real app windows: they'd stay open into the next states)
        case "$s|$name" in
            # (by class for Rust's windows, by title for the QML's: theirs is Quickshell's class)
            "settings|appearance")
                dev hyprctl dispatch 'hl.dsp.window.close({ window = "class:tessera-settings" })' </dev/null >/dev/null 2>&1
                dev hyprctl dispatch 'hl.dsp.window.close({ window = "title:^Tessera Settings$" })' </dev/null >/dev/null 2>&1; sleep 1 ;;
            "appstore|discover")
                dev hyprctl dispatch 'hl.dsp.window.close({ window = "class:tessera-appstore" })' </dev/null >/dev/null 2>&1
                dev hyprctl dispatch 'hl.dsp.window.close({ window = "title:^Software$" })' </dev/null >/dev/null 2>&1; sleep 1 ;;
        esac
    done
    quiet
    inbox "ipc island holdAlerts false" </dev/null >/dev/null 2>&1   # (the look's captures are done: alerts show again)
    [[ -s $PIDF ]] && inbox "kill \$(cat $PIDF) 2>/dev/null; rm -f $PIDF" </dev/null >/dev/null 2>&1
    rm -f "$PIDF"
}
# Standalone: the shell's processes in the arena (pid, memory, CPU, age)
probe() { # look label
    inbox "echo \"ui=\$(pgrep -f 'tessera-ui(.real)? --standalone' | head -1) qs=\$(pgrep -x tessera-shell | head -1)\$(pgrep -x qs | head -1)\$(pgrep -x quickshell | head -1) sock=\$(test -S \$XDG_RUNTIME_DIR/tessera/ipc.sock && echo yes || echo no)\"; for p in \$(pgrep -f 'tessera-ui(.real)? --standalone'; pgrep -x tessera-shell; pgrep -x qs; pgrep -x quickshell); do ps -o pid=,rss=,pcpu=,etimes=,args= -p \$p; done" </dev/null 2>/dev/null | sed "s/^/$2 /" >> "$OUT/logs/$1-standalone-probe.txt"
}
for look in ${LOOKS//,/ }; do
    ((BUILD_FAILED)) && break
    log "[$look] setting up the arena (fixed wallpaper, 1920x1080)"
    setup "$look" "$WALLS/wall-tiles.jpg" 1
    # (behind a wrapper that keeps tessera-ui's own messages: logs/<look>-tessera-ui.log)
    install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui.real"
    printf '#!/bin/sh\nexec env TESSERA_DEBUG=1 RUST_BACKTRACE=1 "$(dirname "$0")/tessera-ui.real" "$@" 2>>"%s"\n' "$OUT/logs/$look-tessera-ui.log" > "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    chmod 755 "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    log "[$look] QML pass"
    set_rust ""; sleep 3
    ((STANDALONE)) && probe "$look" qml-settled
    capture "$look" qml
    log "[$look] reloading the shell with Rust on (nothing of the QML pass carries over)"
    # (Rust first, then the reload: the QML twins never start, so none of them
    # holds a bus name a Rust surface needs, e.g. org.freedesktop.Notifications)
    if ((STANDALONE)); then
        log "[$look] reloading as the standalone shell (tessera-ui --standalone, no Quickshell)"
        set_rust "" 1; sleep 2
        t0=$(date +%s%N)
        dev reload </dev/null >/dev/null 2>&1
        # (how long until it answers IPC: its socket appears)
        up=never
        for _ in $(seq 1 60); do
            if inbox 'test -S $XDG_RUNTIME_DIR/tessera/ipc.sock' </dev/null >/dev/null 2>&1; then up=$(( ($(date +%s%N) - t0) / 1000000 ))ms; break; fi
            sleep 0.5
        done
        echo "ipc-up $up" >> "$OUT/logs/$look-standalone-probe.txt"
        sleep 10
        probe "$look" start
        log "[$look] standalone pass (IPC up after $up)"
        capture "$look" rust
        probe "$look" end
        set_rust "" 0
    else
        set_rust "$USED"; sleep 2
        dev reload </dev/null >/dev/null 2>&1; sleep 20
        log "[$look] Rust pass ($USED)"
        capture "$look" rust
        set_rust ""
    fi
    dev logs 300 > "$OUT/logs/$look-logs.txt" 2>&1
done

# ── 3. Compare ──────────────────────────────────────────────────────────────
log "comparing"
python3 -I - "$OUT" <<'PY'
import json, subprocess, sys, pathlib
out = pathlib.Path(sys.argv[1]); shots = out / "shots"; rows = []
for q in sorted(shots.glob("*-qml.png")):
    r = q.with_name(q.name.replace("-qml.png", "-rust.png"))
    base = q.name[:-len("-qml.png")]
    look, surface, state = base.split("-", 2)
    row = {"look": look, "surface": surface, "state": state, "qml": str(q.relative_to(out))}
    if not r.exists():
        row.update(diff=None, verdict="no rust shot"); rows.append(row); continue
    row["rust"] = str(r.relative_to(out))
    p = subprocess.run(["magick", "compare", "-metric", "RMSE", str(q), str(r), str(shots / f"{base}-diff.png")], capture_output=True, text=True)
    try:
        d = float(p.stderr.split("(")[1].split(")")[0]) * 100
    except Exception:
        d = None
    subprocess.run(["magick", str(q), str(r), "+append", "-resize", "2400x", str(shots / f"{base}-pair.jpg")], capture_output=True)
    row.update(diff=None if d is None else round(d, 2), pair=f"shots/{base}-pair.jpg", diffimg=f"shots/{base}-diff.png",
               verdict="?" if d is None else "same" if d < 2 else "close" if d < 6 else "different")
    rows.append(row)
# (standalone-only states: a Rust picture with no QML one to compare)
for r in sorted(shots.glob("*-rust.png")):
    base = r.name[:-len("-rust.png")]
    if not (shots / f"{base}-qml.png").exists():
        look, surface, state = base.split("-", 2)
        rows.append({"look": look, "surface": surface, "state": state, "rust": str(r.relative_to(out)), "diff": None, "pair": str(r.relative_to(out)), "verdict": "new (no QML)"})
json.dump(rows, open(out / "visual.json", "w"), indent=1)
PY

# ── 4. Tests ────────────────────────────────────────────────────────────────
if ((TESTS)); then
    log "crate tests (one crate at a time, low priority)"
    : > "$OUT/logs/cargo-tests.log"
    for d in "$ROOT/native/tessera-ui/core" "$ROOT"/native/tessera-ui/host/*/ "$ROOT"/native/tessera-ui/surfaces/*/ "$ROOT/native/tessera-ui"; do
        crate=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$d/Cargo.toml" | head -1)
        [[ -n $crate ]] || continue
        res=$(cd "$ROOT/native/tessera-ui" && run_cargo cargo test -p "$crate" 2>&1 | tee -a "$OUT/logs/cargo-tests.log" | grep -E "^test result|error(\[|:)" | awk '/test result/{p+=$4; f+=$6} /error/{e=1} END {printf "%d %d %d", p, f, e}')
        echo "$crate $res" >> "$OUT/logs/cargo-summary.txt"
        log "  $crate: $res (passed failed compile-error)"
    done
    log "python suite"
    (cd "$ROOT" && env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DISPLAY nice -n 19 python3 -m pytest -q tests --deselect tests/test_rust_ui.py::CrateTests) > "$OUT/logs/pytest.log" 2>&1
fi

# ── 5. Report ───────────────────────────────────────────────────────────────
python3 -I - "$OUT" <<'PY'
import json, sys, pathlib
out = pathlib.Path(sys.argv[1]); rows = json.load(open(out / "visual.json"))
L = ["# tessera-ui verification", "", "```", (out / "env.txt").read_text().strip(), "```", ""]
cnt = {}
for r in rows: cnt[r["verdict"]] = cnt.get(r["verdict"], 0) + 1
L += ["## Visual (QML vs Rust, same state)", "", "Difference is the RMSE of the whole screen in % (clock and battery change between passes: under 2% reads as the same).", "",
      "Summary: " + ", ".join(f"{k}: {v}" for k, v in sorted(cnt.items())), "",
      "| look | surface | state | diff % | verdict | pair |", "|---|---|---|---:|---|---|"]
for r in sorted(rows, key=lambda r: (r["verdict"] == "same", r["look"], r["surface"], r["state"])):
    L.append(f"| {r['look']} | {r['surface']} | {r['state']} | {'' if r.get('diff') is None else r['diff']} | {r['verdict']} | {r.get('pair', '')} |")
s = out / "logs/cargo-summary.txt"
if s.exists():
    L += ["", "## Crate tests", "", "| crate | passed | failed | compile error |", "|---|---:|---:|---|"]
    for line in s.read_text().split("\n"):
        if line.strip():
            c, p, f, e = (line.split() + ["0", "0", "0"])[:4]
            L.append(f"| {c} | {p} | {f} | {'yes' if e == '1' else ''} |")
p = out / "logs/pytest.log"
if p.exists():
    t = p.read_text().strip().split("\n")
    L += ["", "## Python suite", "", "```", *[x for x in t if x.startswith("FAILED")][:30], t[-1] if t else "", "```"]
b = out / "logs/build.log"
if b.exists() and "error" in b.read_text():
    # Every error with where it is, grouped by crate (the fixes go crate by crate)
    import re, collections
    lines = b.read_text().split("\n"); errs = []
    for i, x in enumerate(lines):
        if x.startswith("error") and not x.startswith("error: could not compile") and not x.startswith("error: aborting") and "previous error" not in x:
            where = next((y.strip()[4:] for y in lines[i + 1:i + 6] if y.strip().startswith("--> ")), "")
            errs.append((where, x))
    def crate(w):
        m = re.match(r"(host|surfaces)/([^/]+)/", w)
        return f"{m.group(1)}/{m.group(2)}" if m else ("core" if w.startswith("core/") else (w.split("/")[0] or "?"))
    by = collections.defaultdict(list)
    for w, e in errs:
        by[crate(w)].append((w, e))
    warn = sum(1 for x in lines if x.startswith("warning"))
    L += ["", "## Build errors", "", f"{len(errs)} errors, {warn} warnings. By crate: " + ", ".join(f"{c} {len(v)}" for c, v in sorted(by.items(), key=lambda kv: -len(kv[1]))), ""]
    codes = collections.Counter(re.findall(r"error\[(E\d+)\]", "\n".join(e for _, e in errs)))
    if codes:
        L += ["Most common: " + ", ".join(f"{c} ×{n}" for c, n in codes.most_common(8)), ""]
    for c, v in sorted(by.items(), key=lambda kv: -len(kv[1])):
        L += [f"### {c}", "", "```", *[f"{w}  {e}" for w, e in v[:150]], *(["… and more (logs/build.log)"] if len(v) > 150 else []), "```", ""]
# Standalone (wave 4): did it run alone, and what went unanswered
import re, collections
for look_log in sorted(out.glob("logs/*-standalone-probe.txt")):
    look = look_log.name.split("-")[0]
    probe = look_log.read_text().split("\n")
    L += ["", f"## Standalone: {look}", ""]
    up = next((x.split()[1] for x in probe if x.startswith("ipc-up")), "?")
    def first(label, key):
        for x in probe:
            m = re.search(rf"(?:^| ){key}=(\S*)", x) if x.startswith(label + " ") else None
            if m:
                return m.group(1)
        return ""
    ui_start, ui_end = first("start", "ui"), first("end", "ui")
    qs_start = first("start", "qs")
    L += [f"- IPC answering after: **{up}** (from the reload)",
          f"- tessera-ui --standalone running at start: {'yes, pid ' + ui_start if ui_start else '**NO**'}; at the end: {'yes, pid ' + ui_end if ui_end else '**NO (it died)**'}" + (" **(restarted: it crashed in between)**" if ui_start and ui_end and ui_start != ui_end else ""),
          f"- Quickshell running beside it: {'**yes (pid ' + qs_start + '): not standalone**' if qs_start else 'no (as it should be)'}",
          f"- IPC socket: {first('start', 'sock') or '?'}"]
    def mem(label, name):
        for x in probe:
            if x.startswith(label + " ") and name in x and "=" not in x.split()[1]:
                f = x.split()
                try:
                    return f"{int(f[2]) / 1024:.0f} MB, CPU {f[3]}%"
                except Exception:
                    pass
        return "?"
    L += [f"- Memory (RSS) and CPU, standalone: start {mem('start', 'tessera-ui')}, end {mem('end', 'tessera-ui')}; QML shell in the reference pass: {next((m for m in (mem('qml-settled', n) for n in ('tessera-shell', 'quickshell', 'qs')) if m != '?'), '?')}", ""]
    tl = out / f"logs/{look}-tessera-ui.log"
    t = tl.read_text(errors="replace").split("\n") if tl.exists() else []
    na = collections.Counter(m.group(1) for x in t for m in [re.search(r"host: nothing answers (\S+)", x)] if m)
    ni = collections.Counter(m.group(1) + " " + m.group(2) for x in t for m in [re.search(r"host: no ipc target (\S+) \(([^)]*)\)", x)] if m)
    if na:
        L += ["Calls nothing answered (call: times):", "", "```", *[f"{k}: {v}" for k, v in na.most_common()], "```", ""]
    else:
        L += ["Calls nothing answered: none", ""]
    if ni:
        L += ["IPC targets nothing answered (target function: times):", "", "```", *[f"{k}: {v}" for k, v in ni.most_common()], "```", ""]
    pan = [i for i, x in enumerate(t) if "panicked at" in x]
    if pan:
        L += [f"**Panics: {len(pan)}**", "", "```"]
        for i in pan[:5]:
            L += t[i:i + 12] + ["…"]
        L += ["```", ""]
    other = collections.Counter(re.sub(r"\d+", "N", x)[:160] for x in t if x.startswith("tessera-ui:") and any(w in x.lower() for w in ("error", "fail", "can't", "cannot", "missing", "unknown")))
    if other:
        L += ["Other problems tessera-ui reported (numbers folded):", "", "```", *[f"{v}× {k}" for k, v in other.most_common(40)], "```", ""]
    ip = out / f"logs/{look}-standalone-ipc.txt"
    if ip.exists():
        bad = [r.split("|", 4) for r in ip.read_text().split("\n") if r.count("|") >= 4]
        bad = [r for r in bad if r[2] != "0" or re.search(r"error|not |unknown|no such", r[4], re.I)]
        L += [f"IPC commands that failed or complained: {len(bad)}", ""]
        if bad:
            L += ["| surface | state | exit | command | said |", "|---|---|---:|---|---|"]
            L += [f"| {r[0]} | {r[1]} | {r[2]} | `{r[3][:80]}` | {r[4][:120].replace('|', '/')} |" for r in bad]
            L += [""]
if list(out.glob("logs/*-standalone-probe.txt")):
    L += ["", "## Manual checks", "", "What the porting agents say to check by hand (on screen, in the sandbox): native/tessera-ui/host/*/PORT-NOTES.md, \"how to check\" per file."]
L += ["", "## Files", "", f"- shots/: every QML and Rust picture, `*-pair.jpg` side by side, `*-diff.png` differences",
      "- logs/: build, cargo tests, pytest, the arena's shell and compositor logs per look, tessera-ui's own messages per look (*-tessera-ui.log)",
      "- logs/*-standalone-*.txt (standalone): the processes (probe) and every IPC command's exit code and answer (ipc)", "- visual.json: the table above as data"]
(out / "report.md").write_text("\n".join(L) + "\n")
json.dump({"visual": rows}, open(out / "report.json", "w"), indent=1)
print(out / "report.md")
PY
log "done: $OUT/report.md"
