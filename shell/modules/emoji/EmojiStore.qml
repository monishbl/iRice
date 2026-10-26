pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Data and actions for the emoji picker: the emoji, kaomoji and symbol lists
// (assets/data/emoji.json, from tools/gen-emoji-data.py), search, what you
// use most, skin tones per emoji, and putting a pick into the app you were in.
// The data loads on first open and stays (about 160 KB of JSON).
// Usage and tones live in $XDG_STATE_HOME/irice/emoji.json, apart from
// UiState, so this module owns its file.
Singleton {
    id: root

    // Settings page with the KLIPY key field (Panels.openSettings)
    readonly property string settingsPage: "sidebar"

    // ---- Data -----------------------------------------------------------

    property bool loaded: false
    property bool failed: false
    property var groups: [] // [{ id, title }]
    property var emoji: [] // [{ c, n, k, s, g, t, hay }] (t: 0 | 1 | [5])
    property var kaomoji: [] // [{ title, items: [{ c, n, hay }] }]
    property var symbols: [] // [{ title, items: [{ c, n, hay }] }]
    property var byChar: ({}) // char -> emoji entry
    property var emojiByGroup: [] // group index -> its emoji, in order
    property var kaomojiFlat: []
    property var symbolsFlat: []

    // Set on first open; the FileView then reads the file (off the UI thread)
    property bool wanted: false
    function load() {
        if (failed && wanted)
            dataFile.reload();
        wanted = true;
    }

    FileView {
        id: dataFile
        path: root.wanted ? decodeURIComponent(Qt.resolvedUrl("../../assets/data/emoji.json").toString().replace("file://", "")) : ""
        onLoaded: {
            try {
                const j = JSON.parse(text());
                root.groups = j.groups.map(g => ({ id: g[0], title: g[1] }));
                const map = {};
                // Search text gets a leading space and single spaces, so
                // "starts a word" is one includes(" " + word)
                const words = t => " " + t.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim();
                root.emoji = j.emoji.map((e, i) => {
                    const o = { c: e[0], n: e[1], k: e[2], s: e[3] || e[1].toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, ""), g: e[4], t: e[5], i: i };
                    o.nl = o.n.toLowerCase();
                    o.hay = words(o.n + " " + o.k + " " + o.s);
                    map[o.c] = o;
                    return o;
                });
                root.byChar = map;
                const byGroup = root.groups.map(() => []);
                for (const o of root.emoji)
                    if (byGroup[o.g])
                        byGroup[o.g].push(o);
                root.emojiByGroup = byGroup;
                const section = s => ({ title: s[0], items: s[1].map(i => ({ c: i[0], n: i[1], nl: i[1].toLowerCase(), hay: " " + i[0].toLowerCase() + words(i[1]) })) });
                root.kaomoji = j.kaomoji.map(section);
                root.symbols = j.symbols.map(section);
                root.kaomojiFlat = [].concat(...root.kaomoji.map(x => x.items));
                root.symbolsFlat = [].concat(...root.symbols.map(x => x.items));
                root.loaded = true;
                root.failed = false;
            } catch (e) {
                console.warn("emoji: can't read emoji.json:", e);
                root.failed = true;
            }
        }
        onLoadFailed: if (root.wanted) root.failed = true
    }

    // ---- Remembered: usage and skin tones --------------------------------

    property var usage: ({}) // text -> { k: "emoji" | "kaomoji" | "symbol" | "gif", n: count, t: last used (ms) }
    property var tones: ({}) // base emoji -> 1..5 (0 / missing = default yellow)
    property string customerId: "" // stable random id for KLIPY (its API asks for one per user)
    property bool stateLoaded: false
    readonly property string statePath: UiState.dir + "/emoji.json"

    FileView {
        id: stateFile
        path: root.statePath
        onLoaded: {
            try {
                const j = JSON.parse(text());
                if (j.usage && typeof j.usage === "object")
                    root.usage = j.usage;
                if (j.tones && typeof j.tones === "object")
                    root.tones = j.tones;
                if (typeof j.customerId === "string")
                    root.customerId = j.customerId;
            } catch (e) {}
            root.stateLoaded = true;
            root.ensureCustomerId();
        }
        onLoadFailed: {
            root.stateLoaded = true;
            root.ensureCustomerId();
        }
    }
    function ensureCustomerId() {
        if (customerId !== "")
            return;
        let id = "";
        for (let i = 0; i < 32; i++)
            id += "0123456789abcdef"[Math.floor(Math.random() * 16)];
        customerId = id;
        save();
    }
    Timer {
        id: saveTimer
        interval: 400
        onTriggered: stateFile.setText(JSON.stringify({ usage: root.usage, tones: root.tones, customerId: root.customerId }))
    }
    function save() {
        if (stateLoaded)
            saveTimer.restart();
    }

    // Most used first, weighted toward recent use (a week halves the weight)
    function frequent(kind, max) {
        const now = Date.now();
        return Object.keys(usage).filter(c => usage[c].k === kind).map(c => ({ c: c, w: usage[c].n * Math.pow(0.5, (now - usage[c].t) / 604800000) })).sort((a, b) => b.w - a.w).slice(0, max).map(x => x.c);
    }
    function recordUse(text, kind) {
        const u = Object.assign({}, usage);
        const prev = u[text] ?? { n: 0 };
        u[text] = { k: kind, n: Math.min(999, prev.n + 1), t: Date.now() };
        // Keep the file small: the 200 most recent
        const keys = Object.keys(u);
        if (keys.length > 200)
            keys.sort((a, b) => u[a].t - u[b].t).slice(0, keys.length - 200).forEach(k => delete u[k]);
        usage = u;
        save();
    }

    // ---- Skin tones ------------------------------------------------------

    readonly property var toneModifiers: ["\u{1F3FB}", "\u{1F3FC}", "\u{1F3FD}", "\u{1F3FE}", "\u{1F3FF}"]
    readonly property var toneNames: ["Default", "Light", "Medium-light", "Medium", "Medium-dark", "Dark"]
    function supportsTone(e) {
        return !!e && e.t !== 0 && e.t !== undefined;
    }
    // The emoji with tone 1..5 (0 = as is)
    function withTone(e, tone) {
        if (!supportsTone(e) || !tone)
            return e.c;
        if (Array.isArray(e.t))
            return e.t[tone - 1];
        // The modifier goes after the first character, replacing the
        // emoji-style selector (U+FE0F) if there is one. (By code point:
        // Array.from splits a string into UTF-16 halves in Qt's engine,
        // which broke every toned emoji into "�".)
        const first = e.c.codePointAt(0) > 0xFFFF ? 2 : 1;
        let rest = e.c.slice(first);
        if (rest.charAt(0) === "\uFE0F")
            rest = rest.slice(1);
        return e.c.slice(0, first) + toneModifiers[tone - 1] + rest;
    }
    // "U+1D538 U+0020": by code point (Array.from splits UTF-16 halves here)
    function codePoints(str) {
        const out = [];
        for (let i = 0; i < str.length;) {
            const cp = str.codePointAt(i);
            out.push("U+" + cp.toString(16).toUpperCase().padStart(4, "0"));
            i += cp > 0xFFFF ? 2 : 1;
        }
        return out.join(" ");
    }
    function toneOf(e) {
        return e ? (tones[e.c] ?? 0) : 0;
    }
    function display(e) {
        return withTone(e, toneOf(e));
    }
    function setTone(e, tone) {
        const t = Object.assign({}, tones);
        if (tone)
            t[e.c] = tone;
        else
            delete t[e.c];
        tones = t;
        save();
    }

    // ---- Search ----------------------------------------------------------

    // Lower is better; -1 = no match. Every word of the query has to start a
    // word in the name, keywords or shortcode. One pass, no regexes or
    // per-entry objects: it runs on every keystroke over ~2,800 entries.
    function score(e, words, q) {
        const hay = e.hay;
        for (let w = 0; w < words.length; w++)
            if (hay.indexOf(" " + words[w]) < 0)
                return -1;
        const n = e.nl;
        if (n === q)
            return 0;
        if (n.startsWith(q))
            return 1 + n.length / 100;
        if (n.includes(q))
            return 2 + n.length / 100;
        return 3 + n.length / 100;
    }
    function rank(list, words, q, max) {
        const hits = [];
        for (let i = 0; i < list.length; i++) {
            const sc = score(list[i], words, q);
            if (sc >= 0)
                hits.push([sc, i]);
        }
        hits.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
        const out = [];
        for (let i = 0; i < hits.length && i < max; i++)
            out.push(list[hits[i][1]]);
        return out;
    }
    function search(query, max) {
        const q = query.trim().toLowerCase().replace(/^:+|:+$/g, "").replace(/[_\s]+/g, " ").trim();
        if (q === "")
            return { emoji: [], kaomoji: [], symbols: [] };
        const words = q.split(" ").map(w => w.replace(/[^a-z0-9]+/g, "")).filter(w => w !== "");
        if (words.length === 0) // symbols only: match the characters themselves
            return { emoji: emoji.filter(e => e.c.includes(q)).slice(0, max), kaomoji: kaomojiFlat.filter(e => e.c.includes(q)).slice(0, max), symbols: symbolsFlat.filter(e => e.c.includes(q)).slice(0, max) };
        return { emoji: rank(emoji, words, q, max), kaomoji: rank(kaomojiFlat, words, q, max), symbols: rank(symbolsFlat, words, q, max) };
    }

    // ---- Placement -------------------------------------------------------

    // Where to open, in global layout coordinates. Wayland doesn't tell us
    // where the text cursor is, so: near the pointer when it's over the
    // focused window, else centred in that window, else centred on screen.
    property var pointer: null // { x, y }
    property var window: null // { x, y, w, h, address }
    property bool placed: false
    function prepare() {
        placed = false;
        pointer = null;
        window = null;
        load();
        placeProc.running = false;
        placeProc.running = true;
    }
    Process {
        id: placeProc
        command: ["sh", "-c", "hyprctl -j cursorpos; printf '\\n\\036\\n'; hyprctl -j activewindow"]
        stdout: StdioCollector {
            onStreamFinished: {
                const parts = text.split("\x1e");
                try {
                    const p = JSON.parse(parts[0]);
                    if (typeof p.x === "number")
                        root.pointer = { x: p.x, y: p.y };
                } catch (e) {}
                try {
                    const w = JSON.parse(parts[1] ?? "");
                    if (w && w.at && w.size && w.address)
                        root.window = { x: w.at[0], y: w.at[1], w: w.size[0], h: w.size[1], address: w.address };
                } catch (e) {}
                root.placed = true;
            }
        }
    }

    // ---- Putting a pick into the app -------------------------------------

    // Copies text; unless copyOnly, also types it into the window that had
    // focus. The panel closes first so focus can go back to that window.
    // `key`: what to count as used (an emoji without its skin tone)
    function pick(text, kind, copyOnly, key) {
        if (!text)
            return;
        recordUse(key || text, kind);
        const target = window?.address ?? "";
        Panels.close();
        Quickshell.execDetached(["sh", "-c", "printf '%s' \"$1\" | wl-copy", "sh", text]);
        if (copyOnly)
            return;
        typer.text = text;
        typer.address = target;
        refocusTimer.restart();
    }
    QtObject {
        id: typer
        property string text
        property string address
    }
    Timer {
        id: refocusTimer
        interval: 140 // the layer gives up keyboard focus first
        onTriggered: {
            if (typer.address !== "")
                Hyprland.dispatch("hl.dsp.focus({ window = \"address:0x" + typer.address.replace(/^0x/, "") + "\" })");
            typeTimer.restart();
        }
    }
    Timer {
        id: typeTimer
        interval: 60
        onTriggered: Quickshell.execDetached(["sh", "-c", "printf '%s' \"$1\" | wtype -", "sh", typer.text])
    }
}
