pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Display connect and disconnect, shown (modules/osd/DisplayConnect.qml) and
// heard (Settings → Displays → When a display connects). Which way the new
// display is comes from the arrangement: Hyprland's, once Displays has put
// back the one you saved for it ("monitor memory"), so a display you keep on
// the left arrives from the left. A display that left is placed from where it
// was a moment ago.
//
// runs: what each screen shows right now, one entry per screen:
//   { id, screen, role (0 arrived | 1 beside), edge (0 left, 1 top, 2 right,
//     3 bottom: the side facing the other display), focus (where along that
//     edge, 0 .. 1), reverse (1: a display left), style, pending }
// A screen that just arrived is covered at once (pending: it would only show
// a black, still empty desktop), and its effect starts as soon as its place
// is known, a fraction of a second later. The screens already there never
// darken: they only get light at the shared edge.
//
// Windows (WorkspaceHomes): a display that left sends the windows it showed
// onto its neighbour, entering from its side while the effect plays; one that
// arrives gets its spaces and windows back as its effect hands over to the
// desktop, so they glide in over the real wallpaper.
Singleton {
    id: root

    readonly property var cfg: Config.options.displays
    readonly property string style: Config.ready ? (cfg.connectAnimation ?? "extend") : "off"
    readonly property bool directional: Config.ready ? (cfg.directionalSound ?? true) : false
    property var runs: []
    property int serial: 0

    // Logical rectangles of the displays, last we knew: { name: {x, y, w, h} }
    property var geometry: ({})
    property var added: []
    property var removed: []

    function rectOf(m) {
        const s = m.scale || 1;
        const w = Math.round(m.width / s), h = Math.round(m.height / s);
        return (m.transform ?? 0) % 2 ? { x: m.x, y: m.y, w: h, h: w } : { x: m.x, y: m.y, w: w, h: h };
    }
    // (the sandbox's own test output counts there, so hotplug can be tried)
    readonly property bool sandbox: Quickshell.env("IRICE_SANDBOX") === "1"
    function ignored(name) {
        return !name || /^HEADLESS/.test(name) || (/^IRICE-/.test(name) && !sandbox);
    }
    // Reduced motion: no travel (the effect fades, windows just come over)
    readonly property bool still: Config.ready && Config.options.appearance.reduceMotion === true
    // When a display last came or went: focus moves then are the hotplug's,
    // not yours (SpaceSwitch doesn't dim the other displays for them)
    property real hotplugAt: 0
    function settling() {
        return hotplugAt > 0 && Date.now() - hotplugAt < 4000;
    }

    // Where `b` is, seen from `a`: { edge (of a), focus (along a's edge),
    // otherEdge, otherFocus } or null when they don't touch (or overlap)
    function relation(a, b) {
        const tol = 8;
        let edge = -1;
        if (b.x >= a.x + a.w - tol)
            edge = 2;
        else if (b.x + b.w <= a.x + tol)
            edge = 0;
        else if (b.y >= a.y + a.h - tol)
            edge = 3;
        else if (b.y + b.h <= a.y + tol)
            edge = 1;
        if (edge < 0)
            return null;
        const vertical = edge === 0 || edge === 2; // the shared edge runs up and down
        const lo = vertical ? Math.max(a.y, b.y) : Math.max(a.x, b.x);
        const hi = vertical ? Math.min(a.y + a.h, b.y + b.h) : Math.min(a.x + a.w, b.x + b.w);
        const mid = hi > lo ? (lo + hi) / 2 : (vertical ? b.y + b.h / 2 : b.x + b.w / 2);
        const clamp = v => Math.max(0, Math.min(1, v));
        return {
            edge: edge,
            focus: clamp(vertical ? (mid - a.y) / a.h : (mid - a.x) / a.w),
            otherEdge: (edge + 2) % 4,
            otherFocus: clamp(vertical ? (mid - b.y) / b.h : (mid - b.x) / b.w),
            gap: 0
        };
    }
    function distance(a, b) {
        const dx = (a.x + a.w / 2) - (b.x + b.w / 2), dy = (a.y + a.h / 2) - (b.y + b.h / 2);
        return Math.hypot(dx, dy);
    }
    // The displays touching `r` (nearest first), each with its relation
    function neighbours(r, others) {
        return Object.keys(others).map(n => ({ name: n, rel: relation(others[n], r), d: distance(others[n], r) })).filter(x => x.rel).sort((p, q) => p.d - q.d);
    }

    // ms the arrived screen waits for its neighbour's part (DisplayConnect)
    function leadFor(fx) {
        return fx === "liquid" ? 620 : fx === "extend" ? 120 : 0;
    }
    // (pending runs not in the list end; the others are replaced in place,
    // so their screens keep the overlay window they already have)
    function show(list) {
        const keep = runs.filter(r => !r.pending && !list.some(x => x.screen === r.screen));
        runs = keep.concat(list.map(x => Object.assign({ id: ++serial, style: style, reverse: 0, focus: 0.5, pending: false }, x)));
    }
    function finish(id) {
        const r = runs.find(x => x.id === id);
        runs = runs.filter(x => x.id !== id);
        if (r && r.role === 0)
            handover(r.screen);
    }
    // The arrived screen's effect is handing over to its desktop (called by
    // DisplayConnect as the overlay starts to fade)
    property var waiting: []
    function handover(screen) {
        if (!waiting.includes(screen))
            return;
        waiting = waiting.filter(n => n !== screen);
        refreshClients.screen = screen;
        refreshClients.running = true;
    }
    // (the windows may have changed while the effect played)
    Process {
        id: refreshClients
        property string screen
        command: ["hyprctl", "-j", "clients"]
        stdout: StdioCollector {
            onStreamFinished: {
                let list = root.clients;
                try {
                    list = JSON.parse(text);
                } catch (e) {}
                WorkspaceHomes.arrive(refreshClients.screen, list);
            }
        }
    }
    // An effect that never got to play doesn't keep the windows away
    Timer {
        id: handoverLimit
        interval: 3500
        onTriggered: root.waiting.slice().forEach(n => root.handover(n))
    }

    // ── Events ──────────────────────────────────────────────────────────
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            const n = event.name;
            if (n !== "monitoraddedv2" && n !== "monitorremovedv2")
                return;
            const name = event.data.split(",")[1];
            if (root.ignored(name))
                return;
            root.hotplugAt = Date.now();
            if (n === "monitoraddedv2")
                root.added = root.added.concat([name]);
            else
                root.removed = root.removed.concat([name]);
            // The new screen is covered right away (filled once we know where
            // it is), and the others get their (still invisible) overlay now
            // too: making a window costs the GUI thread a moment, better
            // while the new screen is being set up than once its effect plays
            if (root.style !== "off" && Sounds.armed) {
                // (by name: the new screen may not be listed yet; its overlay
                // is made as soon as it is)
                const names = Quickshell.screens.map(s => s.name).filter(s => s !== name).concat(n === "monitoraddedv2" ? [name] : []);
                const want = names.filter(s => !root.ignored(s) && (s === name || !root.still) && !root.runs.some(r => r.screen === s));
                if (want.length > 0)
                    root.runs = root.runs.concat(want.map(s => ({ id: ++root.serial, screen: s, role: s === name ? 0 : 1, edge: 0, focus: 0.5, reverse: 0, style: root.style, pending: true })));
            }
            // The first event is read at once (the new screen is waiting);
            // the rest of a burst (outputs can come and go several times in
            // a second) once it's quiet for a moment, at most 600 ms on
            const quiet = !settle.running && !read.running && Date.now() - root.burstAt > 600;
            if (quiet)
                root.burstAt = Date.now();
            settle.interval = quiet ? 0 : Math.max(0, Math.min(150, 600 - (Date.now() - root.burstAt)));
            settle.restart();
        }
    }
    property real burstAt: 0
    property bool readAgain: false
    Timer {
        id: settle
        interval: 150
        onTriggered: {
            if (read.running)
                root.readAgain = true;
            else
                read.running = true;
        }
    }
    // Displays, windows and Hyprland's window-move animation, in one call
    Process {
        id: read
        command: ["sh", "-c", "hyprctl -j monitors; echo; echo '\x1e'; hyprctl -j clients; echo; echo '\x1e'; hyprctl -j animations"]
        stdout: StdioCollector {
            onStreamFinished: {
                const parts = text.split("\x1e");
                const parse = t => {
                    try {
                        return JSON.parse(t);
                    } catch (e) {
                        return null;
                    }
                };
                const list = parse(parts[0] ?? "") ?? [];
                const now = {};
                for (const m of list)
                    if (!root.ignored(m.name) && !m.disabled)
                        now[m.name] = root.rectOf(m);
                root.monitors = list;
                root.clients = parse(parts[1] ?? "") ?? [];
                const anims = parse(parts[2] ?? "");
                root.moveAnim = root.windowsMove(anims);
                root.happen(now);
                if (root.readAgain) {
                    root.readAgain = false;
                    read.running = true;
                }
            }
        }
    }
    property var monitors: []
    property var clients: []
    property var moveAnim: null
    // windowsMove as it's set (inherited from windows, then global)
    function windowsMove(anims) {
        const all = Array.isArray(anims) ? (Array.isArray(anims[0]) ? anims[0] : anims) : [];
        const get = n => all.find(a => a.name === n);
        const g = get("global");
        if (Config.ready && g && !g.enabled)
            return null;
        for (const n of ["windowsMove", "windows", "global"]) {
            const a = get(n);
            if (a && (a.overridden || n === "global"))
                return { enabled: a.enabled, speed: a.speed, bezier: a.bezier, style: a.style };
        }
        return null;
    }

    function happen(now) {
        const before = geometry;
        const arrived = [...new Set(added)].filter(n => now[n] && !before[n]);
        const left = [...new Set(removed)].filter(n => !now[n] && before[n]);
        // (an event that came while this was being read waits for the next read)
        const more = settle.running || readAgain;
        added = more ? added.filter(n => !arrived.includes(n) && !now[n]) : [];
        removed = more ? removed.filter(n => !left.includes(n) && now[n]) : [];
        geometry = now;
        if (arrived.length === 0 && left.length === 0 || !Sounds.armed) {
            // nothing came or went (or the shell is still starting: displays
            // it found, not plugged in): pending overlays go
            if (!more)
                runs = runs.filter(r => !r.pending);
            return;
        }

        const list = [];
        let side = ""; // for the sound: "left" | "right" | ""
        const sideOf = edge => edge === 0 ? "left" : edge === 2 ? "right" : "";
        const old = {};
        for (const n in now)
            if (!arrived.includes(n))
                old[n] = now[n];
        for (const n of arrived) {
            const near = neighbours(now[n], old);
            const first = near[0];
            // with a neighbour, the new screen starts once the drop has left it
            // (liquid) or the fold has begun (extend)
            list.push({ screen: n, role: 0, edge: first ? first.rel.otherEdge : 0, focus: first ? first.rel.otherFocus : 0.5, lead: first ? root.leadFor(style) : 0 });
            for (const x of near)
                list.push({ screen: x.name, role: 1, edge: x.rel.edge, focus: x.rel.focus, reverse: 0 });
            if (first && side === "")
                side = sideOf(first.rel.edge);
        }
        for (const n of left) {
            const near = neighbours(before[n], now);
            for (const x of near)
                list.push({ screen: x.name, role: 1, edge: x.rel.edge, focus: x.rel.focus, reverse: 1 });
            if (near[0] && side === "")
                side = sideOf(near[0].rel.edge);
            // its windows come onto the display beside it (else the focused one)
            const target = near[0] ? near[0].name : (monitors.find(m => m.focused && now[m.name])?.name ?? Object.keys(now)[0]);
            const tm = monitors.find(m => m.name === target);
            if (tm) {
                const rel = near[0] ? near[0].rel.edge : 2;
                WorkspaceHomes.gather(n, Object.assign({}, tm, { rect: now[target], all: monitors }), rel, clients, moveAnim, still);
            }
        }
        // (reduced motion: the screens beside show nothing)
        if (style !== "off")
            whenQuiet(still ? list.filter(r => r.role === 0) : list);
        else
            runs = runs.filter(r => !r.pending);
        // An arrived display's spaces and windows: as its effect hands over
        // (handover), or now when there's no effect to wait for
        for (const n of arrived) {
            if (style !== "off" && list.some(r => r.screen === n && r.role === 0) && Quickshell.screens.some(s => s.name === n))
                waiting = waiting.concat([n]);
            else
                WorkspaceHomes.arrive(n, clients);
        }
        if (waiting.length > 0)
            handoverLimit.restart();
        if (directional)
            sound(arrived.length > 0, side);
    }

    // A screen that was just added makes the GUI thread wait on the first
    // frame of each of its windows (a GL context each: about a second on an
    // iGPU). The effect starts once that has passed (three frames in a row on
    // time, or 600 ms at most), so it plays smoothly; the new screen stays
    // covered meanwhile.
    property var queued: null
    property real quietSince: 0
    property real lastTick: 0
    property int calm: 0
    function whenQuiet(list) {
        queued = queued ? queued.filter(r => !list.some(x => x.screen === r.screen)).concat(list) : list;
        quietSince = Date.now();
        lastTick = Date.now();
        calm = 0;
        quiet.restart();
    }
    Timer {
        id: quiet
        interval: 16
        repeat: true
        onTriggered: {
            const t = Date.now();
            root.calm = t - root.lastTick < 30 ? root.calm + 1 : 0;
            root.lastTick = t;
            if (root.calm >= 3 || t - root.quietSince > 600) {
                stop();
                const list = root.queued ?? [];
                root.queued = null;
                root.show(list);
            }
        }
    }

    // The connect / disconnect sound, panned toward the display (your own
    // sound file, if you picked one, plays as it is)
    function sound(connected, side) {
        const base = connected ? "displays" : "displaysOff";
        const own = Config.options.sounds.displaysFile;
        Sounds.play(side !== "" && !own ? base + (side === "left" ? "Left" : "Right") : base);
    }

    // Settings' Preview buttons and `qs ipc call displays preview liquid left`:
    // on the focused display, the light runs out toward `side`, then the
    // display fills from that side as if it had just been plugged in there
    function preview(effect, side) {
        const fx = ["extend", "liquid", "warp"].includes(effect) ? effect : (style !== "off" ? style : "extend");
        const edge = ({ left: 0, top: 1, right: 2, bottom: 3 })[side] ?? 2;
        const screen = Hyprland.focusedMonitor?.name ?? Quickshell.screens[0]?.name ?? "";
        if (screen === "")
            return;
        runs = runs.filter(r => r.screen !== screen);
        runs = runs.concat([{ id: ++serial, screen: screen, role: 1, edge: edge, focus: 0.5, reverse: 0, style: fx, pending: false }]);
        previewNext.screen = screen;
        previewNext.edge = edge;
        previewNext.fx = fx;
        previewNext.restart();
        Sounds.play(edge === 0 ? "displaysLeft" : edge === 2 ? "displaysRight" : "displays", true);
    }
    Timer {
        id: previewNext
        property string screen
        property int edge
        property string fx
        interval: Math.round((fx === "liquid" ? 660 : fx === "extend" ? 260 : 650) * CoolDown.motionScale)
        onTriggered: {
            root.runs = root.runs.filter(r => r.screen !== screen).concat([{ id: ++root.serial, screen: screen, role: 0, edge: edge, focus: 0.5, reverse: 0, style: fx, pending: false }]);
        }
    }

    // What's connected now, so the first plug-in has a "before"
    Component.onCompleted: read.running = true
}
