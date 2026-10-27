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
// A screen that just arrived goes dark at once (pending) and is filled once
// its place is known, so it never shows the desktop before the effect.
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
    function ignored(name) {
        return !name || /^(HEADLESS|IRICE-)/.test(name);
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
    function show(list) {
        const keep = runs.filter(r => !list.some(x => x.screen === r.screen));
        runs = keep.concat(list.map(x => Object.assign({ id: ++serial, style: style, reverse: 0, focus: 0.5, pending: false }, x)));
    }
    function finish(id) {
        runs = runs.filter(r => r.id !== id);
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
            if (n === "monitoraddedv2") {
                root.added = root.added.concat([name]);
                // dark right away; filled once we know where it is
                if (root.style !== "off" && Sounds.armed)
                    root.show([{ screen: name, role: 0, edge: 0, pending: true }]);
            } else {
                root.removed = root.removed.concat([name]);
            }
            settle.restart();
        }
    }
    // After a hotplug settles (outputs can come and go several times in a
    // second) and Displays has put the saved arrangement back (500 ms)
    Timer {
        id: settle
        interval: 900
        onTriggered: read.running = true
    }
    Process {
        id: read
        command: ["hyprctl", "-j", "monitors"]
        stdout: StdioCollector {
            onStreamFinished: {
                let list = [];
                try {
                    list = JSON.parse(text);
                } catch (e) {
                    list = [];
                }
                const now = {};
                for (const m of list)
                    if (!root.ignored(m.name) && !m.disabled)
                        now[m.name] = root.rectOf(m);
                root.happen(now);
            }
        }
    }
    function happen(now) {
        const before = geometry;
        const arrived = [...new Set(added)].filter(n => now[n] && !before[n]);
        const left = [...new Set(removed)].filter(n => !now[n] && before[n]);
        added = [];
        removed = [];
        geometry = now;
        // A pending screen that turned out not to be new: let it go
        runs = runs.filter(r => !r.pending || arrived.includes(r.screen));
        if (arrived.length === 0 && left.length === 0)
            return;
        if (!Sounds.armed) // the shell is still starting (displays it found, not plugged in)
            return;

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
        }
        if (style !== "off")
            root.show(list);
        else
            runs = runs.filter(r => !r.pending);
        if (directional)
            sound(arrived.length > 0, side);
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
