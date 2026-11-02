pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import "SpaceMap.js" as Map

// Spaces, per display (like macOS): every display has its own, numbered from
// 1. Hyprland's workspace ids are global, so each display owns a block of 100
// ids (SpaceMap.js: the built-in panel 1..99, the first external 101..199 …),
// kept per display identity in state.json. Everything that shows or picks a
// space goes through here: the menu bar's chips, Overview, Mission Control,
// the switcher; the keybinds map Super+number in Lua (keybinds.lua).
//
// Hyprland itself keeps spaces on their displays: the blocks go to it as
// workspace rules (spaces.lua in your iRice folder, which keybinds.lua loads
// at startup, and live: IRICE_SPACE_RULES), by the display's identity, so a
// space opens on its own display from anywhere, a display starts on its
// first space (docked at login too), and its spaces come back when it's
// plugged in again. A display that's unplugged keeps its spaces meanwhile:
// Hyprland moves them onto another one as they are, listed there after that
// display's own, marked with where they belong. The shell moves nothing
// after the fact; it only renumbers the first space of a display it hasn't
// met yet (Hyprland gave it a plain number), and a display that's back shows
// the space it showed before.
//
// What the views show comes from Quickshell's Hyprland objects, refreshed
// from Hyprland's own lists after every change that can leave them behind
// (a space or window moving between displays, a display coming or going).
//
// Reordering (Overview, Mission Control: drag a space): see move().
Singleton {
    id: root

    readonly property bool sandbox: Quickshell.env("IRICE_SANDBOX") === "1"
    readonly property var store: UiState.data.spaces ?? ({})
    // Connected displays: connector -> base (set from Hyprland's own lists)
    property var bases: ({})
    // Every display we know: base -> short name ("Built-in", "U2720Q")
    property var labels: ({})
    // Connected displays: connector -> identity key
    property var keys: ({})

    // ── Mapping (for views) ─────────────────────────────────────────────
    function baseOf(name) {
        return bases[name] ?? 0;
    }
    function numberOf(id) {
        return id > 0 ? Map.numberOf(id) : id;
    }
    function idFor(name, n) {
        return baseOf(name) + n;
    }
    // The connected display a space belongs to ("" when it isn't connected)
    function ownerName(id) {
        const b = Map.baseOfId(id);
        for (const n in bases)
            if (bases[n] === b)
                return n;
        return "";
    }
    function ownerLabel(id) {
        return labels[Map.baseOfId(id)] ?? "";
    }
    function isOwn(id, name) {
        return id > 0 && Map.baseOfId(id) === baseOf(name);
    }
    // "Space 3", or "Space 3 · U2720Q" when it belongs to another display
    function title(id, name, word) {
        const t = (word ?? "Space") + " " + numberOf(id);
        return isOwn(id, name) || !ownerLabel(id) ? t : t + " · " + ownerLabel(id);
    }
    function labelForName(name) {
        return labels[baseOf(name)] ?? name;
    }

    // Every regular space and the display it's on, by name. (By name, not by
    // Hyprland's monitor object: an object compared by identity could be a
    // stale one, and spaces then went missing from a display's list.)
    readonly property var workspaces: {
        const out = [];
        const seen = {};
        for (const m of Hyprland.monitors.values) {
            const id = m.activeWorkspace?.id ?? 0;
            if (id > 0 && !seen[id]) {
                seen[id] = true;
                out.push({ id: id, monitor: m.name });
            }
        }
        for (const w of Hyprland.workspaces.values)
            if (w.id > 0 && !seen[w.id]) {
                seen[w.id] = true;
                out.push({ id: w.id, monitor: w.monitor?.name ?? w.lastIpcObject?.monitor ?? "" });
            }
        return out;
    }
    readonly property var active: {
        const m = {};
        for (const mon of Hyprland.monitors.values)
            m[mon.name] = mon.activeWorkspace?.id ?? 0;
        return m;
    }

    // What a display lists: [{ kind: own | guest | other, base, label, ids }]
    function groupsFor(name, others) {
        return Map.groups({
            name: name,
            bases: Object.keys(bases).length ? bases : ({ [name]: 0 }),
            workspaces: workspaces,
            active: active,
            count: Config.options.menubar.workspaceCount,
            others: !!others,
            labels: labels
        });
    }
    function idsFor(name, others) {
        const out = [];
        for (const g of groupsFor(name, others))
            for (const id of g.ids)
                out.push(id);
        return out;
    }
    function nextFreeOn(name) {
        return Map.nextFree(baseOf(name), workspaces.map(w => w.id));
    }

    // ── Actions ─────────────────────────────────────────────────────────
    function q(s) {
        return JSON.stringify(String(s));
    }
    function exists(id) {
        return workspaces.some(w => w.id === id);
    }
    // Moves made from the Overview (or a menu) never take you anywhere, but
    // Hyprland focuses a display on the way when a space it shows moves
    // there (focus is put back in the same step). Meanwhile the "you moved to
    // this display" cue (SpaceSwitch) stays quiet.
    property real quietUntil: 0
    function hush(ms) {
        quietUntil = Math.max(quietUntil, Date.now() + (ms ?? 1200));
    }
    function quiet() {
        return Date.now() < quietUntil;
    }
    // Lua that leaves the pointer and the focused display where they were
    function keeping(lua) {
        hush();
        return "local c = hl.get_cursor_pos(); local m = hl.get_active_monitor()\n" + lua + "if m then hl.dispatch(hl.dsp.focus({ monitor = m.name })) end\nif c then hl.dispatch(hl.dsp.cursor.move({ x = c.x, y = c.y })) end\n";
    }
    function run(lua) {
        Quickshell.execDetached(["hyprctl", "eval", lua]);
        refreshSoon.restart();
    }
    // Go to a space (made on its own display if it doesn't exist yet)
    function go(id) {
        const owner = ownerName(id);
        if (!exists(id) && owner !== "")
            run("hl.dispatch(hl.dsp.focus({ monitor = " + q(owner) + " }))\nhl.dispatch(hl.dsp.focus({ workspace = " + id + " }))\n");
        else
            Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
    }
    // A new empty space on a display, and go there
    function newSpace(name) {
        go(nextFreeOn(name));
    }
    // A window to a space (on that space's own display, even if the space
    // doesn't exist yet); you stay where you are
    function sendWindow(address, id) {
        hush();
        const a = address.startsWith("0x") ? address : "0x" + address;
        let lua = "hl.dispatch(hl.dsp.window.move({ workspace = " + id + ", follow = false, window = " + q("address:" + a) + " }))\n";
        const owner = ownerName(id);
        if (!exists(id) && owner !== "")
            lua = keeping(lua + "hl.dispatch(hl.dsp.workspace.move({ workspace = " + id + ", monitor = " + q(owner) + " }))\n");
        run(lua);
    }
    // A window to another display: onto the space it shows (Overview: drop
    // on a display's edge zone, or right-click a window)
    function sendWindowToDisplay(address, name) {
        if (bases[name] === undefined)
            return;
        const id = active[name] > 0 ? active[name] : nextFreeOn(name);
        sendWindow(address, id);
    }
    // Where each other display is from this one, for the Overview's edge
    // zones: here { x, y, w, h }, others [{ name, x, y, w, h }]
    function edgeZones(here, others) {
        return Map.edgeZones(here, others);
    }
    // The display a drag is pushed against (pointer at or past an edge that
    // faces it), "" if none
    function edgeTarget(here, others, px, py) {
        return Map.edgeTarget(here, others, px, py, 3);
    }
    // A whole space to another display: it becomes that display's next
    // number there, windows and all, as they were (menu bar: right-click a
    // space; Overview: drag a space to a display's edge zone, or right-click)
    function moveSpaceTo(id, name) {
        if (!(id > 0) || bases[name] === undefined)
            return;
        const to = nextFreeOn(name);
        run(keeping("hl.dispatch(hl.dsp.workspace.change_id({ workspace = " + id + ", id = " + to + " }))\nhl.dispatch(hl.dsp.workspace.move({ workspace = " + to + ", monitor = " + q(name) + " }))\n"));
    }

    // Reordering spaces (Overview, Mission Control: drag a space), like macOS:
    // a space dropped before another takes that position and the ones between
    // shift along by one. Hyprland's spaces keep their numbers, so it's their
    // windows that move: in one Lua chunk (hyprctl eval), in order, through
    // hidden holding spaces so nothing lands on top of anything mid-way. If you
    // were on a space that moved, you follow it; Super+number follows the new
    // order by itself.
    // order: the spaces as shown (ids, in order); from: the id dragged;
    // to: the id it was dropped on (it takes that one's place)
    function move(order, from, to) {
        const list = order.filter(id => id > 0);
        const a = list.indexOf(from), b = list.indexOf(to);
        if (a < 0 || b < 0 || a === b)
            return;
        const next = list.slice();
        next.splice(a, 1);
        next.splice(b, 0, from);
        // position i keeps its id list[i], and gets next[i]'s windows
        const lo = Math.min(a, b), hi = Math.max(a, b);
        const winsOf = id => Hyprland.toplevels.values.filter(t => (t.workspace?.id ?? 0) === id).map(t => t.address.startsWith("0x") ? t.address : "0x" + t.address);
        const moves = []; // [address, target id]
        for (let i = lo; i <= hi; i++)
            for (const addr of winsOf(next[i]))
                moves.push([addr, list[i]]);
        if (moves.length === 0)
            return;
        hush();
        const step = (addr, ws) => "hl.dispatch(hl.dsp.window.move({ workspace = " + ws + ", follow = false, window = \"address:" + addr + "\" }))";
        let lua = "";
        for (const [addr] of moves) // everything aside first…
            lua += step(addr, "\"special:irice-reorder\"") + "\n";
        for (const [addr, ws] of moves) // …then into its new place
            lua += step(addr, ws) + "\n";
        // Follow the space you were on
        const here = Hyprland.focusedMonitor?.activeWorkspace?.id ?? 0;
        const i = next.indexOf(here);
        if (here > 0 && i >= lo && i <= hi && list[i] !== here)
            lua += "hl.dispatch(hl.dsp.focus({ workspace = " + list[i] + " }))\n";
        Quickshell.execDetached(["hyprctl", "eval", lua]);
        Qt.callLater(Hyprland.refreshToplevels);
    }

    // ── Keeping track of displays ───────────────────────────────────────
    // A display came (back), as its connect effect hands over (DisplayFx):
    // it shows the space it showed before (its spaces are back already:
    // Hyprland's rules)
    function arrive(name) {
        if (!showOn.includes(name))
            showOn = showOn.concat([name]);
        reconcileSoon.interval = 0;
        reconcileSoon.restart();
    }
    function reconcile() {
        reconcileSoon.interval = 250;
        reconcileSoon.restart();
    }
    property var showOn: [] // displays that should show their last space again
    property var known: [] // space ids seen last time (new ones are Hyprland's)
    property bool started: false
    property int chain: 0 // follow-up passes after our own changes
    Timer {
        id: reconcileSoon
        interval: 250
        onTriggered: {
            if (reader.running) {
                restart();
                return;
            }
            reader.running = true;
        }
    }
    Timer {
        id: refreshSoon
        interval: 250
        onTriggered: Hyprland.refreshWorkspaces()
    }
    Process {
        id: reader
        command: ["sh", "-c", "hyprctl -j monitors; echo; echo '\x1e'; hyprctl -j workspaces"]
        stdout: StdioCollector {
            onStreamFinished: root.apply(text)
        }
    }
    function apply(text) {
        const parts = text.split("\x1e");
        const parse = t => {
            try {
                return JSON.parse(t);
            } catch (e) {
                return null;
            }
        };
        const allMons = parse(parts[0] ?? "");
        const allWs = parse(parts[1] ?? "");
        if (!Array.isArray(allMons) || !Array.isArray(allWs) || !UiState.loaded)
            return;
        const mons = allMons.filter(m => !m.disabled && !DisplayFx.ignored(m.name)).map(m => ({ name: m.name, description: m.description, model: m.model, x: m.x, y: m.y, active: m.activeWorkspace?.id ?? 0 }));
        if (mons.length === 0)
            return;
        // Blocks for every connected display
        const before = store.displays ?? {};
        const displays = Map.assign(before, mons, sandbox);
        const nextBases = Map.basesByName(displays, mons);
        const nextKeys = {};
        for (const m of mons)
            nextKeys[m.name] = Map.keyOf(m);
        const nextLabels = {};
        for (const k in displays)
            nextLabels[displays[k].base] = displays[k].label;
        const changedBases = JSON.stringify(nextBases) !== JSON.stringify(bases) || JSON.stringify(displays) !== JSON.stringify(store.displays ?? {});
        bases = nextBases;
        keys = nextKeys;
        labels = nextLabels;
        if (changedBases || !pushed)
            push(displays);

        // What should move
        const ws = allWs.filter(w => w.id > 0).map(w => ({ id: w.id, monitor: w.monitor, windows: w.windows }));
        const activeOf = {};
        for (const m of mons)
            activeOf[m.name] = m.active;
        // (returning spaces to their displays is Hyprland's, by the rules)
        const ops = Map.plan({ bases: nextBases, workspaces: ws, active: activeOf, known: started ? known : ws.map(w => w.id), migrated: store.migrated === true, returns: false });
        const shownBefore = store.shown ?? {};
        const show = showOn.filter(n => nextBases[n] !== undefined);
        showOn = [];

        let lua = "";
        let ids = ws.map(w => w.id);
        for (const op of ops) {
            if (op.op === "renumber") {
                lua += "hl.dispatch(hl.dsp.workspace.change_id({ workspace = " + op.from + ", id = " + op.to + " }))\n";
                ids = ids.filter(id => id !== op.from).concat([op.to]);
            } else {
                lua += "hl.dispatch(hl.dsp.workspace.move({ workspace = " + op.id + ", monitor = " + q(op.monitor) + " }))\n";
            }
        }
        // A display that came back shows what it showed before (if that
        // space is still there)
        for (const n of show) {
            const id = shownBefore[nextKeys[n]] ?? 0;
            if (id > 0 && Map.baseOfId(id) === nextBases[n] && ids.includes(id))
                lua += "hl.dispatch(hl.dsp.focus({ monitor = " + q(n) + " }))\nhl.dispatch(hl.dsp.focus({ workspace = " + id + " }))\n";
        }
        known = ids;
        started = true;
        const next = Object.assign({}, store);
        next.displays = displays;
        next.migrated = true;
        if (JSON.stringify(next) !== JSON.stringify(store))
            UiState.data.spaces = next;
        if (lua !== "") {
            run(keeping(lua));
            // (what Hyprland does in turn: a display left without a space gets one)
            if (chain < 3) {
                chain++;
                reconcileSoon.interval = 600;
                reconcileSoon.restart();
            }
        } else {
            chain = 0;
        }
        noteShown();
    }

    // The blocks to Hyprland: every display we know, by identity, as its
    // workspace rules (now, and in spaces.lua for the next login), and the
    // connected ones by connector for the keybinds
    property bool pushed: false
    readonly property string rulesFile: Config.dir + "/spaces.lua"
    function push(displays) {
        const known = displays ?? store.displays ?? {};
        let b = "IRICE_SPACE_BASES = {";
        for (const k in known)
            b += " [" + q(k) + "] = " + known[k].base + ",";
        b += " }\n";
        let t = "IRICE_SPACES = {";
        for (const n in bases)
            t += " [" + q(n) + "] = " + bases[n] + ",";
        t += " }\n";
        Quickshell.execDetached(["hyprctl", "eval", b + t + "if IRICE_SPACE_RULES then IRICE_SPACE_RULES(IRICE_SPACE_BASES) end\n"]);
        if (!sandbox || Object.keys(known).length > 0) {
            rulesWriter.command = ["sh", "-c", "mkdir -p \"$(dirname \"$1\")\" && printf '%s' \"$2\" > \"$1.tmp\" && mv -f \"$1.tmp\" \"$1\"", "sh", rulesFile,
                "-- Generated by iRice (shell/services/Spaces.qml): which display each block of spaces\n-- belongs to. Loaded by hypr/irice/keybinds.lua; changes here are overwritten.\n" + b];
            rulesWriter.running = true;
        }
        pushed = true;
    }
    Process {
        id: rulesWriter
    }

    // The space each display shows (its own ones only), kept for when it's
    // unplugged and comes back
    function noteShown() {
        const cur = store.shown ?? {};
        const next = Object.assign({}, cur);
        let changed = false;
        for (const m of Hyprland.monitors.values) {
            const id = m.activeWorkspace?.id ?? 0;
            const k = keys[m.name];
            if (k && id > 0 && Map.baseOfId(id) === bases[m.name] && next[k] !== id) {
                next[k] = id;
                changed = true;
            }
        }
        if (changed)
            UiState.data.spaces = Object.assign({}, store, { shown: next });
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            switch (event.name) {
            case "configreloaded": // (a reload starts Hyprland's Lua afresh)
                root.push();
                break;
            case "workspacev2":
            case "focusedmonv2":
                shownLater.restart();
                freshen.restart();
                break;
            case "monitoraddedv2":
            case "monitorremovedv2":
            case "createworkspacev2":
            case "moveworkspacev2":
                root.reconcile();
                freshen.restart();
                break;
            case "destroyworkspacev2":
            case "renameworkspace":
            case "movewindowv2":
            case "openwindow":
            case "closewindow":
                freshen.restart();
                break;
            }
        }
    }
    Timer {
        id: shownLater
        interval: 150
        onTriggered: root.noteShown()
    }
    // Quickshell's monitors, workspaces and windows from Hyprland's own lists
    // (which space a display shows, which display a space is on, where a
    // window is): its event-by-event updates can miss a move between displays
    Timer {
        id: freshen
        interval: 60
        onTriggered: {
            Hyprland.refreshMonitors();
            Hyprland.refreshWorkspaces();
            Hyprland.refreshToplevels();
        }
    }
    Connections {
        target: UiState
        function onLoadedChanged() {
            if (UiState.loaded)
                root.reconcile();
        }
    }
    Component.onCompleted: if (UiState.loaded) reconcile()
}
