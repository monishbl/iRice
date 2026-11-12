#!/usr/bin/env python3
"""Step files for the deterministic parity runner (one action per line)."""
import json, os, sys

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)

def cfg(path, value):
    """setup line: one config.json key set in the arena (one line of Python)"""
    keys = path.split(".")
    chain = "c" + "".join(f".setdefault({k!r},{{}})" for k in keys[:-1])
    code = ("import json,os;p=os.path.expanduser('~/.config/tessera/config.json');c=json.load(open(p));"
            f"{chain}[{keys[-1]!r}]=json.loads({json.dumps(value)!r});json.dump(c,open(p,'w'),indent=4)")
    return "setup python3 -c " + json.dumps(code)

USER_WIDGETS = ["clock:m:0:0", "stack:m:0:1:pz3mh8", "batteries:m:0:2", "shortcuts:m:2:0:5a2cav"]

def add_widget(kind, size, wid=""):
    """the user's four plus one more at cell (2,1) (free in their layout)"""
    e = f"{kind}:{size}:2:1" + (f":{wid}" if wid else "")
    return cfg("desktop.widgets", USER_WIDGETS + [e])

RESTORE = None  # (set below, after cfg)

CLOSE_FOOT = "setup hyprctl dispatch 'hl.dsp.window.close({ window = \"class:foot\" })'"
RESTORE = cfg("desktop.widgets", USER_WIDGETS)
HEAD = ["# Arena IRICE-TEST 1920x1080 scale 1, the user's config (expressive motion, Colloid, their 4 widgets).",
        "# Each file runs top to bottom from a fresh desktop. `static NAME x y w h`: that region must not change",
        "# from there until the next `shot`, `rec` or `look` (the no-reload blocks end with their last close).",
        "# Right-clicks: `ptr ... down up` lines marked (right button) need the runner's right button."]

# the user's widgets (grid origin 32,54; cell 164, gap 16)
W = {"clock": (32, 54, 344, 164), "stack": (32, 234, 344, 164), "batteries": (32, 414, 344, 164), "shortcuts": (392, 54, 344, 164)}

HIDE_FOOT = "setup hyprctl dispatch 'hl.dsp.window.move({ workspace = \"special:parity\", follow = false, window = \"class:foot\" })'"
OPEN_HIDDEN = """setup sh -c "foot & sleep 0.8; hyprctl dispatch \\"hl.dsp.window.move({ workspace = 'special:parity', follow = false, window = 'class:foot' })\\"" """.strip()
FOCUS_FOOT ="setup hyprctl dispatch 'hl.dsp.focus({ window = \"class:foot\" })'"

def no_reload(tag):
    """the no-reload check: every widget still while windows open, focus and
    close (each window goes to a hidden special workspace right after it
    opens, so it never covers the widgets the check watches)"""
    s = ["note no reload: widgets must not fade or grow in again while windows open, focus and close (only the clock's minute may change)",
         "ptr move 1500 600",
         "shot " + tag + "-before",
         # opening: a window covers the widgets for a moment; it's sent away
         # and the widgets shot at once (a reload would show them small/faint)
         OPEN_HIDDEN,
         "shot " + tag + "-after-open-1",
         OPEN_HIDDEN,
         "shot " + tag + "-after-open-2",
         "note the after-open shots equal the before shot over the widgets (no widget at 0.9 scale or faded)"]
    for k, (x, y, w, h) in W.items():
        s.append(f"static {tag}-{k} {x} {y} {w} {h}")
    s += ["ptr move 1300 700 click", "wait 1",
          CLOSE_FOOT, "wait 1.5", CLOSE_FOOT, "wait 1.5"]
    return s

files = {}
def F(stem, looks, body):
    lines = list(HEAD)
    for lk in looks:
        b = body(lk)
        lines += ["", f"look {lk}"] + b + ([RESTORE, "wait 1"] if any("desktop.widgets" in x for x in b) else [])
    files[stem] = lines

# ── desktop ────────────────────────────────────────────────────────────────
F("DesktopWidgets", ["glass", "material"], lambda lk: no_reload("dw") + [
    "ptr move 600 600 glide 200 300 300 sleep 200",
    "note (right button, manual) right-click the stack at 200,300:",
    "note right-click on the stack: sizes with ✓, kinds, Smart Rotate, Widget Options…, Edit Widgets…, Remove Widget (red); opens at the pointer, downward",
    "key -k Escape", "wait 0.5",
    "note right-click the bare desktop at 1200 400: Edit Widgets…, Change Wallpaper…, ✓ Use Stacks, Show Files on Desktop, Desktop Settings…",
    "ptr move 1200 400", "key -k Escape", "wait 0.3",
    "rec dw-edit 1.0 ipc panels widgets true",
    "note editing: faint cells fade in ≈220 ms, each widget's − badge springs in with a small overshoot",
    "ipc panels widgets false", "wait 0.5",
    "rec dw-drag 3 ptr move 600 130 down glide 600 330 400 glide 1000 330 400 up",
    "note the shortcuts widget follows after 6 px, grows 1.04, ghost springs cell to cell; on release springs into the cell (~450 ms) and config desktop.widgets has it moved",
    "ptr move 1000 300 down glide 600 130 400 up", "wait 1",
    "shot dw-after",
])
F("WidgetFrame", ["glass", "material"], lambda lk: no_reload("wf") + [
    "ipc lock lock", "wait 2",
    "rec wf-unlock 2.0 ipc lock unlock",
    "note widgets start ~1.2x and see-through, zoom to their cells and fade in, outer ones later (batteries after the clock); Glass: the glass under each zooms with it; ≈900 ms Glass / 800 ms Material, ease-out",
    "rec wf-badges 1.0 ipc panels widgets true",
    "note each − badge grows from 0 with a small overshoot (~400 ms)",
    "ipc panels widgets false", "wait 0.5",
    "rec wf-lift 1.0 ptr move 200 130 down glide 240 130 200 sleep 400 up",
    "note dragged widget grows to 1.04 in ≈150 ms, springs back on release",
])
F("WidgetDefs", ["glass", "material"], lambda lk: [
    "shot wd-idle",
    "note the user's 4 widgets at clock (32,54) stack (32,234) batteries (32,414) shortcuts (392,54); Material cards in their containers",
])
F("WidgetHeader", ["glass", "material"], lambda lk: [
    add_widget("reminders", "m", "pt1"), "wait 1.5", "shot wh-reminders",
    "note a 26 px disc with the glyph, title 8 px after it; Material primary disc",
])
F("ClockWidget", ["glass", "material"], lambda lk: [
    "setup sh -c 'sleep $((60 - $(date +%S) - 2))'",
    "static ck-others-stack 32 234 344 164", "static ck-others-batteries 32 414 344 164", "static ck-others-shortcuts 392 54 344 164",
    "rec ck-minute 4 ptr move 1500 600",
    "note across the minute only the clock's digits change; the colon and unchanged digits don't move sideways (digit slots as wide as a 0)",
    "note Material: the time is light (400) as QML, not bold",
] + no_reload("ck"))
F("AnalogClock", ["glass", "material"], lambda lk: [
    "ptr move 200 130 sleep 200", "note resize the clock: right-click it → Small", cfg("desktop.widgets", ["clock:s:0:0"] + USER_WIDGETS[1:]), "wait 1",
    "setup sh -c 'sleep $((60 - $(date +%S) - 1))'",
    "rec ac-minute 3 ptr move 1500 600",
    "note the minute hand turns 6° clockwise with a small overshoot over ≈400 ms (not a jump)",
    RESTORE, "wait 1",
])
F("GaugeRing", ["glass", "material"], lambda lk: ["shot gr-batteries", "note batteries widget (32..376 x 414..578): arcs from 12 o'clock, glyph/picture centred"] + no_reload("gr"))
F("SystemWidget", ["glass", "material"], lambda lk: [
    add_widget("system", "m", "pt1"), "wait 2",
    "rec sy-graphs 5 ptr move 1500 600",
    "note graphs move each second; labels never blink; the widget never fades/grows in again",
])
F("NotesWidget", ["glass", "material"], lambda lk: [
    add_widget("notes", "s", "pt1"), "wait 1.5",
    "note click inside the new Note widget (find it in the shot)", "shot nw-added",
    "key hello world", "wait 1",
    "note text appears as typed; state desktopNotes has it after ~0.5 s",
    "key -k Escape", "wait 0.5",
] + no_reload("nw"))
F("ShortcutsWidget", ["glass", "material"], lambda lk: [
    "rec sc-click 2 ptr move 604 175 click",
    "note App Drawer tile: the launchpad opens (log: tessera-ui: desktop shortcut \"App Drawer\")",
    "key -k Escape", "wait 0.8",
    "rec sc-drag-tile 3 ptr move 600 130 down glide 600 330 400 glide 1400 330 400 up",
    "note the whole Shortcuts widget follows from 6 px (1.04), ghost shows; on release it springs into the cell; no action runs; config desktop.widgets changed",
    "ptr move 1400 330 down glide 600 130 400 up", "wait 1",
    "note right-click a tile (600,130): the widget's menu opens", "key -k Escape",
    "rec sc-hover 2 ptr move 440 100 glide 520 100 300 glide 600 100 300 glide 680 100 300",
    "note hovered tile brightens (Glass tint 0.18→0.26, 150 ms; Material 8 % layer)",
    "rec sc-press 1.5 ptr move 440 100 down sleep 500 up",
    "note Glass: 0.95 in 100 ms; Material: swells ≈4 %, corners 20→12, bounces past 1 on release",
] + no_reload("sc"))
F("WorldClockWidget", ["glass", "material"], lambda lk: [
    add_widget("worldClock", "m", "pt1"), "wait 2",
    "setup sh -c 'sleep $((60 - $(date +%S) - 1))'",
    "rec wc-minute 3 ptr move 1500 600",
    "note all four minute hands turn ≈400 ms with overshoot; no face blinks",
])
F("NowPlayingWidget", ["material", "glass"], lambda lk: [
    "setup sh -c 'ffmpeg -loglevel quiet -y -f lavfi -i sine=d=120 -i /usr/share/pixmaps/archlinux-logo.png -map 0 -map 1 -disposition:v attached_pic /tmp/np.mp3 2>/dev/null; mpv --no-video --really-quiet /tmp/np.mp3 &'",
    add_widget("nowPlaying", "m", "pt1"), "wait 2", "shot np-playing",
    "note Material: the cover is a 9-scallop cookie while playing; Glass: 12 px rounded square",
    "note click the middle (play/pause) button of the new widget (find it in the shot)",
    "rec np-pause 1.5 ptr move 1500 600",
    "note paused: the cover goes back to the rounded square (≈260 ms); the button rounds",
])
F("ScreenTimeWidget", ["glass"], lambda lk: [
    add_widget("screenTime", "m", "pt1"), "wait 1",
    "setup foot &", "wait 1",
    "ptr move 900 500 sleep 30000 glide 950 520 400 sleep 30000 glide 900 500 400 sleep 30000 glide 950 520 400 sleep 40000",
    CLOSE_FOOT, "wait 5", "shot st-after",
    "note total ≥ 2m, today's bar filled, foot in the app rows with its Colloid icon; state.json screenTime.<today> has foot",
])
F("PhotosWidget", ["glass", "material"], lambda lk: [
    "setup sh -c 'mkdir -p ~/Pictures; for c in red green blue; do ffmpeg -loglevel quiet -y -f lavfi -i color=$c:s=800x600 -frames:v 1 ~/Pictures/$c.jpg; done'",
    add_widget("photos", "m", "pt1"), "wait 2", "shot ph-added",
    "note click the Photos widget (find it): the next picture cross-fades ≈800 ms",
    "rec ph-next 1.5 ptr move 1500 600",
    "note Material, small size: the picture is a four-leaf clover on the surface container colour",
])
F("BatteryHistoryWidget", ["glass", "material"], lambda lk: [add_widget("batteryHistory", "m", "pt1"), "wait 2", "shot bh"] + no_reload("bh"))
F("BatteriesWidget", ["glass", "material"], lambda lk: ["shot bt-idle", "note rings as the QML pass; Fast Pair earbuds (only with real earbuds): Left / Right / Case rings"] + no_reload("bt"))
F("StocksWidget", ["glass"], lambda lk: [add_widget("stocks", "m", "pt1"), "wait 5", "shot sk"] + no_reload("sk"))
F("RemindersWidget", ["glass", "material"], lambda lk: [
    "note add two to-dos first (Today or Reminders' field): Buy milk, Call Ada", add_widget("reminders", "m", "pt1"), "wait 2", "shot rm-added",
    "note click the first row's tick (find it): that row leaves; the second moves up without the widget redrawing whole",
    "rec rm-tick 1.5 ptr move 1500 600",
])
F("StackWidget", ["glass", "material"], lambda lk: [
    "rec sw-notch 1.2 ptr move 200 316 scroll 0 15",
    "note exactly one flip: the next widget slides up 24 px and fades in (300/200 ms); the current dot grows to 12 px",
    "ptr scroll 0 -15", "wait 0.8",
    "rec sw-touchpad 1.2 ptr move 200 316 scroll 0 3 sleep 10 scroll 0 3 sleep 10 scroll 0 3 sleep 10 scroll 0 3 sleep 10 scroll 0 3",
    "note five small steps (15 px in all): one flip, not five",
    "rec sw-dot 1 ptr glide 366 300 300 click",
    "note the dot turns label/on-surface-variant on hover; click jumps with the slide",
] + no_reload("sw"))
F("WeatherWidget", ["glass", "material"], lambda lk: [
    add_widget("weather", "m", "pt1"), "wait 3", "shot we",
    "note medium: hourly row (5 slots) as QML (empty in Rust only if core's Weather has none: core issue)",
] + no_reload("we"))
F("TodayWidget", ["glass", "material"], lambda lk: [
    add_widget("today", "m", "pt1"), "wait 2", "shot td-added",
    "note click the greeting: Today opens; Escape",
] + no_reload("td"))
F("CalendarWidget", ["glass", "material"], lambda lk: [
    "note scroll the stack to Calendar (one notch at a time, shot after each) or add it:",
    add_widget("calendar", "m", "pt1"), "wait 2", "shot ca-added",
    "note glide over the month's days: each hovered day gets a faint round layer; click one: picked (secondary container), its events on the left; click again: back to today",
])
F("DesktopIcons", ["glass", "material"], lambda lk: [
    "setup sh -c 'mkdir -p ~/Desktop/Work; touch ~/Desktop/notes.txt; sleep 0.3; touch ~/Desktop/report.pdf ~/Desktop/song.mp3 ~/Desktop/archive.zip; ffmpeg -loglevel quiet -y -f lavfi -i color=red:s=400x300 -frames:v 1 ~/Desktop/photo.jpg'",
    "wait 4", "shot di-stacks",
    "note Use Stacks on: stacks top right; each fan uses Colloid icons; Documents' front icon is the PDF (red), as QML",
    "static di-stacks 1500 54 420 600",
    "setup touch ~/Desktop/new.txt", "wait 4",
    "note only Documents' fan may change; other stacks don't blink",
    cfg("desktop.useStacks", False), "wait 4", "shot di-files",
    "setup touch ~/Desktop/a-very-long-file-name-for-testing-middle-elide.txt", "wait 4", "shot di-long",
    "note long name: two lines at most, cut in the middle (a-very-…elide.txt)",
    cfg("desktop.useStacks", True), "wait 4",
] + no_reload("di"))
F("DesktopFeed", ["glass"], lambda lk: ["shot df", "note covered by BatteriesWidget and ScreenTimeWidget; the feed log line has batteries and screenTime filled"])
F("WidgetGallery", ["glass", "material"], lambda lk: [
    "rec wg-open 1.5 ipc panels widgets true",
    "note the gallery rises and settles on a spring",
    "key -k Down -k Down -k Down -k Up", "wait 0.5", "shot wg-picked",
    "note hover a size preview: accent border, grows 1.04 with a spring; click: the widget is added (grows in 0.9→1); others don't blink",
    "key -k Escape", "wait 1",
])
F("WidgetOptions", ["glass", "material"], lambda lk: [
    "note open Widget Options from the stack's right-click menu (right button: manual)", "wait 1", "shot wo",
    "note toggle a kind, Done: the stack gains/loses that page; other widgets don't blink",
    "key -k Escape",
])
F("AnalogClockMaterialShapes", [], lambda lk: [])
del files["AnalogClockMaterialShapes"]

# ── dock ───────────────────────────────────────────────────────────────────
def dock_y(lk):
    return 1038 if lk == "glass" else 1032

F("Dock", ["glass", "material"], lambda lk: [
    "shot dk-idle",
    "static dk-dock 820 940 280 140",
    "ipc panels open controlCenter IRICE-TEST", "wait 1", "ipc panels close", "wait 1",
    "ipc panels open launchpad IRICE-TEST", "wait 1", "ipc panels close", "wait 1",
    "note the dock never vanishes or redraws from blank when other surfaces close",
    cfg("dock.enabled", False), "wait 1.5", "shot dk-off", cfg("dock.enabled", True), "wait 1.5", "shot dk-on",
])
F("DockGlass", ["glass"], lambda lk: [
    cfg("dock.autoHide", "always"), "ptr move 1500 500", "wait 1.5",
    "rec dg-reveal 1.5 ptr glide 960 1079 300 sleep 800",
    "note reveal 350 ms with a few px overshoot (OutBack 1.2)",
    "rec dg-hide 1.5 ptr glide 960 700 300 sleep 800",
    "note hide 250 ms InCubic, 300 ms after leaving",
    cfg("dock.autoHide", "off"), "wait 1",
    "rec dg-band 1.5 ptr move 960 950 glide 960 1040 600",
    "note magnification starts only once y ≥ 961",
])
F("DockItem", ["glass"], lambda lk: [
    "rec di-magnify 2.5 ptr move 760 1000 glide 760 1040 200 glide 1160 1040 1600",
    "note each icon grows to 1.6x as the pointer passes, cosine falloff over 2.5 icons; follows the pointer with no lag",
    "ptr move 970 1038 sleep 300", "shot di-label", "note \"Downloads\" label on a frosted pill above",
    "rec di-squish 1 ptr move 970 1038 down sleep 400 up",
    "note press: 0.9 on a spring (~320 ms); release springs back",
    "shot di-icons", "note Apps: Colloid coloured grid on white tile; Downloads: blue Colloid folder, no dark tile; Trash: grey can",
    "setup foot &", "wait 1.5", "shot di-running", "note running dot under foot", CLOSE_FOOT, "wait 1",
])
F("DockLogic", ["glass", "material"], lambda lk: [
    "static dl-right 1000 990 200 90",
    "setup foot &", "wait 2", "setup foot &", "wait 2",
    CLOSE_FOOT, "wait 1.5", CLOSE_FOOT, "wait 1.5",
    "note on open only foot's icon is inserted; on close nothing hops and Downloads/Trash don't reload (they may slide as a block)",
    cfg("dock.autoHide", "always"), "ptr move 1500 500", "wait 1.5",
    "rec dl-autohide 3 ptr glide 960 1079 300 sleep 1200 glide 960 700 300 sleep 800",
    cfg("dock.autoHide", "off"), "wait 1",
    f"ptr move 1027 {dock_y(lk)}",
    "note (right button, manual) right-click Trash to open its menu, then:", "ipc panels open controlCenter IRICE-TEST", "wait 0.5", "shot dl-menu-gone",
    "note opening a panel closes the dock menu at once", "ipc panels close",
])
F("DockMenu", ["glass", "material"], lambda lk: [
    "setup sh -c 'mkdir -p ~/.local/share/Trash/files && echo x > ~/.local/share/Trash/files/a.txt'", "wait 2.5",
    "note right-click Trash (the runner's right button), then:",
    f"ptr move 1027 {dock_y(lk)}",
    "note menu centred over Trash, 8 px above the pill: Open Trash / — / Empty Trash… (red, enabled); spring ≈350 ms",
    "key -k Down -k Down -k Up", "wait 0.3", "shot dm-keys",
    "key -k Escape", "wait 0.5",
    "note click Empty Trash… once: \"Delete 1 item for good\"; again: emptied, trash sound, icon empty within 2 s",
])
F("DockSlot", ["glass", "material"], lambda lk: [
    f"rec ds-apps 1.5 ptr move 894 {dock_y(lk)} click", "note launchpad opens", "key -k Escape", "wait 0.8",
    f"rec ds-downloads 1.5 ptr move 970 {dock_y(lk)} click", "note the Downloads stack opens", "key -k Escape", "wait 0.8",
    cfg("dock.pinned", ["foot"]), "wait 1.5",
    f"rec ds-launch 2 ptr move 951 {dock_y(lk)} click", "note foot hops twice and opens",
    f"rec ds-expose 1.5 ptr move 951 {dock_y(lk)} click", "note foot in front: App Exposé opens", "key -k Escape",
    CLOSE_FOOT, "wait 1", cfg("dock.pinned", []),
])
F("DownloadsFiles", ["glass", "material"], lambda lk: [
    "setup sh -c 'cd ~/Downloads && touch report.pdf photos.zip shot.png notes.txt clip.mp4 .x && mkdir -p dir'",
    f"ptr move 970 {dock_y(lk)} click", "wait 1", "shot df-open",
    "note 5 files newest first; no .x, no dir",
    "setup sh -c 'echo y > ~/Downloads/new.txt'", "wait 2.5", "shot df-live",
    "note new.txt first; others moved, not flashed", "key -k Escape",
])
F("DownloadsStack", ["glass"], lambda lk: [
    "setup sh -c 'cd ~/Downloads && touch report.pdf photos.zip shot.png notes.txt clip.mp4'",
    "rec dsk-open 1.2 ptr move 970 1038 click",
    "note card fades in, 0.92→1 from its bottom centre on a spring, ≈375 ms with a small overshoot; Colloid mimetype icons",
    "key -k Escape", "wait 0.5",
])
F("MaterialDock", ["material"], lambda lk: [
    "shot md-idle",
    "rec md-tooltip 2 ptr move 970 1100 glide 970 1032 200 sleep 1200 glide 1027 1032 200 sleep 300",
    "note Downloads tooltip after ≈500 ms (150 ms fade, 4 px rise); Trash's at once",
])
F("MaterialDockItem", ["material"], lambda lk: [
    "rec mdi-press 1.5 ptr move 970 1032 sleep 300 down sleep 400 up",
    "note 52 px state layer 8 %→10 %; glyph swells ≈6 % and bounces on release",
    "shot mdi-icons", "note Downloads blue Colloid folder, Trash grey can",
    cfg("dock.pinned", ["foot"]), "wait 1.5",
    "rec mdi-bounce 2 ptr move 951 1032 click", "note hop 14.4 px twice: up 220 ms emphasized decelerate, down 260 ms OutBounce",
    CLOSE_FOOT, cfg("dock.pinned", []),
])
F("MaterialDownloadsStack", ["material"], lambda lk: [
    "setup sh -c 'cd ~/Downloads && touch report.pdf photos.zip shot.png notes.txt clip.mp4'",
    "rec mds-open 1.2 ptr move 970 1032 click",
    "note grows from the bottom on Material's default spring ≈320 ms; title, Open in Files; Colloid icons; Quick Look eye in Material Symbols",
    "key -k Escape",
])

# ── launchpad and Today ─────────────────────────────────────────────────────
F("AppDrawer", ["material"], lambda lk: [
    "ptr move 1700 900",
    "rec ad-open 1.2 ipc panels open launchpad IRICE-TEST",
    "note sheet rises 96 px from 0.94, settles ≈270 ms, no overshoot; cells come in one after another (14 ms apart)",
    "rec ad-hover 1.2 ptr move 531 346 glide 1387 346 600",
    "note each card turns secondaryContainer, corner 28→64 on a quick spring; no other card flickers",
    "rec ad-press 1.2 ptr move 531 346 down sleep 300 up",
    "key -k Tab", "rec ad-keys 1.2 key -d 100 -k Down -k Down -k Down -k Down",
    "note selection moves a row each; grid scrolls on a spring, settles ≈460 ms after the last key",
    "key fire", "wait 0.5", "shot ad-search", "key -k Escape", "rec ad-close 0.8 key -k Escape",
    "note close 192 ms on the exit curve",
    "ipc panels open launchpad IRICE-TEST", "wait 0.8", "key -k Tab",
    "rec ad-fling 2.5 ptr move 900 500 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30",
    "note keeps gliding after the last step, slowing steadily; glide length within ±15 % of QML",
    "rec ad-onestep 1.5 ptr scroll 0 30", "note a lone step moves 30 px and stops dead",
    "key -k Escape",
])
F("AppLibraryGlass", ["glass"], lambda lk: [
    "ptr move 1700 900",
    "rec alg-open 1.2 ipc panels open launchpad IRICE-TEST",
    "note 0.96→1 and fade; expressive ≈450 ms with slight overshoot",
    "rec alg-hover 1.2 ptr move 512 294 glide 1408 294 600", "note fillHover + 2 px accent border at once",
    "rec alg-press 1 ptr move 512 294 down sleep 300 up", "note 0.97 in 100 ms linear",
    "key -k Tab", "rec alg-scrub 1 ptr move 1508 300 click", "note grid scrolls OutCubic 320 ms; F's apps lit",
    "key fire", "wait 0.5", "key -k Escape", "rec alg-close 0.8 key -k Escape",
    "ipc panels open launchpad IRICE-TEST", "wait 0.8", "key -k Tab",
    "rec alg-fling 2.5 ptr move 900 500 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30 sleep 10 scroll 0 30",
    "key -k Escape",
])
F("AppLibraryModel", ["material", "glass"], lambda lk: [
    "ipc panels open launchpad IRICE-TEST", "wait 1", "shot alm-folders",
    "note same folders, order and counts as QML; every preview icon the app's coloured Colloid logo",
    "key term", "wait 0.6", "shot alm-search", "key -k Escape", "key -k Tab", "wait 0.5", "shot alm-az", "key -k Escape -k Escape",
])
F("AppLibrary", ["material", "glass"], lambda lk: [
    "ipc panels open launchpad IRICE-TEST", "wait 0.8", "ipc panels close", "wait 0.3",
    "rec al-reopen 1 ipc panels open launchpad IRICE-TEST",
    "note the second open starts at once (no empty frame)", "ipc panels close",
])
F("TodayScreen", ["glass", "material"], lambda lk: [
    "rec ts-bloom 1.6 ipc today preview bloom",
    "note frost 380 ms, panel condenses from 106 % from 140 ms (520 ms OutCubic), rows rise one after another; ≈880 ms",
    "static ts-panel 530 230 860 620", "wait 3", "note nothing reloads while open",
    "rec ts-close 0.8 key -k Escape",
    "rec ts-shrink 1.2 ipc today preview shrink", "key -k Escape", "wait 0.6",
    "rec ts-drop 1.2 ipc today preview drop", "key -k Escape",
])
F("TodayContent", ["glass"], lambda lk: [
    "note add two to-dos first (Today or Reminders' field): Buy milk, Call Ada",
    "ipc today preview bloom", "wait 2",
    "rec tc-tick 1.5 ptr move 571 500 glide 571 520 200 click",
    "note find the first to-do ring (x 571): hover turns it accent; click: it leaves, others don't fade in again",
    "key -k Tab -k Tab -k Tab", "wait 0.3", "shot tc-focus", "key -k Escape",
])
F("MaterialTodayContent", ["material"], lambda lk: [
    "ipc today preview bloom", "wait 2",
    "rec mtc-press 1.5 ptr move 1321 806 down sleep 300 up",
    "note Close swells ≈6 %, corners 20→12, bounces on release (≈0.5 s)",
    "key -k Escape",
])
F("TodayButton", ["glass"], lambda lk: [
    "ipc today preview bloom", "wait 2",
    "rec tb-press 1 ptr move 1318 800 sleep 300 down sleep 300 up",
    "note hover darker accent (150 ms); press 0.97 over 100 ms",
    "key -k Escape",
])
F("TodayOptions", ["glass", "material"], lambda lk: [
    "ipc today preview bloom", "wait 2",
    "note click Options (left of Pin, left of Close)", "shot to-buttons",
    "key -k Escape",
])

# ── background and wallpaper ────────────────────────────────────────────────
F("Background", ["material", "glass"], lambda lk: [
    "ipc panels open wallpaper IRICE-TEST", "wait 1", "key -k Right -k Right",
    "rec bg-crossfade 1.6 key -k Return",
    "note new picture fades in over the old, 700 ms InOutQuad, no dark frame",
    "key -k Escape", "wait 0.5",
    "static bg-wall 1500 300 300 200",
    "setup foot &", "wait 1.5", CLOSE_FOOT, "wait 1.5",
    "ipc panels open launchpad IRICE-TEST", "wait 1", "ipc panels close", "wait 1",
    "ipc lock lock", "wait 2",
    "rec bg-lock 1.5 ipc lock unlock",
    "note wallpaper starts 5 % larger and settles to 1.0 with the veil",
])
F("ScreenCorners", ["material"], lambda lk: [
    "shot sc-corners", "note 12 px concave black corners bottom-left and bottom-right",
    cfg("appearance.screenCornerRadius", 24), "wait 1.5", "shot sc-24",
    cfg("appearance.screenCornerRadius", 12),
])
F("WallpaperPicker", ["material", "glass"], lambda lk: [
    "ptr move 1500 400",
    "rec wp-open 1 ipc panels open wallpaper IRICE-TEST",
    "note opacity 0→1 and 16 px rise over 320 ms; thumbnails fade in 220 ms; applied one ringed",
    "key -k Right -k Right", "wait 0.4", "shot wp-keys",
    "rec wp-apply 1.6 key -k Return",
    "note Material: the chosen thumbnail's corner rounds 16→28, no overshoot (≈250-300 ms)",
    "rec wp-close 0.8 key -k Escape",
])

# ── services ────────────────────────────────────────────────────────────────
files["Recorder"] = HEAD + ["", "look glass",
    "ipc recorder screen", "wait 3",
    "setup sh -c 'p=$(pgrep -x wf-recorder || pgrep -x gpu-screen-reco); grep -E \"SigIgn|SigBlk\" /proc/$p/status'",
    "note SigIgn without bit 2 (SIGINT) and SigBlk 0",
    "ptr move 900 500 glide 1100 600 2000",
    "rec rc-stop 3 ipc recorder stop",
    "note within ~1 s: recorder gone, timer gone, \"Recording saved\" on the island; the file plays (ffprobe)",
    "ipc recorder screen", "wait 10",
    "rec rc-static-stop 9 ipc recorder stop",
    "note a still screen: ends within 8 s at the latest (log: SIGTERM / SIGKILL escalation)",
]

for stem, lines in files.items():
    with open(os.path.join(OUT, stem + ".steps"), "w") as f:
        f.write("\n".join(lines) + "\n")
print(len(files), "step files")
