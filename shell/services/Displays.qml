pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Displays (Settings → Displays): resolution, refresh rate, scale, rotation,
// mirroring and arrangement, applied live with `hyprctl eval hl.monitor(…)`.
// - Saved in Config.options.displays.monitors, one line per display the user
//   changed: "name\tmode\tx\ty\tscale\ttransform\tenabled\tmirror".
// - Also written to ~/.config/irice/monitors.lua, which hyprland.lua loads
//   right after its default monitor line, so displays come up right at login.
// - Put back on shell start, after a config reload and when a display is
//   plugged in, only where Hyprland differs (no needless mode sets).
// - Changes that can leave you without a picture (mode, scale, rotation,
//   mirroring, turning a display off) wait 15 s for "Keep", then go back.
Singleton {
    id: root

    readonly property string file: Config.dir + "/monitors.lua"
    property var monitors: [] // from hyprctl, see parse()
    property bool loaded: false

    // Confirmation after a risky change
    property var pendingRevert: null // [{ name, state }] to put back
    property int confirmLeft: 0
    readonly property bool confirming: pendingRevert !== null

    // "Identify": each screen shows its number for a moment
    property bool identifying: false

    function refresh() {
        reader.running = true;
    }

    // ── Reading ──────────────────────────────────────────────────────────
    function parseModes(list) {
        const out = [], seen = {};
        for (const s of list ?? []) {
            const m = s.match(/^(\d+)x(\d+)@([\d.]+)Hz$/);
            if (!m)
                continue;
            const text = m[1] + "x" + m[2] + "@" + m[3];
            if (seen[text])
                continue;
            seen[text] = true;
            out.push({ w: +m[1], h: +m[2], r: parseFloat(m[3]), text: text });
        }
        return out;
    }
    function rateText(r) {
        return (Math.round(r * 100) / 100).toFixed(2);
    }
    function parse(list) {
        return list.filter(m => !/^(HEADLESS|IRICE-)/.test(m.name)).map(m => ({
            name: m.name,
            description: m.description ?? "",
            label: niceName(m),
            width: m.width,
            height: m.height,
            refresh: m.refreshRate,
            mode: m.width + "x" + m.height + "@" + rateText(m.refreshRate),
            x: m.x,
            y: m.y,
            scale: m.scale,
            transform: m.transform ?? 0,
            enabled: !m.disabled,
            mirror: m.mirrorOf && m.mirrorOf !== "none" ? mirrorName(list, m.mirrorOf) : "",
            builtIn: /^(eDP|LVDS|DSI)/.test(m.name),
            modes: parseModes(m.availableModes)
        }));
    }
    // mirrorOf is the mirrored display's id (or name, depending on version)
    function mirrorName(list, of) {
        const byId = list.find(m => String(m.id) === String(of));
        return byId ? byId.name : String(of);
    }
    function niceName(m) {
        if (/^(eDP|LVDS|DSI)/.test(m.name))
            return "Built-in Display";
        const model = (m.model ?? "").trim();
        const make = (m.make ?? "").trim();
        if (model && model !== "0x0000")
            return model.startsWith(make) || !make ? model : make + " " + model;
        return m.name;
    }

    Process {
        id: reader
        command: ["hyprctl", "-j", "monitors", "all"]
        stdout: StdioCollector {
            onStreamFinished: {
                let list = [];
                try {
                    list = JSON.parse(text);
                } catch (e) {
                    return;
                }
                root.monitors = root.parse(list);
                const first = !root.loaded;
                root.loaded = true;
                if (root.restorePending) {
                    root.restorePending = false;
                    root.restore();
                } else if (first)
                    root.restore();
            }
        }
    }

    // ── Saved settings ───────────────────────────────────────────────────
    // name -> { mode, x, y, scale, transform, enabled, mirror }
    function savedMap() {
        const out = {};
        for (const line of Config.options.displays.monitors ?? []) {
            const f = line.split("\t");
            if (f.length < 8 || !f[0])
                continue;
            out[f[0]] = { mode: f[1], x: parseInt(f[2]) || 0, y: parseInt(f[3]) || 0, scale: parseFloat(f[4]) || 1, transform: parseInt(f[5]) || 0, enabled: f[6] !== "0", mirror: f[7] };
        }
        return out;
    }
    function lineOf(name, s) {
        return [name, s.mode, Math.round(s.x), Math.round(s.y), s.scale, s.transform, s.enabled ? 1 : 0, s.mirror ?? ""].join("\t");
    }
    function save(map) {
        const lines = [];
        for (const k in map)
            lines.push(lineOf(k, map[k]));
        Config.options.displays.monitors = lines;
        writer.command = ["sh", "-c", "mkdir -p \"$(dirname \"$1\")\" && printf '%s' \"$2\" > \"$1.tmp\" && mv -f \"$1.tmp\" \"$1\"", "sh", root.file, fileText(map)];
        writer.running = true;
    }
    Process {
        id: writer
    }

    // What a display looks like now, as a saved entry (cool-down's lowered
    // refresh rate isn't the user's choice: save the rate it will put back)
    function stateOf(m) {
        let mode = m.mode;
        const kept = CoolDown.savedRates?.[m.name];
        if (kept !== undefined)
            mode = m.width + "x" + m.height + "@" + rateText(kept);
        return { mode: mode, x: m.x, y: m.y, scale: m.scale, transform: m.transform, enabled: m.enabled, mirror: m.mirror };
    }

    // ── Applying ─────────────────────────────────────────────────────────
    function luaStr(s) {
        return "\"" + String(s).replace(/\\/g, "\\\\").replace(/"/g, "\\\"") + "\"";
    }
    function spec(name, s) {
        if (!s.enabled)
            return "hl.monitor({ output = " + luaStr(name) + ", disabled = true })";
        let f = "output = " + luaStr(name) + ", mode = " + luaStr(s.mode || "preferred") + ", position = " + luaStr(Math.round(s.x) + "x" + Math.round(s.y)) + ", scale = " + s.scale;
        if (s.transform)
            f += ", transform = " + s.transform;
        if (s.mirror)
            f += ", mirror = " + luaStr(s.mirror);
        return "hl.monitor({ " + f + " })";
    }
    function fileText(map) {
        let s = "-- Generated by iRice from Settings → Displays; changes here are overwritten.\n" + "-- Loaded from hyprland.lua: pcall(dofile, IRICE_USER_DIR .. \"/monitors.lua\")\n";
        for (const k in map)
            s += spec(k, map[k]) + "\n";
        return s;
    }
    function same(a, b) {
        if (a.enabled !== b.enabled)
            return false;
        if (!a.enabled)
            return true;
        const ma = String(a.mode).match(/^(\d+)x(\d+)@?([\d.]*)/), mb = String(b.mode).match(/^(\d+)x(\d+)@?([\d.]*)/);
        const modeSame = a.mode === "preferred" || b.mode === "preferred" ? true : ma && mb ? ma[1] === mb[1] && ma[2] === mb[2] && (!ma[3] || !mb[3] || Math.abs(parseFloat(ma[3]) - parseFloat(mb[3])) < 0.5) : a.mode === b.mode;
        return modeSame && Math.round(a.x) === Math.round(b.x) && Math.round(a.y) === Math.round(b.y) && Math.abs(a.scale - b.scale) < 0.01 && a.transform === b.transform && (a.mirror ?? "") === (b.mirror ?? "");
    }
    function evalAll(entries) {
        if (entries.length === 0)
            return;
        Quickshell.execDetached(["hyprctl", "eval", entries.map(e => spec(e.name, e.state)).join("\n")]);
        settle.restart();
    }
    Timer {
        id: settle
        interval: 700
        onTriggered: root.refresh()
    }

    // Saved settings where Hyprland differs (cool-down's own rate is left alone).
    // A display whose mode is already right is sent its mode as it is now
    // (rate included), so moving it, say, never asks for a new mode: a mode
    // set blanks the screen for a moment. Displays that already match get
    // nothing at all.
    function modeSame(a, b) {
        return same({ enabled: true, mode: a, x: 0, y: 0, scale: 1, transform: 0, mirror: "" }, { enabled: true, mode: b, x: 0, y: 0, scale: 1, transform: 0, mirror: "" });
    }
    function restore() {
        const saved = savedMap();
        const todo = [];
        for (const m of monitors) {
            const s = saved[m.name];
            if (!s)
                continue;
            const now = stateOf(m);
            if (CoolDown.savedRates?.[m.name] !== undefined)
                now.mode = s.mode; // the rate is cool-down's business while it lasts
            if (same(now, s))
                continue;
            const want = Object.assign({}, s);
            if (m.enabled && s.enabled && modeSame(now.mode, s.mode))
                want.mode = m.mode; // what it runs at now, exactly
            todo.push({ name: m.name, state: want });
        }
        evalAll(todo);
    }

    // Change one display. `changes` holds any of mode, x, y, scale,
    // transform, enabled, mirror. Risky changes ask to be kept.
    function set(name, changes) {
        setMany([{ name: name, changes: changes }]);
    }
    function setMany(list) {
        const saved = savedMap();
        const entries = [], before = [];
        let risky = false;
        for (const item of list) {
            const m = monitors.find(x => x.name === item.name);
            if (!m)
                continue;
            const old = stateOf(m);
            const next = Object.assign({}, saved[item.name] ?? old, item.changes);
            // Positions always come from what's on screen now unless changed
            if (item.changes.x === undefined) {
                next.x = old.x;
                next.y = old.y;
            }
            // Turning a display back on: its preferred mode, right of the others
            if (next.enabled && (!next.mode || /^0x0/.test(next.mode)))
                next.mode = "preferred";
            if (next.enabled && !old.enabled && !saved[item.name] && item.changes.x === undefined) {
                next.x = monitors.filter(o => o.enabled && !o.mirror && o.name !== item.name).reduce((r, o) => Math.max(r, o.x + logicalSize(o).w), 0);
                next.y = 0;
            }
            if (same(old, next))
                continue;
            for (const k of ["mode", "scale", "transform", "enabled", "mirror"])
                if (item.changes[k] !== undefined && item.changes[k] !== old[k])
                    risky = true;
            before.push({ name: item.name, state: saved[item.name] ? saved[item.name] : old, hadSaved: !!saved[item.name] });
            saved[item.name] = next;
            entries.push({ name: item.name, state: next });
        }
        if (entries.length === 0)
            return;
        // A second change while confirming keeps the oldest state to go back to
        if (risky && !confirming) {
            pendingRevert = before;
            confirmLeft = 15;
            countdown.restart();
        }
        save(saved);
        evalAll(entries);
    }

    function keep() {
        countdown.stop();
        pendingRevert = null;
    }
    function revert() {
        countdown.stop();
        const back = pendingRevert ?? [];
        pendingRevert = null;
        const saved = savedMap();
        for (const b of back) {
            if (b.hadSaved)
                saved[b.name] = b.state;
            else
                delete saved[b.name];
        }
        save(saved);
        evalAll(back.map(b => ({ name: b.name, state: b.state })));
    }
    Timer {
        id: countdown
        interval: 1000
        repeat: true
        onTriggered: {
            root.confirmLeft -= 1;
            if (root.confirmLeft <= 0)
                root.revert();
        }
    }

    // Back to what Hyprland picks by itself
    function resetAll() {
        Config.options.displays.monitors = [];
        writer.command = ["sh", "-c", "rm -f \"$1\" && hyprctl reload config-only >/dev/null", "sh", root.file];
        writer.running = true;
        settle.restart();
    }

    function identify() {
        identifying = true;
        identifyTimer.restart();
    }
    Timer {
        id: identifyTimer
        interval: 2500
        onTriggered: root.identifying = false
    }

    // Numbers left to right, then top to bottom (as in the arrangement)
    function numberOf(name) {
        const order = monitors.slice().sort((a, b) => a.x - b.x || a.y - b.y);
        return order.findIndex(m => m.name === name) + 1;
    }

    // Logical size: what windows see (scaled, and turned for 90° / 270°)
    function logicalSize(m, s) {
        const st = s ?? m;
        const mm = String(st.mode ?? m.mode).match(/^(\d+)x(\d+)/);
        let w = mm ? +mm[1] : m.width, h = mm ? +mm[2] : m.height;
        if ((st.transform ?? 0) % 2 === 1)
            [w, h] = [h, w];
        const sc = st.scale || 1;
        return { w: Math.round(w / sc), h: Math.round(h / sc) };
    }

    property bool restorePending: false
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            const n = event.name;
            // (one read for a burst of hotplug events)
            if (n === "configreloaded" || n === "monitoraddedv2") {
                root.restorePending = true;
                restoreDelay.restart();
            } else if (n === "monitorremovedv2")
                restoreDelay.restart();
        }
    }
    Timer {
        id: restoreDelay
        interval: 500 // after cool-down puts its own rate back
        onTriggered: root.refresh()
    }

    Connections {
        target: Config
        function onReadyChanged() {
            if (Config.ready)
                root.refresh();
        }
    }
    Component.onCompleted: if (Config.ready) refresh()
}
