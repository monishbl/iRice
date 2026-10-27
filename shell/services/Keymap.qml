pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Keyboard shortcuts and trackpad gestures for Settings: iRice's defaults
// (hypr/irice/keymap.json and gestures.json, written by tools/gen-keymap.py)
// merged with your changes (<Config.dir>/keybinds.json). Hyprland reads the
// same files (hypr/irice/keybinds.lua and gestures.lua), so saving writes the
// file and reloads Hyprland's config.
//
// keybinds.json:
//   { "version": 1,
//     "keybinds":  { "<default id>": { "keys": "...", "disabled": true } },
//     "shortcuts": [ { id: "custom-…", keys, group: "Custom", label, action, opts? } ],
//     "gestures":  { "changes": { "<id>": { fingers, direction, action, disabled } },
//                    "custom": [ { id, fingers, direction, label, action } ] } }
//
// The cheatsheet still reads live binds from Hyprland (Keybinds.qml); it is
// refreshed after each reload.
Singleton {
    id: root

    // ── Where things are ─────────────────────────────────────────────────
    readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config"
    // The hypr data: $IRICE_ROOT/hypr/irice; else the repo next to the shell
    // (shell/ and hypr/ are siblings) when keymap.json is there; else the
    // installed copy in ~/.config/hypr/irice. Empty until that's known.
    // (Quickshell.shellDir, not a relative URL: those can't leave the shell folder)
    readonly property string repoHyprDir: Quickshell.shellDir.replace(/\/[^\/]*\/?$/, "") + "/hypr/irice"
    property string probedHyprDir: ""
    readonly property string hyprDir: Quickshell.env("IRICE_ROOT") ? Quickshell.env("IRICE_ROOT") + "/hypr/irice" : probedHyprDir
    Process {
        running: !Quickshell.env("IRICE_ROOT")
        command: ["test", "-f", root.repoHyprDir + "/keymap.json"]
        onExited: code => root.probedHyprDir = code === 0 ? root.repoHyprDir : root.configHome + "/hypr/irice"
    }
    readonly property string userPath: Config.dir + "/keybinds.json"
    readonly property string home: Quickshell.env("HOME")
    readonly property string scriptsDir: decodeURIComponent(Qt.resolvedUrl("../assets/scripts").toString().replace("file://", ""))
    // $MOD is Super, except in the nested sandbox (Alt)
    readonly property string modKey: Quickshell.env("IRICE_SANDBOX") === "1" && !Launch.tty ? "ALT" : "SUPER"

    // ── State ────────────────────────────────────────────────────────────
    property var defaults: []
    property var gestureDefaults: []
    property var user: blankUser()
    property bool defaultsLoaded: false
    property bool userLoaded: false
    readonly property bool ready: defaultsLoaded && userLoaded
    // What went wrong, written so the user can fix it ("" = fine)
    property string defaultsError: ""
    property string userError: ""
    property string saveError: ""
    property bool saving: false
    // Bumped on every change so bindings re-evaluate
    property int revision: 0

    property bool hasZenity: false
    property bool hasKdialog: false
    property bool hasGtkLaunch: false

    function blankUser() {
        return { version: 1, keybinds: {}, shortcuts: [], gestures: { changes: {}, custom: [] } };
    }
    function clone(x) {
        return JSON.parse(JSON.stringify(x));
    }
    function normalizeUser(u) {
        const out = blankUser();
        if (!u || typeof u !== "object")
            return out;
        if (u.keybinds && typeof u.keybinds === "object" && !Array.isArray(u.keybinds))
            out.keybinds = u.keybinds;
        if (Array.isArray(u.shortcuts))
            out.shortcuts = u.shortcuts.filter(s => s && typeof s === "object");
        const g = u.gestures;
        if (g && typeof g === "object") {
            if (g.changes && typeof g.changes === "object" && !Array.isArray(g.changes))
                out.gestures.changes = g.changes;
            if (Array.isArray(g.custom))
                out.gestures.custom = g.custom.filter(x => x && typeof x === "object");
        }
        return out;
    }

    // ── Readable names ───────────────────────────────────────────────────
    // iRice actions (GlobalShortcut names, appid "irice")
    readonly property var shellActions: [
        { name: "spotlight", label: "Spotlight" },
        { name: "overview", label: "Overview" },
        { name: "missionControl", label: "Mission Control" },
        { name: "appExpose", label: "App Exposé" },
        { name: "launchpad", label: "App Library" },
        { name: "showDesktop", label: "Show desktop" },
        { name: "switcher", label: "Switch windows" },
        { name: "switcherBack", label: "Switch windows backwards" },
        { name: "controlCenter", label: "Control Center" },
        { name: "notificationCenter", label: "Notification Center" },
        { name: "sidebar", label: "AI and tools sidebar" },
        { name: "clipboard", label: "Clipboard history" },
        { name: "emoji", label: "Emoji and symbols" },
        { name: "settings", label: "Settings" },
        { name: "cheatsheet", label: "Keyboard shortcuts cheatsheet" },
        { name: "keyboard", label: "On-screen keyboard" },
        { name: "overlay", label: "Overlay widgets" },
        { name: "wallpaper", label: "Change wallpaper" },
        { name: "darkMode", label: "Light or dark appearance" },
        { name: "caffeine", label: "Caffeine on or off" },
        { name: "quickNote", label: "Quick note" },
        { name: "session", label: "Power options" },
        { name: "lock", label: "Lock the screen" },
        { name: "reset", label: "Close everything (if something is stuck)" },
        { name: "screenshotRegion", label: "Screenshot of an area" },
        { name: "screenshotScreen", label: "Screenshot of the screen" },
        { name: "screenTools", label: "Screenshot and recording tools" },
        { name: "record", label: "Record an area (again to stop)" },
        { name: "ocr", label: "Copy text from the screen" },
        { name: "screenTranslate", label: "Translate text on the screen" },
        { name: "lens", label: "Search an area with Google Lens" },
        { name: "askScreen", label: "Ask AI about part of the screen" },
        { name: "volumeUp", label: "Volume up" },
        { name: "volumeDown", label: "Volume down" },
        { name: "volumeMute", label: "Mute sound" },
        { name: "micMute", label: "Mute microphone" },
        { name: "brightnessUp", label: "Brightness up" },
        { name: "brightnessDown", label: "Brightness down" },
        { name: "snap-left", label: "Tile window left half" },
        { name: "snap-right", label: "Tile window right half" },
        { name: "snap-top", label: "Tile window top half" },
        { name: "snap-bottom", label: "Tile window bottom half" },
        { name: "snap-fill", label: "Window fills the screen" },
        { name: "snap-center", label: "Centre window" },
        { name: "snap-restore", label: "Window back to its previous size" },
        { name: "tabs", label: "Merge windows into tabs" },
        { name: "pip", label: "Picture in Picture" }
    ]
    readonly property var roles: [
        { name: "terminal", label: "Terminal" },
        { name: "files", label: "Files" },
        { name: "browser", label: "Browser" },
        { name: "code", label: "Code editor" },
        { name: "text", label: "Text editor" },
        { name: "office", label: "Office" },
        { name: "mixer", label: "Volume mixer" },
        { name: "tasks", label: "Task manager" }
    ]
    // Hyprland's own gesture actions
    readonly property var gestureActions: [
        { name: "workspace", label: "Switch workspace" },
        { name: "move", label: "Move window" },
        { name: "fullscreen", label: "Full screen" },
        { name: "float", label: "Float or tile" },
        { name: "close", label: "Close window" }
    ]
    readonly property var directions: [
        { name: "horizontal", label: "Left or right" },
        { name: "vertical", label: "Up or down" },
        { name: "left", label: "Left" },
        { name: "right", label: "Right" },
        { name: "up", label: "Up" },
        { name: "down", label: "Down" },
        { name: "swipe", label: "Any direction" },
        { name: "pinch", label: "Pinch or spread" },
        { name: "pinchin", label: "Pinch" },
        { name: "pinchout", label: "Spread" }
    ]

    function shellLabel(name) {
        return shellActions.find(a => a.name === name)?.label ?? name;
    }
    function roleLabel(name) {
        return roles.find(r => r.name === name)?.label ?? name;
    }
    function directionLabel(name) {
        return directions.find(d => d.name === name)?.label ?? name;
    }

    // A readable description of what an action does
    function describe(a) {
        if (!a || typeof a !== "object")
            return "Does nothing";
        if (a.shell !== undefined)
            return shellLabel(a.shell);
        if (a.app !== undefined)
            return "Opens your " + roleLabel(a.app).toLowerCase();
        if (a.desktop) {
            const e = Apps.byId(a.desktop);
            return "Opens " + (e?.name ?? a.desktop);
        }
        if (a.exec !== undefined)
            return "Runs " + String(a.exec).replace("$IRICE_SHELL/assets/scripts/", "");
        if (a.zoom !== undefined)
            return a.zoom > 0 ? "Zooms in" : "Zooms out";
        if (a.dsp !== undefined)
            return "Window manager: " + String(a.dsp).replace(/^hl\.dsp\./, "");
        return "Unknown action";
    }
    function describeGesture(a) {
        if (typeof a === "string")
            return gestureActions.find(g => g.name === a)?.label ?? a;
        return describe(a);
    }
    function sameAction(a, b) {
        return actionKey(a) === actionKey(b);
    }
    // One string per distinct action (whitespace in commands doesn't count)
    function actionKey(a) {
        if (typeof a === "string")
            return "g:" + a;
        if (!a || typeof a !== "object")
            return "";
        if (a.shell !== undefined)
            return "shell:" + a.shell;
        if (a.app !== undefined)
            return "app:" + a.app;
        if (a.exec !== undefined)
            return "exec:" + String(a.exec).trim().replace(/\s+/g, " ");
        if (a.zoom !== undefined)
            return "zoom:" + Number(a.zoom).toFixed(2);
        if (a.dsp !== undefined)
            return "dsp:" + String(a.dsp).replace(/\s+/g, "");
        return JSON.stringify(a);
    }

    // ── Keys ─────────────────────────────────────────────────────────────
    readonly property var modOrder: ["$MOD", "CTRL", "ALT", "SHIFT"]
    readonly property var keyLabels: ({
            "Period": ".", "period": ".", "Comma": ",", "comma": ",", "Slash": "/", "slash": "/", "Semicolon": ";", "semicolon": ";",
            "Apostrophe": "'", "apostrophe": "'", "Minus": "-", "minus": "-", "Equal": "=", "equal": "=",
            "bracketleft": "[", "BracketLeft": "[", "bracketright": "]", "BracketRight": "]", "grave": "`", "Grave": "`",
            "backslash": "\\", "Backslash": "\\", "Print": "Print Screen", "Page_Up": "Page Up", "Page_Down": "Page Down",
            "Delete": "Delete", "Home": "Home", "End": "End", "Insert": "Insert",
            "mouse:272": "Left click", "mouse:273": "Right click", "mouse:274": "Middle click", "mouse:275": "Back button", "mouse:276": "Forward button",
            "SUPER_R": "Right Super", "Super_R": "Right Super",
            "mouse_down": "Scroll down", "mouse_up": "Scroll up", "mouse_left": "Scroll left", "mouse_right": "Scroll right",
            "code:86": "Keypad +", "code:82": "Keypad −", "code:63": "Keypad ×", "code:106": "Keypad ÷", "code:104": "Keypad Enter"
        })

    // "$MOD + SHIFT + S" → { mods: ["SUPER", "SHIFT"], key: "S", tap: false }
    function parseKeys(keys) {
        const parts = String(keys ?? "").split("+").map(s => s.trim()).filter(s => s !== "");
        const mods = [];
        const rest = [];
        let tap = false;
        for (const p of parts) {
            const u = p.toUpperCase();
            if (u === "$MOD" || u === "SUPER" || u === "MOD4" || u === "WIN" || u === "META")
                mods.includes("SUPER") || mods.push("SUPER");
            else if (u === "CTRL" || u === "CONTROL")
                mods.includes("CTRL") || mods.push("CTRL");
            else if (u === "ALT" || u === "MOD1")
                mods.includes("ALT") || mods.push("ALT");
            else if (u === "SHIFT")
                mods.includes("SHIFT") || mods.push("SHIFT");
            else if (u === "$MODKEY" || u === "SUPER_L") {
                tap = true;
                rest.push("$MODKEY");
            } else
                rest.push(p);
        }
        return { mods: mods, key: rest.join(" + "), tap: tap };
    }
    // Back to Hyprland's syntax, in a fixed order
    function formatKeys(mods, key) {
        const out = [];
        for (const m of ["SUPER", "CTRL", "ALT", "SHIFT"])
            if (mods.includes(m))
                out.push(m === "SUPER" ? "$MOD" : m);
        if (key)
            out.push(key);
        return out.join(" + ");
    }
    // The same combo, whatever the order or case
    function canonical(keys) {
        const p = parseKeys(keys);
        return p.mods.slice().sort().join("+") + "|" + p.key.toLowerCase();
    }
    function keyLabel(k) {
        if (k === "$MODKEY")
            return "";
        if (keyLabels[k] !== undefined)
            return keyLabels[k];
        if (/^code:\d+$/.test(k))
            return "Key " + k.slice(5);
        if (/^mouse:\d+$/.test(k))
            return "Mouse button " + k.slice(6);
        if (k.startsWith("XF86"))
            return k.slice(4).replace(/([a-z])([A-Z])/g, "$1 $2");
        return k.length === 1 ? k.toUpperCase() : k;
    }
    // Keycaps for KeyCombo, e.g. ["SUPER", "SHIFT", "S"] (KeyNames writes them
    // in the chosen style: Settings → Appearance → Shortcut symbols)
    function keyList(keys) {
        const p = parseKeys(keys);
        // Tapping Right Super (SUPER + SUPER_R) reads as just that key
        const rightTap = /^super_r$/i.test(p.key);
        const mods = p.mods.filter(m => !(rightTap && m === "SUPER")).map(m => m === "SUPER" ? modKey : m);
        if (p.tap && !mods.includes(modKey))
            mods.push(modKey);
        const k = p.key.split(" + ").map(keyLabel).filter(x => x !== "");
        return mods.concat(k);
    }
    // "Super Shift S" (or ⇧⌘S with Mac symbols)
    function keyText(keys) {
        const l = KeyNames.keys(keyList(keys));
        return KeyNames.style === "mac" ? l.join("") : l.join(" ");
    }

    // ── Merged lists ─────────────────────────────────────────────────────
    // Each bind: { id, keys, defaultKeys, group, label, action, opts,
    //   isDefault (unchanged default), custom, disabled, changed, keyList,
    //   keyText, actionText }
    readonly property var binds: {
        revision;
        KeyNames.style;
        const out = [];
        const changes = user.keybinds ?? {};
        for (const d of defaults) {
            const c = changes[d.id] ?? {};
            const keys = c.keys ?? d.keys;
            const keysChanged = c.keys !== undefined && canonical(c.keys) !== canonical(d.keys);
            out.push({
                id: d.id, keys: keys, defaultKeys: d.keys, group: d.group ?? "Other", label: d.label ?? d.id,
                action: d.action, opts: d.opts ?? {}, custom: false, disabled: !!c.disabled,
                changed: keysChanged || !!c.disabled, keysChanged: keysChanged,
                isDefault: !keysChanged && !c.disabled,
                keyList: keyList(keys), keyText: keyText(keys), actionText: describe(d.action)
            });
        }
        for (const s of user.shortcuts ?? []) {
            if (!s.id)
                continue;
            out.push({
                id: s.id, keys: s.keys ?? "", defaultKeys: "", group: "Your shortcuts", label: s.label || describe(s.action),
                action: s.action, opts: s.opts ?? {}, custom: true, disabled: !!s.disabled, changed: false, keysChanged: false,
                isDefault: false, keyList: keyList(s.keys ?? ""), keyText: keyText(s.keys ?? ""), actionText: describe(s.action)
            });
        }
        return out;
    }
    // [{ name, binds }] in the data's order, your own shortcuts first
    readonly property var groups: {
        const order = [];
        const by = {};
        for (const b of binds) {
            if (!by[b.group]) {
                by[b.group] = [];
                order.push(b.group);
            }
            by[b.group].push(b);
        }
        const names = order.filter(g => g !== "Your shortcuts");
        return [{ name: "Your shortcuts", binds: by["Your shortcuts"] ?? [] }].concat(names.map(g => ({ name: g, binds: by[g] })));
    }
    readonly property int changedCount: binds.filter(b => b.changed).length

    // Gestures: { id, fingers, direction, label, action, custom, disabled,
    //   changed, isDefault, text ("4 fingers up"), actionText }
    readonly property var gestures: {
        revision;
        const out = [];
        const changes = user.gestures?.changes ?? {};
        for (const d of gestureDefaults) {
            const c = changes[d.id] ?? {};
            const g = {
                id: d.id, fingers: c.fingers ?? d.fingers, direction: c.direction ?? d.direction,
                action: c.action ?? d.action, label: d.label ?? d.id, custom: false, disabled: !!c.disabled
            };
            g.changed = g.disabled || g.fingers !== d.fingers || g.direction !== d.direction || !sameAction(g.action, d.action);
            g.isDefault = !g.changed;
            out.push(decorateGesture(g));
        }
        for (const x of user.gestures?.custom ?? []) {
            if (!x.id)
                continue;
            out.push(decorateGesture({
                id: x.id, fingers: x.fingers, direction: x.direction, action: x.action,
                label: x.label || describeGesture(x.action), custom: true, disabled: !!x.disabled, changed: false, isDefault: false
            }));
        }
        return out;
    }
    function decorateGesture(g) {
        g.text = g.fingers + " fingers, " + directionLabel(g.direction).toLowerCase();
        g.actionText = describeGesture(g.action);
        return g;
    }

    function bind(id) {
        return binds.find(b => b.id === id) ?? null;
    }
    function gesture(id) {
        return gestures.find(g => g.id === id) ?? null;
    }

    // Binds (turned on) that use these keys, except `exceptId`
    function conflicts(keys, exceptId) {
        if (!keys)
            return [];
        const c = canonical(keys);
        return binds.filter(b => !b.disabled && b.id !== exceptId && canonical(b.keys) === c);
    }

    // Directions that overlap: "swipe" covers every swipe, "horizontal" left
    // and right, "pinch" both pinches
    function overlaps(a, b) {
        if (a === b)
            return true;
        const cover = { swipe: ["horizontal", "vertical", "left", "right", "up", "down"], horizontal: ["left", "right"], vertical: ["up", "down"], pinch: ["pinchin", "pinchout"] };
        return (cover[a] ?? []).includes(b) || (cover[b] ?? []).includes(a);
    }
    function gestureConflicts(fingers, direction, exceptId) {
        return gestures.filter(g => !g.disabled && g.id !== exceptId && g.fingers === fingers && overlaps(g.direction, direction));
    }

    // ── Search ───────────────────────────────────────────────────────────
    readonly property var modAliases: ({
            super: "SUPER", win: "SUPER", windows: "SUPER", cmd: "SUPER", command: "SUPER", meta: "SUPER", mod: "SUPER", "⌘": "SUPER",
            ctrl: "CTRL", control: "CTRL", ctl: "CTRL", "⌃": "CTRL",
            alt: "ALT", option: "ALT", opt: "ALT", "⌥": "ALT",
            shift: "SHIFT", "⇧": "SHIFT"
        })
    readonly property var keyAliases: ({
            enter: "return", esc: "escape", backspace: "backspace", del: "delete", pgup: "page_up", pgdn: "page_down",
            pageup: "page_up", pagedown: "page_down", prtsc: "print", printscreen: "print", ".": "period", ",": "comma",
            "/": "slash", ";": "semicolon", "'": "apostrophe", "-": "minus", "=": "equal", "[": "bracketleft",
            "]": "bracketright", "`": "grave", "\\": "backslash", "←": "left", "→": "right", "↑": "up", "↓": "down",
            "space": "space", "tab": "tab"
        })
    // Whether a query reads as keys ("super s", "ctrl print", "⇧⌘S")
    function parseQuery(q) {
        let s = q.toLowerCase().trim();
        // Mac symbols run together: split them out
        s = s.replace(/([⌘⌃⌥⇧])/g, " $1 ").replace(/\+/g, " ");
        const words = s.split(/\s+/).filter(w => w !== "");
        if (words.length === 0)
            return null;
        const mods = [];
        const keys = [];
        for (const w of words) {
            if (modAliases[w] !== undefined)
                mods.includes(modAliases[w]) || mods.push(modAliases[w]);
            else
                keys.push(keyAliases[w] ?? w);
        }
        if (mods.length === 0 && !(keys.length === 1 && keys[0].length > 1 && keyNameSet[keys[0]]))
            return null;
        if (keys.length > 1)
            return null;
        return { mods: mods, key: keys[0] ?? "" };
    }
    readonly property var keyNameSet: ({ print: true, escape: true, return: true, tab: true, space: true, backspace: true, delete: true, home: true, end: true, page_up: true, page_down: true, insert: true })
    function keyMatches(b, k) {
        const p = parseKeys(b.keys);
        for (const m of k.mods)
            if (!p.mods.includes(m))
                return false;
        if (k.key === "")
            return true;
        const key = p.key.toLowerCase();
        const label = keyLabel(p.key).toLowerCase();
        return key === k.key || label === k.key || (/^f\d+$/.test(k.key) && key === k.key);
    }
    function matches(b, query) {
        const q = query.trim().toLowerCase();
        if (q === "")
            return true;
        // Keys ("super s") match keys only; words match names
        const k = parseQuery(q);
        if (k && k.mods.length > 0)
            return keyMatches(b, k);
        if (k && keyMatches(b, k))
            return true;
        return b.label.toLowerCase().includes(q) || b.group.toLowerCase().includes(q) || b.actionText.toLowerCase().includes(q);
    }

    // ── Editing keybinds ─────────────────────────────────────────────────
    function isDefaultId(id) {
        return defaults.some(d => d.id === id);
    }
    function change(fn) {
        const u = clone(user);
        fn(u);
        user = u;
        revision++;
        save();
    }

    function setKeys(id, keys) {
        const d = defaults.find(x => x.id === id);
        change(u => {
            if (d) {
                const c = Object.assign({}, u.keybinds[id] ?? {});
                if (canonical(keys) === canonical(d.keys))
                    delete c.keys;
                else
                    c.keys = keys;
                if (Object.keys(c).length === 0)
                    delete u.keybinds[id];
                else
                    u.keybinds[id] = c;
            } else {
                const s = u.shortcuts.find(x => x.id === id);
                if (s)
                    s.keys = keys;
            }
        });
    }
    function disable(id, on) {
        const d = isDefaultId(id);
        change(u => {
            if (d) {
                const c = Object.assign({}, u.keybinds[id] ?? {});
                if (on)
                    c.disabled = true;
                else
                    delete c.disabled;
                if (Object.keys(c).length === 0)
                    delete u.keybinds[id];
                else
                    u.keybinds[id] = c;
            } else {
                const s = u.shortcuts.find(x => x.id === id);
                if (s) {
                    if (on)
                        s.disabled = true;
                    else
                        delete s.disabled;
                }
            }
        });
    }
    function reset(id) {
        change(u => delete u.keybinds[id]);
    }
    // Keys for one bind, turning off the ones that used them
    function setKeysTakingOver(id, keys) {
        const others = conflicts(keys, id).map(b => b.id);
        change(u => {
            for (const o of others)
                applyDisable(u, o, true);
        });
        setKeys(id, keys);
    }
    function applyDisable(u, id, on) {
        if (isDefaultId(id)) {
            const c = Object.assign({}, u.keybinds[id] ?? {});
            if (on)
                c.disabled = true;
            else
                delete c.disabled;
            if (Object.keys(c).length === 0)
                delete u.keybinds[id];
            else
                u.keybinds[id] = c;
        } else {
            const s = u.shortcuts.find(x => x.id === id);
            if (s && on)
                s.disabled = true;
            else if (s)
                delete s.disabled;
        }
    }

    function newId(prefix) {
        return prefix + Date.now().toString(36) + Math.floor(Math.random() * 1296).toString(36);
    }
    function cleanShortcut(e) {
        const s = { id: e.id || newId("custom-"), keys: String(e.keys ?? ""), group: "Custom", label: String(e.label ?? ""), action: e.action };
        if (e.opts && Object.keys(e.opts).length > 0)
            s.opts = e.opts;
        if (e.disabled)
            s.disabled = true;
        return s;
    }
    // Returns the new shortcut's id
    function addShortcut(entry) {
        const s = cleanShortcut(entry);
        change(u => u.shortcuts.push(s));
        return s.id;
    }
    function updateShortcut(id, entry) {
        change(u => {
            const i = u.shortcuts.findIndex(x => x.id === id);
            if (i >= 0)
                u.shortcuts[i] = cleanShortcut(Object.assign({}, u.shortcuts[i], entry, { id: id }));
        });
    }
    function removeShortcut(id) {
        change(u => u.shortcuts = u.shortcuts.filter(x => x.id !== id));
    }

    // ── Editing gestures ─────────────────────────────────────────────────
    // Three-finger drag (Settings → Trackpad), as on macOS: on, three fingers
    // move the window under them and spaces swipe with four (the defaults);
    // off, three fingers swipe between spaces and nothing drags
    readonly property bool threeFingerDrag: {
        const m = gestures.find(g => g.id === "move");
        return !!m && !m.disabled && m.fingers === 3;
    }
    function setThreeFingerDrag(on) {
        change(u => {
            const ch = u.gestures.changes;
            const set = (id, k, v) => {
                const c = Object.assign({}, ch[id] ?? {});
                if (v === undefined)
                    delete c[k];
                else
                    c[k] = v;
                if (Object.keys(c).length === 0)
                    delete ch[id];
                else
                    ch[id] = c;
            };
            set("move", "disabled", on ? undefined : true);
            set("move", "fingers", undefined);
            set("workspaces", "fingers", on ? undefined : 3);
        });
    }

    // patch: any of { fingers, direction, action }
    function setGesture(id, patch) {
        const d = gestureDefaults.find(x => x.id === id);
        change(u => {
            if (d) {
                const c = Object.assign({}, u.gestures.changes[id] ?? {}, patch);
                for (const k of ["fingers", "direction"])
                    if (c[k] === d[k])
                        delete c[k];
                if (c.action !== undefined && sameAction(c.action, d.action))
                    delete c.action;
                if (Object.keys(c).length === 0)
                    delete u.gestures.changes[id];
                else
                    u.gestures.changes[id] = c;
            } else {
                const g = u.gestures.custom.find(x => x.id === id);
                if (g) {
                    Object.assign(g, patch);
                    if (patch.action !== undefined)
                        g.label = describeGesture(patch.action);
                }
            }
        });
    }
    function disableGesture(id, on) {
        const d = gestureDefaults.some(x => x.id === id);
        change(u => {
            if (d) {
                const c = Object.assign({}, u.gestures.changes[id] ?? {});
                if (on)
                    c.disabled = true;
                else
                    delete c.disabled;
                if (Object.keys(c).length === 0)
                    delete u.gestures.changes[id];
                else
                    u.gestures.changes[id] = c;
            } else {
                const g = u.gestures.custom.find(x => x.id === id);
                if (g && on)
                    g.disabled = true;
                else if (g)
                    delete g.disabled;
            }
        });
    }
    function resetGesture(id) {
        change(u => delete u.gestures.changes[id]);
    }
    function addGesture(g) {
        const x = { id: newId("gesture-"), fingers: g.fingers, direction: g.direction, label: describeGesture(g.action), action: g.action };
        change(u => u.gestures.custom.push(x));
        return x.id;
    }
    function removeGesture(id) {
        change(u => u.gestures.custom = u.gestures.custom.filter(x => x.id !== id));
    }

    // ── Undo ─────────────────────────────────────────────────────────────
    // Take one before a change; restore() puts everything back
    function snapshot() {
        return JSON.stringify(user);
    }
    function restore(snap) {
        user = normalizeUser(JSON.parse(snap));
        revision++;
        save();
    }

    // ── Saving ───────────────────────────────────────────────────────────
    // Debounced: several quick edits make one write and one reload
    function save() {
        saveTimer.restart();
    }
    Timer {
        id: saveTimer
        interval: 400
        onTriggered: root.writeNow()
    }
    function writeNow() {
        const text = JSON.stringify(user, null, 1) + "\n";
        saving = true;
        // Temp file, then mv: Hyprland never reads half a file
        run(["sh", "-c", "mkdir -p \"$1\" && t=\"$2.tmp.$$\" && printf '%s' \"$3\" > \"$t\" && mv -f \"$t\" \"$2\" && { hyprctl reload config-only >/dev/null 2>&1 || true; }", "sh", Config.dir, userPath, text], (code, out, err) => {
            root.saving = false;
            root.saveError = code === 0 ? "" : "Couldn't save your shortcuts to " + root.userPath + (err.trim() ? " (" + err.trim() + ")" : "") + ". Check that the folder is writable, then change a shortcut again.";
            // Re-applying window looks after a reload is System.qml's job
            // (configreloaded); the cheatsheet re-reads the live binds
            refreshTimer.restart();
        });
    }
    Timer {
        id: refreshTimer
        interval: 600
        onTriggered: Keybinds.refresh()
    }

    // ── Files and helper processes ───────────────────────────────────────
    Component {
        id: jobComponent
        Process {
            id: job
            property var callback: null
            property int code: -1
            property bool exited: false
            property bool outDone: false
            property bool errDone: false
            function finish() {
                if (!exited || !outDone || !errDone)
                    return;
                const cb = callback;
                if (!cb)
                    return;
                callback = null;
                job.grace.stop();
                cb(code, out.text ?? "", err.text ?? "");
                job.destroy();
            }
            stdout: StdioCollector {
                id: out
                onStreamFinished: { job.outDone = true; job.finish(); }
            }
            stderr: StdioCollector {
                id: err
                onStreamFinished: { job.errDone = true; job.finish(); }
            }
            onExited: c => {
                job.code = c;
                job.exited = true;
                job.grace.start();
                job.finish();
            }
            // In case a stream never reports that it finished (a Process
            // takes no children, so the timer lives in a property)
            property Timer grace: Timer {
                interval: 80
                onTriggered: {
                    job.outDone = true;
                    job.errDone = true;
                    job.finish();
                }
            }
        }
    }
    // Runs a command; cb(exitCode, stdout, stderr)
    function run(cmd, cb) {
        const p = jobComponent.createObject(root, { command: cmd, callback: cb });
        p.running = true;
    }
    function expand(path) {
        const p = String(path ?? "").trim();
        return p === "~" ? home : p.startsWith("~/") ? home + p.slice(1) : p;
    }
    function writeFile(path, text, cb) {
        const p = expand(path);
        run(["sh", "-c", "mkdir -p \"$(dirname \"$1\")\" && t=\"$1.tmp.$$\" && printf '%s' \"$2\" > \"$t\" && mv -f \"$t\" \"$1\"", "sh", p, text], (code, o, e) => cb(code === 0, e.trim()));
    }
    function readFile(path, cb) {
        const p = expand(path);
        run(["cat", "--", p], (code, o, e) => cb(code === 0, o, e.trim()));
    }

    FileView {
        id: defaultsFile
        path: root.hyprDir !== "" ? root.hyprDir + "/keymap.json" : ""
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const d = JSON.parse(text());
                root.defaults = Array.isArray(d) ? d.filter(x => x && x.id && x.keys) : [];
                root.defaultsError = Array.isArray(d) ? "" : "iRice's default shortcuts (" + path + ") aren't a list. Run tools/gen-keymap.py again, or reinstall iRice.";
            } catch (e) {
                root.defaults = [];
                root.defaultsError = "iRice's default shortcuts (" + path + ") can't be read: " + e.message + ". Run tools/gen-keymap.py again, or reinstall iRice.";
            }
            root.defaultsLoaded = true;
            root.revision++;
        }
        onLoadFailed: error => {
            root.defaults = [];
            root.defaultsError = "iRice's default shortcuts weren't found at " + path + ". Reinstall iRice (./install.sh), or set IRICE_ROOT to the repo when running it from there.";
            root.defaultsLoaded = true;
        }
    }
    FileView {
        id: gesturesFile
        path: root.hyprDir !== "" ? root.hyprDir + "/gestures.json" : ""
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const d = JSON.parse(text());
                root.gestureDefaults = Array.isArray(d) ? d.filter(x => x && x.id) : [];
            } catch (e) {
                root.gestureDefaults = [];
            }
            root.revision++;
        }
        onLoadFailed: root.gestureDefaults = []
    }
    FileView {
        id: userFile
        path: root.userPath
        watchChanges: true
        printErrors: false // no file yet just means no changes
        // Our own write comes back as a change: ignore it while edits wait
        onFileChanged: if (!saveTimer.running && !root.saving) reload()
        onLoaded: {
            const t = text();
            if (t.trim() === "") {
                root.user = root.blankUser();
                root.userError = "";
            } else {
                try {
                    root.user = root.normalizeUser(JSON.parse(t));
                    root.userError = "";
                } catch (e) {
                    // Keep what we had; never overwrite a file the user is fixing
                    root.userError = "Your shortcuts file (" + path + ") isn't valid JSON: " + e.message + ". Fix it in a text editor, or delete it to go back to the defaults. Changes here won't be saved until then.";
                }
            }
            root.userLoaded = true;
            root.revision++;
        }
        onLoadFailed: error => {
            if (error === FileViewError.FileNotFound) {
                root.user = root.blankUser();
                root.userError = "";
            } else {
                root.userError = "Your shortcuts file (" + path + ") can't be read. Check its permissions.";
            }
            root.userLoaded = true;
            root.revision++;
        }
    }
    // A broken file stays as it is until the user fixes it
    onUserErrorChanged: if (userError !== "") saveTimer.stop()

    Process {
        running: true
        command: ["sh", "-c", "for c in zenity kdialog gtk-launch; do command -v $c >/dev/null && echo $c; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                const t = text.split("\n");
                root.hasZenity = t.includes("zenity");
                root.hasKdialog = t.includes("kdialog");
                root.hasGtkLaunch = t.includes("gtk-launch");
            }
        }
    }

    // An app from its desktop entry, as a bind action
    function appAction(entry) {
        if (hasGtkLaunch)
            return { exec: "gtk-launch " + entry.id, desktop: entry.id };
        const q = s => /^[\w@%+=:,./-]+$/.test(s) ? s : "'" + String(s).replace(/'/g, "'\\''") + "'";
        return { exec: (entry.command ?? []).map(q).join(" "), desktop: entry.id };
    }

    // ── Export ───────────────────────────────────────────────────────────
    // sections: any of "keybinds", "shortcuts", "gestures"
    function exportData(sections) {
        const out = { "irice-keybinds": 1 };
        if (sections.includes("keybinds"))
            out.keybinds = clone(user.keybinds);
        if (sections.includes("shortcuts"))
            out.shortcuts = clone(user.shortcuts);
        if (sections.includes("gestures"))
            out.gestures = clone(user.gestures);
        return out;
    }
    function exportTo(path, sections, cb) {
        writeFile(path, JSON.stringify(exportData(sections), null, 1) + "\n", cb);
    }

    // ── Import ───────────────────────────────────────────────────────────
    // A preview: { title, summary, lines: [{ text, kind }], skipped: [...],
    //   result (the user file after applying), changes (count) }
    function previewFile(text, sections) {
        let data;
        try {
            data = JSON.parse(text);
        } catch (e) {
            return { error: "That file isn't valid JSON (" + e.message + "). Export it again from iRice, or fix it in a text editor." };
        }
        if (!data || typeof data !== "object" || data["irice-keybinds"] === undefined)
            return { error: "That isn't an iRice shortcuts file. Choose a file made with Export here." };
        const has = { keybinds: data.keybinds !== undefined, shortcuts: data.shortcuts !== undefined, gestures: data.gestures !== undefined };
        const wanted = sections.filter(s => has[s]);
        if (wanted.length === 0) {
            const names = { keybinds: "changes to default shortcuts", shortcuts: "shortcuts of your own", gestures: "gestures" };
            return { error: "That file has no " + sections.map(s => names[s]).join(" or ") + ". Choose a file exported from the same section, or one made with Export all." };
        }
        const u = clone(user);
        const lines = [];
        const counts = { changed: 0, added: 0, unknown: 0, gestures: 0 };
        if (wanted.includes("keybinds") && typeof data.keybinds === "object") {
            for (const id in data.keybinds) {
                const d = defaults.find(x => x.id === id);
                const c = data.keybinds[id];
                if (!d || !c || typeof c !== "object") {
                    counts.unknown++;
                    continue;
                }
                const clean = {};
                if (typeof c.keys === "string" && canonical(c.keys) !== canonical(d.keys))
                    clean.keys = c.keys;
                if (c.disabled)
                    clean.disabled = true;
                if (JSON.stringify(u.keybinds[id] ?? {}) === JSON.stringify(clean))
                    continue;
                if (Object.keys(clean).length === 0)
                    delete u.keybinds[id];
                else
                    u.keybinds[id] = clean;
                counts.changed++;
                lines.push({ kind: "changed", text: d.label + ": " + (clean.disabled ? "off" : keyText(clean.keys ?? d.keys)) });
            }
        }
        if (wanted.includes("shortcuts") && Array.isArray(data.shortcuts)) {
            for (const s of data.shortcuts) {
                if (!s || typeof s.keys !== "string" || !s.action || typeof s.action !== "object") {
                    counts.unknown++;
                    continue;
                }
                const i = s.id ? u.shortcuts.findIndex(x => x.id === s.id) : -1;
                const dup = u.shortcuts.some(x => canonical(x.keys) === canonical(s.keys) && sameAction(x.action, s.action));
                if (i >= 0) {
                    const next = cleanShortcut(s);
                    if (JSON.stringify(u.shortcuts[i]) === JSON.stringify(next))
                        continue;
                    u.shortcuts[i] = next;
                    counts.changed++;
                    lines.push({ kind: "changed", text: (next.label || describe(next.action)) + ": " + keyText(next.keys) });
                } else if (!dup) {
                    const next = cleanShortcut(s);
                    u.shortcuts.push(next);
                    counts.added++;
                    lines.push({ kind: "added", text: (next.label || describe(next.action)) + ": " + keyText(next.keys) });
                }
            }
        }
        if (wanted.includes("gestures") && data.gestures && typeof data.gestures === "object") {
            const g = data.gestures;
            for (const id in (g.changes ?? {})) {
                if (!gestureDefaults.some(x => x.id === id)) {
                    counts.unknown++;
                    continue;
                }
                if (JSON.stringify(u.gestures.changes[id] ?? {}) === JSON.stringify(g.changes[id]))
                    continue;
                u.gestures.changes[id] = g.changes[id];
                counts.gestures++;
                lines.push({ kind: "changed", text: "Gesture: " + (gestureDefaults.find(x => x.id === id)?.label ?? id) });
            }
            for (const x of (Array.isArray(g.custom) ? g.custom : [])) {
                if (!x || !x.fingers || !x.direction || x.action === undefined) {
                    counts.unknown++;
                    continue;
                }
                const i = x.id ? u.gestures.custom.findIndex(y => y.id === x.id) : -1;
                const next = { id: x.id || newId("gesture-"), fingers: x.fingers, direction: x.direction, label: x.label || describeGesture(x.action), action: x.action };
                if (i >= 0) {
                    if (JSON.stringify(u.gestures.custom[i]) === JSON.stringify(next))
                        continue;
                    u.gestures.custom[i] = next;
                } else
                    u.gestures.custom.push(next);
                counts.gestures++;
                lines.push({ kind: "added", text: "Gesture: " + next.fingers + " fingers, " + directionLabel(next.direction).toLowerCase() + " → " + describeGesture(next.action) });
            }
        }
        const parts = [];
        if (counts.changed)
            parts.push(counts.changed + (counts.changed === 1 ? " shortcut changed" : " shortcuts changed"));
        if (counts.added)
            parts.push(counts.added + " added");
        if (counts.gestures)
            parts.push(counts.gestures + (counts.gestures === 1 ? " gesture" : " gestures"));
        if (counts.unknown)
            parts.push(counts.unknown + " skipped (not in this version of iRice)");
        const total = counts.changed + counts.added + counts.gestures;
        return {
            title: "Import from file",
            summary: total === 0 ? "Nothing to change: this file matches what you have." : parts.join(", "),
            lines: lines, skipped: [], result: u, total: total
        };
    }

    // end4: the helper parses its Lua files; this matches them to iRice's binds.
    // cb(preview) — preview.error says what went wrong.
    function previewEnd4(withBinds, withGestures, cb) {
        const h = home + "/.config/hypr";
        const bindFiles = withBinds ? [h + "/hyprland/keybinds.lua", h + "/custom/keybinds.lua"] : [];
        const gestureFiles = withGestures ? [h + "/hyprland/general.lua", h + "/custom/general.lua"] : [];
        run(["python3", "-I", scriptsDir + "/import-end4.py"].concat(bindFiles, ["--"], gestureFiles), (code, out, err) => {
            if (code !== 0) {
                cb({ error: code === 127 ? "Python 3 is needed to read end4's files. Install python, then try again." : "Couldn't read end4's files (" + (err.trim().split("\n").pop() || "exit " + code) + ")." });
                return;
            }
            let parsed;
            try {
                parsed = JSON.parse(out);
            } catch (e) {
                cb({ error: "Couldn't read end4's files: the helper's output was cut off. Try again." });
                return;
            }
            if (parsed.files.length === 0) {
                cb({ error: "No end4 config was found in ~/.config/hypr/hyprland/. Install end4's dotfiles, or import an iRice file instead." });
                return;
            }
            cb(root.matchEnd4(parsed));
        });
    }

    function matchEnd4(parsed) {
        const u = clone(user);
        const lines = [];
        const current = binds;
        // How many binds each action has, on each side
        const end4By = {};
        for (const e of parsed.binds) {
            const k = actionKey(e.action);
            (end4By[k] = end4By[k] ?? []).push(e);
        }
        const iriceBy = {};
        for (const b of current.filter(b => !b.custom)) {
            const k = actionKey(b.action);
            (iriceBy[k] = iriceBy[k] ?? []).push(b);
        }
        // Keys end4 gives to each action, to see which iRice binds it displaced
        const end4Keys = {};
        for (const e of parsed.binds)
            end4Keys[canonical(e.keys)] = actionKey(e.action);

        const touched = {}; // ids this import sets
        const claimed = []; // canonical keys this import takes
        let changed = 0, added = 0, same = 0, off = 0;
        for (const k in end4By) {
            const ends = end4By[k];
            const mine = (iriceBy[k] ?? []).slice();
            const left = [];
            for (const e of ends) {
                const i = mine.findIndex(b => canonical(b.keys) === canonical(e.keys) && !b.disabled);
                if (i >= 0) {
                    touched[mine[i].id] = true;
                    claimed.push(canonical(e.keys));
                    mine.splice(i, 1);
                    same++;
                } else
                    left.push(e);
            }
            for (const e of left) {
                // Move an iRice default when it's unambiguous (one bind each
                // side) or end4 gave its keys to something else
                let i = -1;
                if (ends.length === 1 && (iriceBy[k] ?? []).length === 1 && mine.length === 1)
                    i = 0;
                else
                    i = mine.findIndex(b => end4Keys[canonical(b.keys)] !== undefined && end4Keys[canonical(b.keys)] !== k);
                const label = e.label || (e.action.shell !== undefined ? shellLabel(e.action.shell) : describe(e.action));
                if (i >= 0) {
                    const b = mine[i];
                    mine.splice(i, 1);
                    const c = Object.assign({}, u.keybinds[b.id] ?? {});
                    delete c.disabled;
                    if (canonical(e.keys) === canonical(b.defaultKeys))
                        delete c.keys;
                    else
                        c.keys = e.keys;
                    if (Object.keys(c).length === 0)
                        delete u.keybinds[b.id];
                    else
                        u.keybinds[b.id] = c;
                    touched[b.id] = true;
                    claimed.push(canonical(e.keys));
                    changed++;
                    lines.push({ kind: "changed", text: b.label + ": " + keyText(b.keys) + " → " + keyText(e.keys) });
                } else {
                    // Already one of yours?
                    if (u.shortcuts.some(s => canonical(s.keys) === canonical(e.keys) && sameAction(s.action, e.action))) {
                        same++;
                        claimed.push(canonical(e.keys));
                        continue;
                    }
                    const s = cleanShortcut({ keys: e.keys, label: label, action: e.action, opts: e.opts });
                    u.shortcuts.push(s);
                    touched[s.id] = true;
                    claimed.push(canonical(e.keys));
                    added++;
                    lines.push({ kind: "added", text: label + ": " + keyText(e.keys) });
                }
            }
        }
        // end4 wins a clash: iRice binds left on keys end4 now uses turn off
        for (const b of current) {
            if (touched[b.id] || b.disabled)
                continue;
            const k = u.keybinds[b.id]?.keys ?? (b.custom ? b.keys : b.defaultKeys);
            if (claimed.includes(canonical(k))) {
                applyDisable(u, b.id, true);
                off++;
                lines.push({ kind: "off", text: b.label + " (" + keyText(k) + ") turns off: end4 uses those keys" });
            }
        }

        // Gestures: same fingers and direction → that gesture's action
        let gChanged = 0, gAdded = 0;
        const gNow = gestures;
        for (const g of parsed.gestures) {
            const at = gNow.find(x => x.fingers === g.fingers && x.direction === g.direction);
            if (at && sameAction(at.action, g.action) && !at.disabled)
                continue;
            if (at && !at.custom) {
                const d = gestureDefaults.find(x => x.id === at.id);
                const c = Object.assign({}, u.gestures.changes[at.id] ?? {});
                delete c.disabled;
                if (sameAction(d.action, g.action))
                    delete c.action;
                else
                    c.action = g.action;
                if (Object.keys(c).length === 0)
                    delete u.gestures.changes[at.id];
                else
                    u.gestures.changes[at.id] = c;
                gChanged++;
                lines.push({ kind: "changed", text: "Gesture " + at.text + ": " + describeGesture(g.action) });
            } else if (!at) {
                u.gestures.custom.push({ id: newId("gesture-"), fingers: g.fingers, direction: g.direction, label: describeGesture(g.action), action: g.action });
                gAdded++;
                lines.push({ kind: "added", text: "Gesture " + g.fingers + " fingers, " + directionLabel(g.direction).toLowerCase() + ": " + describeGesture(g.action) });
            }
        }

        // Skipped, without end4's fallbacks (they only run without its shell)
        const seen = {};
        const skipped = [];
        for (const s of parsed.skipped) {
            if (s.fallback)
                continue;
            const key = canonical(s.keys) + "|" + s.reason;
            if (seen[key])
                continue;
            seen[key] = true;
            skipped.push({ text: (s.label ? s.label + " " : "") + "(" + keyText(s.keys) + "): " + s.reason });
        }
        for (const s of parsed.gesturesSkipped)
            skipped.push({ text: "Gesture " + s.fingers + " fingers " + s.direction + ": " + s.reason });

        const parts = [];
        if (changed + same + added + off > 0 || parsed.binds.length > 0)
            parts.push(changed + (changed === 1 ? " key changed" : " keys changed"), added + " added");
        if (off)
            parts.push(off + " turned off");
        if (gChanged + gAdded)
            parts.push((gChanged + gAdded) + ((gChanged + gAdded) === 1 ? " gesture" : " gestures"));
        parts.push(skipped.length + " skipped");
        const total = changed + added + off + gChanged + gAdded;
        return {
            title: "Import from end4",
            summary: "Import from end4: " + parts.join(", ") + (total === 0 ? ". " + same + " already match." : ""),
            lines: lines, skipped: skipped, result: u, total: total,
            note: parsed.ignored > 0 ? parsed.ignored + " binds inside loops (workspaces 1–10) are left out: iRice's defaults have them." : ""
        };
    }

    // Applies a preview's result
    function applyPreview(p) {
        if (!p || !p.result)
            return;
        user = normalizeUser(p.result);
        revision++;
        save();
    }
}
