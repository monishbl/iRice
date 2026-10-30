pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common

// Each space remembers the display it belongs on. A space's home is where it
// was made, or where you moved it (menu bar: right-click a space). Kept in
// state.json, so it survives restarts.
//
// When a display leaves (DisplayFx calls gather): Hyprland moves its spaces to
// another display. The windows of the space it was showing come onto the
// remaining display's current space, entering from the side the display was
// on and settling into place (Hyprland's own window animation, so it costs the
// shell nothing). When it comes back (DisplayFx calls arrive, as its connect
// effect hands over): its spaces go home, it shows the space it showed
// before, and the windows that came over go back to it, gliding across.
Singleton {
    id: root

    readonly property var homes: UiState.data.workspaceHomes ?? ({}) // id -> display name
    property real removedAt: 0 // moves right after a display goes are Hyprland's, not yours
    property real movingUntil: 0 // our own moves (their events aren't a new home)

    // The space each display showed last: { display name: id }. Kept for
    // displays that left, so the same space is shown when they're back
    property var shown: ({})
    // Windows gathered from a display that left: { address: { ws, into, mon } }
    property var gathered: ({})

    function setHome(id, mon) {
        if (!(id > 0) || !mon || homes[id] === mon)
            return;
        const next = Object.assign({}, homes);
        next[id] = mon;
        UiState.data.workspaceHomes = next;
    }
    function homeAll() { // spaces without a home yet: where they are now
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0 && w.monitor && !homes[w.id])
                setHome(w.id, w.monitor.name);
    }
    function noteShown() {
        const next = Object.assign({}, shown);
        let changed = false;
        for (const m of Hyprland.monitors.values) {
            const id = m.activeWorkspace?.id ?? 0;
            if (m.name && id > 0 && next[m.name] !== id) {
                next[m.name] = id;
                changed = true;
            }
        }
        if (changed)
            shown = next;
    }

    function q(s) {
        return JSON.stringify(String(s));
    }
    function win(addr) {
        return "window = " + q("address:" + addr);
    }
    function run(lua) {
        movingUntil = Date.now() + 2000;
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }

    // A display left. removed: its name; rect: where it was {x, y, w, h};
    // target: the display its windows come onto, from `monitors` (hyprctl -j
    // monitors, logical rect in target.rect); edge: target's side facing
    // where it was (0 left, 1 top, 2 right, 3 bottom); clients: hyprctl -j
    // clients; anim: windowsMove as hyprctl -j animations has it (null:
    // no glide); still: reduced motion (no glide, they just come over)
    function gather(removed, target, edge, clients, anim, still) {
        const from = shown[removed] ?? 0;
        const into = target.activeWorkspace?.id ?? 0;
        if (!(from > 0) || !(into > 0) || from === into)
            return;
        const list = clients.filter(c => c.workspace?.id === from && c.mapped !== false && !c.pinned);
        if (list.length === 0)
            return;
        const T = target.rect;
        const glide = !still && anim && anim.enabled;
        let lua = "", settle = "", plain = "";
        const next = Object.assign({}, gathered);
        list.forEach((c, i) => {
            next[c.address] = { ws: from, into: into, mon: removed };
            const W = win(c.address);
            const toWs = "hl.dispatch(hl.dsp.window.move({ workspace = " + into + ", follow = false, " + W + " }))\n";
            if (!glide || (c.fullscreen ?? 0) > 0) {
                plain += toWs;
                return;
            }
            // Its size now (Hyprland laid it out on the display it went to),
            // at most most of the target's
            const sw = Math.max(120, Math.min(c.size[0], Math.round(T.w * 0.9)));
            const sh = Math.max(80, Math.min(c.size[1], Math.round(T.h * 0.9)));
            // Starts two-thirds beyond the shared edge, a little apart from
            // the others along it
            const off = (i % 4) * 32;
            let x, y;
            if (edge === 0 || edge === 2) {
                x = edge === 2 ? T.x + T.w - Math.round(sw / 3) : T.x - sw + Math.round(sw / 3);
                y = T.y + Math.max(0, Math.round((T.h - sh) / 2)) + off;
            } else {
                y = edge === 3 ? T.y + T.h - Math.round(sh / 3) : T.y - sh + Math.round(sh / 3);
                x = T.x + Math.max(0, Math.round((T.w - sw) / 2)) + off;
            }
            if (!c.floating)
                lua += "hl.dispatch(hl.dsp.window.float({ action = \"enable\", " + W + " }))\n";
            lua += "hl.dispatch(hl.dsp.window.resize({ x = " + sw + ", y = " + sh + ", " + W + " }))\n";
            lua += "hl.dispatch(hl.dsp.window.move({ x = " + x + ", y = " + y + ", " + W + " }))\n" + toWs;
            if (!c.floating) {
                settle += "hl.dispatch(hl.dsp.window.float({ action = \"disable\", " + W + " }))\n";
            } else {
                // a floating window keeps its size and its place on the
                // display it went to, kept inside the target
                const fw = Math.min(c.size[0], T.w), fh = Math.min(c.size[1], T.h);
                const mon = (target.all ?? []).find(m => m.id === c.monitor);
                const rx = mon ? c.at[0] - mon.x : c.at[0] - T.x, ry = mon ? c.at[1] - mon.y : c.at[1] - T.y;
                const fx = T.x + Math.max(0, Math.min(rx, T.w - fw)), fy = T.y + Math.max(0, Math.min(ry, T.h - fh));
                settle += "hl.dispatch(hl.dsp.window.resize({ x = " + fw + ", y = " + fh + ", " + W + " }))\n";
                settle += "hl.dispatch(hl.dsp.window.move({ x = " + fx + ", y = " + fy + ", " + W + " }))\n";
            }
        });
        gathered = next;
        let all = plain;
        if (lua !== "") {
            // Placed at the edge in one step (window moves don't animate for
            // that moment), then let go: Hyprland glides them into place
            all += "hl.animation({ leaf = \"windowsMove\", enabled = false })\n" + lua + restoreAnim(anim) + "hl.timer(function()\n" + settle + "end, { timeout = 34, type = \"oneshot\" })\n";
        }
        run(all);
    }
    // windowsMove back as it was
    function restoreAnim(a) {
        let s = "hl.animation({ leaf = \"windowsMove\", enabled = true";
        if (a.speed > 0)
            s += ", speed = " + a.speed;
        if (a.bezier)
            s += ", bezier = " + q(a.bezier);
        if (a.style)
            s += ", style = " + q(a.style);
        return s + " })\n";
    }

    // A display came (back): its spaces return, it shows the space it showed
    // before, and windows gathered from it go back. The pointer and focus stay
    // where they are. clients: hyprctl -j clients
    function arrive(mon, clients) {
        const here = {};
        for (const w of Hyprland.workspaces.values)
            here[w.id] = w.monitor?.name ?? "";
        let lua = "";
        for (const id in homes)
            if (homes[id] === mon && here[id] !== undefined && here[id] !== mon)
                lua += "hl.dispatch(hl.dsp.workspace.move({ workspace = " + id + ", monitor = " + q(mon) + " }))\n";
        const back = [];
        const keep = {};
        for (const addr in gathered) {
            const g = gathered[addr];
            if (g.mon !== mon) {
                keep[addr] = g;
                continue;
            }
            // only while it's still where we put it
            const c = clients.find(x => x.address === addr);
            if (c && c.workspace?.id === g.into)
                back.push({ addr: addr, ws: g.ws });
        }
        gathered = keep;
        const show = shown[mon] ?? 0;
        if (show > 0 && (homes[show] === mon || back.some(b => b.ws === show))) {
            // (focusing the display first makes the space there if it's gone)
            lua = "local c = hl.get_cursor_pos(); local m = hl.get_active_monitor()\n" + lua;
            lua += "hl.dispatch(hl.dsp.focus({ monitor = " + q(mon) + " }))\n";
            lua += "hl.dispatch(hl.dsp.focus({ workspace = " + show + " }))\n";
            for (const b of back)
                lua += "hl.dispatch(hl.dsp.window.move({ workspace = " + b.ws + ", follow = false, " + win(b.addr) + " }))\n";
            lua += "if m then hl.dispatch(hl.dsp.focus({ monitor = m.name })) end\n";
            lua += "if c then hl.dispatch(hl.dsp.cursor.move({ x = c.x, y = c.y })) end\n";
        } else {
            for (const b of back)
                lua += "hl.dispatch(hl.dsp.window.move({ workspace = " + b.ws + ", follow = false, " + win(b.addr) + " }))\n";
        }
        if (lua !== "")
            run(lua);
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            const f = event.data.split(",");
            switch (event.name) {
            case "monitorremovedv2":
            case "monitorremoved":
                root.removedAt = Date.now();
                break;
            case "workspacev2":
            case "focusedmonv2":
                shownLater.restart();
                break;
            case "moveworkspacev2": // id,name,monitor
                if (Date.now() - root.removedAt > 4000 && Date.now() > root.movingUntil)
                    root.setHome(Number(f[0]), f[2]);
                // (not right after a display left: it isn't showing anything any more)
                if (Date.now() - root.removedAt > 4000)
                    shownLater.restart();
                break;
            case "createworkspacev2":
                Qt.callLater(root.homeAll);
                break;
            }
        }
    }
    Timer {
        id: shownLater
        interval: 100
        onTriggered: root.noteShown()
    }
    Connections {
        target: UiState
        function onLoadedChanged() {
            if (UiState.loaded)
                root.homeAll();
        }
    }
    // (state may have loaded before this started; spaces arrive a moment after start)
    Timer {
        running: true
        interval: 2000
        onTriggered: {
            if (UiState.loaded)
                root.homeAll();
            root.noteShown();
        }
    }
}
