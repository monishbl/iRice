pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Tahoe-style window tiling: move a window to a half, quarter or third of the
// screen, fill or centre it, arrange several side by side, and return it to
// its previous size. Snapped windows float, so the layout stays put.
Singleton {
    id: root

    readonly property int gapOut: 10 // matches general.gaps_out
    readonly property int gapIn: 5 // half of the space between two tiles

    // [x, y, w, h] as fractions of the usable area
    readonly property var layouts: ({
            left: [0, 0, 0.5, 1],
            right: [0.5, 0, 0.5, 1],
            top: [0, 0, 1, 0.5],
            bottom: [0, 0.5, 1, 0.5],
            topLeft: [0, 0, 0.5, 0.5],
            topRight: [0.5, 0, 0.5, 0.5],
            bottomLeft: [0, 0.5, 0.5, 0.5],
            bottomRight: [0.5, 0.5, 0.5, 0.5],
            leftThird: [0, 0, 1 / 3, 1],
            centerThird: [1 / 3, 0, 1 / 3, 1],
            rightThird: [2 / 3, 0, 1 / 3, 1],
            leftTwoThirds: [0, 0, 2 / 3, 1],
            rightTwoThirds: [1 / 3, 0, 2 / 3, 1],
            fill: [0, 0, 1, 1],
            center: [0.15, 0.1, 0.7, 0.8]
        })
    readonly property var names: ({
            left: "Left", right: "Right", top: "Top", bottom: "Bottom",
            topLeft: "Top Left", topRight: "Top Right", bottomLeft: "Bottom Left", bottomRight: "Bottom Right",
            leftThird: "Left Third", centerThird: "Center Third", rightThird: "Right Third",
            leftTwoThirds: "Left Two Thirds", rightTwoThirds: "Right Two Thirds",
            fill: "Fill", center: "Center"
        })

    // address -> { floating, at, size } before the first snap
    property var previous: ({})

    function ipcOf(t) {
        return t?.lastIpcObject ?? null;
    }

    // Usable area of the window's monitor in global coordinates
    function area(t) {
        const mon = t?.monitor ?? Hyprland.focusedMonitor;
        const o = mon?.lastIpcObject;
        const r = o?.reserved ?? [0, 0, 0, 0]; // left, top, right, bottom
        const x = (mon?.x ?? 0) + r[0], y = (mon?.y ?? 0) + r[1];
        const w = (o?.width ?? mon?.width ?? 1920) / (o?.scale ?? 1) - r[0] - r[2];
        const h = (o?.height ?? mon?.height ?? 1080) / (o?.scale ?? 1) - r[1] - r[3];
        return { x: x + gapOut, y: y + gapOut, w: w - 2 * gapOut, h: h - 2 * gapOut };
    }

    // Rect for a layout; edges shared with another tile get half the inner gap
    function rect(layout, a) {
        const f = layouts[layout];
        const l = f[0] > 0 ? gapIn : 0, t = f[1] > 0 ? gapIn : 0;
        const r = f[0] + f[2] < 0.999 ? gapIn : 0, b = f[1] + f[3] < 0.999 ? gapIn : 0;
        if (layout === "center")
            return { x: Math.round(a.x + a.w * f[0]), y: Math.round(a.y + a.h * f[1]), w: Math.round(a.w * f[2]), h: Math.round(a.h * f[3]) };
        return { x: Math.round(a.x + a.w * f[0] + l), y: Math.round(a.y + a.h * f[1] + t), w: Math.round(a.w * f[2] - l - r), h: Math.round(a.h * f[3] - t - b) };
    }

    function place(t, r) {
        const addr = t?.address;
        if (!addr)
            return;
        const sel = "address:" + (addr.startsWith("0x") ? addr : "0x" + addr);
        const o = ipcOf(t);
        if (o && !previous[addr]) {
            const p = Object.assign({}, previous);
            p[addr] = { floating: o.floating, at: o.at, size: o.size };
            previous = p;
        }
        if (o?.fullscreen > 0)
            Hyprland.dispatch("hl.dsp.window.fullscreen({ mode = \"maximized\", action = \"unset\", window = \"" + sel + "\" })");
        if (!o?.floating)
            Hyprland.dispatch("hl.dsp.window.float({ action = \"enable\", window = \"" + sel + "\" })");
        // resize keeps the centre, so move afterwards
        Hyprland.dispatch("hl.dsp.window.resize({ x = " + r.w + ", y = " + r.h + ", window = \"" + sel + "\" })");
        Hyprland.dispatch("hl.dsp.window.move({ x = " + r.x + ", y = " + r.y + ", window = \"" + sel + "\" })");
    }

    function snap(layout, toplevel) {
        const t = toplevel ?? Apps.activeHyprland;
        if (!t || !layouts[layout])
            return;
        if (layout === "center") {
            // Keep the size, centre in the usable area
            const o = ipcOf(t), a = area(t);
            const w = o?.floating ? o.size[0] : Math.round(a.w * 0.7), h = o?.floating ? o.size[1] : Math.round(a.h * 0.8);
            place(t, { x: Math.round(a.x + (a.w - w) / 2), y: Math.round(a.y + (a.h - h) / 2), w: w, h: h });
            return;
        }
        place(t, rect(layout, area(t)));
    }

    function canRestore(toplevel) {
        return !!previous[(toplevel ?? Apps.activeHyprland)?.address];
    }

    // Back to the size and place it had before the first snap (or tiled again)
    function restore(toplevel) {
        const t = toplevel ?? Apps.activeHyprland;
        const p = previous[t?.address];
        if (!p)
            return;
        const sel = "address:" + (t.address.startsWith("0x") ? t.address : "0x" + t.address);
        if (!p.floating) {
            Hyprland.dispatch("hl.dsp.window.float({ action = \"disable\", window = \"" + sel + "\" })");
        } else {
            Hyprland.dispatch("hl.dsp.window.resize({ x = " + p.size[0] + ", y = " + p.size[1] + ", window = \"" + sel + "\" })");
            Hyprland.dispatch("hl.dsp.window.move({ x = " + p.at[0] + ", y = " + p.at[1] + ", window = \"" + sel + "\" })");
        }
        const n = Object.assign({}, previous);
        delete n[t.address];
        previous = n;
    }

    // Windows on the focused window's workspace, focused first
    function workspaceWindows() {
        const active = Apps.activeHyprland;
        const ws = active?.workspace ?? Hyprland.focusedMonitor?.activeWorkspace;
        const list = Hyprland.toplevels.values.filter(t => t.workspace === ws && !ipcOf(t)?.hidden);
        return list.sort((a, b) => (a === active ? -1 : b === active ? 1 : (ipcOf(a)?.focusHistoryID ?? 99) - (ipcOf(b)?.focusHistoryID ?? 99)));
    }

    // Arrange the most recent windows: leftRight | topBottom | quarters
    function arrange(kind) {
        const plan = { leftRight: ["left", "right"], topBottom: ["top", "bottom"], quarters: ["topLeft", "topRight", "bottomLeft", "bottomRight"] }[kind];
        if (!plan)
            return;
        const wins = workspaceWindows().slice(0, plan.length);
        wins.forEach((t, i) => snap(plan[i], t));
    }

    IpcHandler {
        target: "tiling"
        function snap(layout: string): void { root.snap(layout); }
        function arrange(kind: string): void { root.arrange(kind); }
        function restore(): void { root.restore(); }
    }
}
