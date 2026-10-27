pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Per-app window rules (Settings → Window Rules): float, open on a space,
// always opaque, start maximized or fullscreen, show on all spaces, default
// size. Matched by window class.
//
// Stored in Config.options.windowRules.rules, one JSON string per app (plain
// strings: Quickshell 0.2's JsonAdapter is unreliable with var properties).
//
// Applied three ways:
// - Written to ~/.config/irice/window-rules.lua, which hyprland.lua loads with
//   pcall(dofile, ...), so the rules exist from the moment Hyprland starts.
// - Live, with `hyprctl eval`. Each app's rule keeps its handle in a Lua table
//   so a changed rule replaces the old one (set_enabled(false)) instead of
//   piling up. When a rule loses an effect and the hook is in the Hyprland
//   config, the config is reloaded instead, which is exact.
// - After a config reload: if hyprland.lua doesn't load the file, the rules
//   are evaluated again (a reload drops everything added with eval).
Singleton {
    id: root

    readonly property string file: Config.dir + "/window-rules.lua"
    readonly property string hyprDir: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/hypr"
    // False until the windowRules section exists in Config
    readonly property bool available: Config.options.windowRules !== undefined

    // [{ class, name, float, tile, workspace (0 = any), opaque, start ("" |
    // "maximize" | "fullscreen"), pin, width, height (0 = no size) }]
    // (tile: tiles even when new windows float by default, defaultLayout)
    readonly property var rules: {
        const out = [];
        for (const s of (Config.options.windowRules?.rules ?? [])) {
            try {
                const r = JSON.parse(s);
                if (r && r.class)
                    out.push(normalize(r));
            } catch (e) {}
        }
        return out;
    }

    // Whether hyprland.lua (or custom.lua) loads window-rules.lua
    property bool hooked: false
    property bool hookChecked: false
    property string error: ""

    // ── Editing ──────────────────────────────────────────────────────────
    function normalize(r) {
        const toInt = v => Math.max(0, parseInt(v) || 0);
        return {
            class: String(r.class),
            name: String(r.name || r.class),
            float: !!r.float,
            tile: !!r.tile && !r.float && !r.pin,
            workspace: toInt(r.workspace),
            opaque: !!r.opaque,
            start: r.start === "maximize" || r.start === "fullscreen" ? r.start : "",
            pin: !!r.pin,
            width: toInt(r.width),
            height: toInt(r.height)
        };
    }

    function ruleFor(cls) {
        return rules.find(r => r.class === cls) ?? null;
    }

    // A rule to show for an app that has none yet (saved on the first change)
    function draft(cls, name) {
        return ruleFor(cls) ?? normalize({ class: cls, name: name });
    }

    function save(list) {
        if (available)
            Config.options.windowRules.rules = list.map(r => JSON.stringify(normalize(r)));
    }

    function set(cls, name, patch) {
        const list = rules.slice();
        const i = list.findIndex(r => r.class === cls);
        const r = Object.assign({}, i >= 0 ? list[i] : normalize({ class: cls, name: name }), patch);
        if (i >= 0)
            list[i] = r;
        else
            list.push(r);
        save(list);
    }

    // Returns the index it had, for undo
    function remove(cls) {
        const i = rules.findIndex(r => r.class === cls);
        save(rules.filter(r => r.class !== cls));
        return i;
    }

    function restore(rule, index) {
        const list = rules.filter(r => r.class !== rule.class);
        list.splice(Math.max(0, Math.min(index, list.length)), 0, rule);
        save(list);
    }

    function hasEffects(r) {
        return r.float || r.tile || r.workspace > 0 || r.opaque || r.start !== "" || r.pin || (r.width > 0 && r.height > 0);
    }

    // "Floats · Space 3 · Opaque"
    function summary(r) {
        const s = [];
        if (r.pin)
            s.push("All spaces");
        else if (r.float)
            s.push("Floats");
        else if (r.tile)
            s.push("Tiles");
        if (r.workspace > 0)
            s.push("Space " + r.workspace);
        if (r.start === "maximize")
            s.push("Maximized");
        else if (r.start === "fullscreen")
            s.push("Fullscreen");
        if (r.opaque)
            s.push("Opaque");
        if (r.width > 0 && r.height > 0)
            s.push(r.width + " × " + r.height);
        return s.length > 0 ? s.join(" · ") : "Nothing set";
    }

    // ── Lua ──────────────────────────────────────────────────────────────
    function luaStr(s) {
        return '"' + String(s).replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n") + '"';
    }

    function classPattern(cls) {
        return "^(" + cls.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + ")$";
    }

    // The hl.window_rule spec for a rule, or "nil" when it does nothing
    function spec(r) {
        if (!r || !hasEffects(r))
            return "nil";
        const f = ["name = " + luaStr("irice-window-rules:" + r.class), "match = { class = " + luaStr(classPattern(r.class)) + " }"];
        if (r.float || r.pin) // (pinning only works on floating windows)
            f.push("float = true");
        else if (r.tile)
            f.push("tile = true");
        if (r.workspace > 0)
            f.push("workspace = " + luaStr(String(r.workspace)));
        if (r.opaque)
            f.push('opacity = "1.0 override 1.0 override"');
        if (r.start === "fullscreen")
            f.push("fullscreen = true");
        else if (r.start === "maximize")
            f.push("maximize = true");
        if (r.pin)
            f.push("pin = true");
        if (r.width > 0 && r.height > 0)
            f.push("size = " + luaStr(r.width + " " + r.height));
        return "{ " + f.join(", ") + " }";
    }

    // Replaces an app's rule: disables the one it had (if any), adds the new
    readonly property string prelude: "IRICE_WINDOW_RULES = IRICE_WINDOW_RULES or {}\n" + "local function irice_rule(key, spec)\n" + "    local old = IRICE_WINDOW_RULES[key]\n" + "    if old then pcall(function() old:set_enabled(false) end) end\n" + "    IRICE_WINDOW_RULES[key] = spec and hl.window_rule(spec) or nil\n" + "end\n"

    function fileText(list) {
        // (Loaded only when the config loads, when no earlier rule exists, so
        // the handle table starts empty)
        let s = "-- Generated by iRice from Settings → Window Rules; changes here are overwritten.\n" + "-- Loaded from hyprland.lua: pcall(dofile, cfg .. \"/irice/window-rules.lua\")\n" + "IRICE_WINDOW_RULES = {}\n" + prelude + layoutLua(list);
        for (const r of list)
            if (hasEffects(r))
                s += "irice_rule(" + luaStr(r.class) + ", " + spec(r) + ")\n";
        return s;
    }

    // ── New windows: tiled or floating ───────────────────────────────────
    // Floating: one rule floats every window, except apps whose rule says
    // "tile" (left out of its match, so no rule order is involved). Dialogs
    // and other child windows float either way (Hyprland). Kept in the rules
    // file (from the start of the session) and applied live with eval.
    readonly property string layout: Config.ready ? (Config.options.windowRules?.defaultLayout ?? "tiled") : ""
    readonly property bool floating: layout === "floating"
    function layoutSpec(list) {
        const tiles = list.filter(r => r.tile).map(r => r.class.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
        const match = tiles.length > 0 ? "negative:^(" + tiles.join("|") + ")$" : ".*";
        return "{ name = \"irice-default-layout\", match = { class = " + luaStr(match) + " }, float = true }";
    }
    function layoutLua(list) {
        return "if IRICE_DEFAULT_LAYOUT then pcall(function() IRICE_DEFAULT_LAYOUT:set_enabled(false) end) end\n"
            + "IRICE_DEFAULT_LAYOUT = " + (floating ? "hl.window_rule(" + layoutSpec(list) + ")" : "nil") + "\n";
    }
    property string layoutApplied: ""
    function applyLayout() {
        if (!Config.ready || layout === "")
            return;
        const lua = layoutLua(rules);
        if (lua === layoutApplied)
            return;
        layoutApplied = lua;
        Quickshell.execDetached(["hyprctl", "eval", lua]);
    }
    // Switching the setting also re-arranges what's open on the spaces you
    // can see (other spaces follow when you open new windows there)
    property string lastLayout: ""
    onLayoutChanged: {
        if (layout === "")
            return;
        applyLayout();
        if (lastLayout !== "" && lastLayout !== layout)
            arrange.running = true;
        lastLayout = layout;
    }
    Process {
        id: arrange
        command: ["sh", "-c", "hyprctl -j clients; echo '\u001e'; hyprctl -j monitors"]
        stdout: StdioCollector {
            onStreamFinished: root.rearrange(text)
        }
    }
    function rearrange(text) {
        const parts = text.split("\u001e");
        let clients = [], mons = [];
        try {
            clients = JSON.parse(parts[0]);
            mons = JSON.parse(parts[1]);
        } catch (e) {
            return;
        }
        const ruleOf = cls => rules.find(r => r.class === cls) ?? null;
        const addr = a => "address:" + (a.startsWith("0x") ? a : "0x" + a);
        let lua = "";
        for (const m of mons) {
            const ws = m.activeWorkspace?.id ?? 0;
            if (ws <= 0)
                continue;
            const mine = clients.filter(c => c.workspace?.id === ws && c.mapped !== false && !c.fullscreen && !c.pinned);
            if (floating) {
                // Float the tiled ones, cascading from the centre at about
                // two thirds of the screen
                const w = Math.round(m.width / (m.scale || 1) * 0.62), h = Math.round(m.height / (m.scale || 1) * 0.68);
                let i = 0;
                for (const c of mine) {
                    if (c.floating || ruleOf(c.class)?.tile)
                        continue;
                    const x = Math.round(m.x + (m.width / (m.scale || 1) - w) / 2 + (i % 6) * 32);
                    const y = Math.round(m.y + (m.height / (m.scale || 1) - h) / 2 + (i % 6) * 32);
                    lua += "hl.dispatch(hl.dsp.window.float({ action = \"enable\", window = \"" + addr(c.address) + "\" }))\n"
                        + "hl.dispatch(hl.dsp.window.resize({ x = " + w + ", y = " + h + ", window = \"" + addr(c.address) + "\" }))\n"
                        + "hl.dispatch(hl.dsp.window.move({ x = " + x + ", y = " + y + ", window = \"" + addr(c.address) + "\" }))\n";
                    i++;
                }
            } else {
                // Tile the floating ones, except apps set to float and the
                // small ones (dialogs, pickers and other helpers)
                for (const c of mine) {
                    const r = ruleOf(c.class);
                    if (!c.floating || r?.float || r?.pin || (c.size?.[0] ?? 0) < 480 || (c.size?.[1] ?? 0) < 360)
                        continue;
                    lua += "hl.dispatch(hl.dsp.window.float({ action = \"disable\", window = \"" + addr(c.address) + "\" }))\n";
                }
            }
        }
        if (lua !== "")
            Quickshell.execDetached(["hyprctl", "eval", lua]);
    }

    // ── Syncing ──────────────────────────────────────────────────────────
    // What Hyprland has now (class -> rule) and what the file holds
    property var applied: ({})
    property var written: ({})
    property bool started: false // the first sync after the shell starts
    property bool resetTable: false // after a reload: old handles are gone
    property bool reloadSeen: false // a reload happened; re-check the hook, then re-apply

    function snapshot() {
        const m = {};
        for (const r of rules)
            m[r.class] = r;
        return m;
    }

    // Each effect the old rule had is still there, unchanged
    function additive(a, b) {
        if (!a)
            return true;
        if (!b)
            return !hasEffects(a);
        const fx = r => [r.float || r.pin, r.tile, r.workspace, r.opaque, r.start, r.pin, r.width > 0 && r.height > 0 ? r.width + "x" + r.height : ""];
        const x = fx(a), y = fx(b);
        for (let i = 0; i < x.length; i++)
            if (x[i] && x[i] !== y[i])
                return false;
        return true;
    }

    onRulesChanged: {
        syncTimer.restart();
        Qt.callLater(applyLayout);
    }
    Connections {
        target: Config
        function onReadyChanged() {
            syncTimer.restart();
            Qt.callLater(root.applyLayout);
        }
    }
    Timer {
        id: syncTimer
        interval: 400 // one write for a burst of edits
        onTriggered: root.sync()
    }

    function sync() {
        if (!Config.ready || !available || !hookChecked)
            return;
        if (writer.running) {
            syncTimer.restart();
            return;
        }
        const next = snapshot();
        writer.next = next;
        writer.command = ["sh", "-c", "mkdir -p \"$(dirname \"$1\")\" && printf '%s' \"$2\" > \"$1.tmp\" && mv -f \"$1.tmp\" \"$1\"", "sh", root.file, fileText(rules)];
        writer.running = true;
    }

    function apply(next) {
        // On shell start with the hook in place, Hyprland loaded the file
        // itself; nothing to add
        if (!started) {
            started = true;
            if (hooked) {
                applied = next;
                return;
            }
        }
        const changed = [];
        let exact = true;
        for (const k in next)
            if (spec(applied[k]) !== spec(next[k]))
                changed.push(k);
        for (const k in applied)
            if (!(k in next) && spec(applied[k]) !== "nil")
                changed.push(k);
        if (changed.length === 0)
            return;
        for (const k of changed)
            exact = exact && additive(applied[k], next[k]);

        if (!exact && hooked) {
            // Something was turned off: reload, and the file brings exactly
            // the current rules back
            Quickshell.execDetached(["sh", "-c", "hyprctl reload config-only >/dev/null"]);
            applied = next;
            return;
        }
        let lua = (resetTable ? "IRICE_WINDOW_RULES = {}\n" : "") + prelude;
        resetTable = false;
        for (const k of changed)
            lua += "irice_rule(" + luaStr(k) + ", " + spec(next[k]) + ")\n";
        Quickshell.execDetached(["hyprctl", "eval", lua]);
        applied = next;
    }

    Process {
        id: writer
        property var next: ({})
        onExited: code => {
            if (code !== 0) {
                root.error = "Couldn't save " + root.file + ". Check that the folder is writable.";
                return;
            }
            root.error = "";
            root.written = next;
            root.apply(next);
        }
    }

    Process {
        id: hookCheck
        running: true
        command: ["sh", "-c", "grep -qsF window-rules.lua \"$1/hyprland.lua\" \"$1/custom.lua\"", "sh", root.hyprDir]
        onExited: code => {
            root.hooked = code === 0;
            root.hookChecked = true;
            if (root.reloadSeen) {
                root.reloadSeen = false;
                if (root.hooked) {
                    // The reload loaded the file as last written
                    root.applied = root.written;
                } else {
                    // A reload drops everything added with eval: add it all
                    // again, into a fresh handle table
                    root.applied = {};
                    root.resetTable = true;
                }
            }
            syncTimer.restart();
        }
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name !== "configreloaded")
                return;
            root.reloadSeen = true;
            root.layoutApplied = ""; // a reload drops rules added with eval
            Qt.callLater(root.applyLayout);
            hookCheck.running = true; // the hook may have just been added
        }
    }
}
