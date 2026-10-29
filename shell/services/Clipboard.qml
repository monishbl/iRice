pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Clipboard history (cliphist) with image previews and pinned items. Pins
// survive restarts in UiState; pinned images are copied into the state dir so
// they outlive cliphist's history.
// When each item was copied: cliphist keeps no times, so the shell watches
// the clipboard (wl-paste --watch) and stamps every new entry as it appears,
// in clip-times.json next to UiState. Entries from before that have no time
// and are grouped as "Earlier".
Singleton {
    id: root

    // [{ id, text, image }] newest first; image = preview path for binary entries
    property var items: []
    readonly property var pins: UiState.data.clipPins // [{ text } | { image }]
    readonly property string previewDir: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-clip"
    readonly property string pinDir: UiState.dir + "/clip-pins"

    function refresh() {
        if (listProc.running)
            again = true; // a copy landed mid-list: look once more after
        else
            listProc.running = true;
    }
    property bool again: false

    // ── When things were copied ─────────────────────────────────────────
    property var stamps: ({}) // cliphist id -> ms since epoch (0: from before)
    // The history the session's watcher (wl-paste --watch cliphist store)
    // writes: your usual ~/.cache/cliphist/db. Named on every call, because
    // the iRice session gives the shell a cache folder of its own and a
    // restart (an update) could start it without the hint pointing back here
    readonly property string db: Quickshell.env("CLIPHIST_DB_PATH") || (Quickshell.env("HOME") + "/.cache/cliphist/db")
    property bool stampsLoaded: false
    property bool stampsExisted: false
    FileView {
        id: stampFile
        path: UiState.dir + "/clip-times.json"
        blockLoading: false
        printErrors: false
        onLoaded: {
            try {
                root.stamps = JSON.parse(text()) ?? {};
                root.stampsExisted = true;
            } catch (e) {}
            root.stampsLoaded = true;
            root.refresh(); // a list read before the times were in gets its times now
        }
        onLoadFailed: { // (first run: no file yet)
            root.stampsLoaded = true;
            root.refresh();
        }
    }
    Timer {
        id: saveStamps
        interval: 1000
        onTriggered: stampFile.setText(JSON.stringify(root.stamps))
    }
    // A new copy: list again shortly (cliphist stores it first)
    Process {
        running: Tools.has("wl-copy") && Tools.has("cliphist")
        command: [Tools.watcher, "wl-paste", "--watch", "echo", "copied"]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: () => copiedTimer.restart()
        }
    }
    Timer {
        id: copiedTimer
        interval: 500
        onTriggered: root.refresh()
    }
    Component.onCompleted: Qt.callLater(refresh)
    // Stamp the ids not seen before (now; on the very first run, all of them
    // as "from before"), and forget the ones cliphist dropped
    function stampRows(rows) {
        if (!stampsLoaded)
            return;
        const now = Date.now();
        const fresh = !stampsExisted && Object.keys(stamps).length === 0;
        const next = {};
        let changed = false;
        for (const r of rows) {
            if (stamps[r.id] !== undefined)
                next[r.id] = stamps[r.id];
            else {
                next[r.id] = fresh ? 0 : now;
                changed = true;
            }
        }
        if (changed || Object.keys(next).length !== Object.keys(stamps).length) {
            stamps = next;
            stampsExisted = true;
            saveStamps.restart();
        }
    }
    // Section headings: "Just now", "Earlier this hour", "2 hours ago",
    // "Yesterday", "Monday", "12 Oct"; "Earlier" for untimed items
    function sectionFor(ts, now) {
        if (!ts)
            return "Earlier";
        const d = new Date(ts), n = new Date(now);
        const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
        const days = Math.round((day(n) - day(d)) / 86400000);
        if (days <= 0) {
            const mins = (now - ts) / 60000;
            if (mins < 5)
                return "Just now";
            const hours = Math.floor(mins / 60);
            return hours === 0 ? "Earlier this hour" : hours === 1 ? "1 hour ago" : hours + " hours ago";
        }
        if (days === 1)
            return "Yesterday";
        if (days < 7)
            return d.toLocaleDateString(Qt.locale(), "dddd");
        return d.toLocaleDateString(Qt.locale(), d.getFullYear() === n.getFullYear() ? "d MMM" : "d MMM yyyy");
    }

    Process {
        id: listProc
        command: ["cliphist", "-db-path", root.db, "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                const rows = text.split("\n").filter(l => l.includes("\t")).slice(0, 200).map(l => {
                    const tab = l.indexOf("\t");
                    const t = l.slice(tab + 1);
                    const m = t.match(/^\[\[ binary data .* (png|jpe?g|webp|gif|bmp) /i);
                    return { id: l.slice(0, tab), text: t, image: m ? root.previewDir + "/" + l.slice(0, tab) + "." + m[1].toLowerCase() : "" };
                });
                root.stampRows(rows);
                if (JSON.stringify(root.items) !== JSON.stringify(rows))
                    root.items = rows;
                root.decodePreviews(rows);
            }
        }
    }
    // Stable URLs let Qt reuse decoded images when switching filters or reopening.
    // Publish each path only after an atomic decode; never load incomplete files.
    property var previewReady: ({})
    property var pendingPreviews: []
    function decodePreviews(rows) {
        pendingPreviews = rows.filter(r => r.image);
        if (!previewProc.running)
            startPreviews();
    }
    function startPreviews() {
        const want = pendingPreviews;
        pendingPreviews = [];
        if (!want.length)
            return;
        previewProc.command = ["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/clipboard-previews.py").toString().replace("file://", "")), root.db, root.previewDir].concat(...want.map(r => [r.id, r.image]));
        previewProc.running = true;
    }
    Connections {
        target: listProc
        function onRunningChanged() {
            if (!listProc.running && root.again) {
                root.again = false;
                listProc.running = true;
            }
        }
    }
    Process {
        id: previewProc
        stdout: SplitParser {
            onRead: path => {
                if (!root.previewReady[path])
                    root.previewReady = Object.assign({}, root.previewReady, { [path]: true });
            }
        }
        onExited: Qt.callLater(root.startPreviews)
    }

    // cliphist's list shows a whitespace-collapsed preview ending in "…" when cut
    function norm(t) {
        return (t ?? "").replace(/\s+/g, " ").trim();
    }
    function isPinned(item) {
        const t = norm(item.text), cut = t.endsWith("…");
        const head = cut ? t.slice(0, -1) : t;
        return pins.some(p => item.image ? p.source === item.id : (cut ? norm(p.text).startsWith(head) : norm(p.text) === t));
    }

    function pin(item) {
        if (isPinned(item))
            return;
        if (item.image) {
            const dest = pinDir + "/" + Date.now() + "." + item.image.split(".").pop();
            Quickshell.execDetached(["sh", "-c", "umask 077; mkdir -p \"$1\" && cp \"$2\" \"$3\"", "sh", pinDir, item.image, dest]);
            UiState.data.clipPins = [{ image: dest, source: item.id }].concat(pins);
        } else {
            // cliphist list truncates long text; pin the full entry
            pinProc.command = ["cliphist", "-db-path", root.db, "decode", item.id];
            pinProc.running = true;
        }
    }
    Process {
        id: pinProc
        stdout: StdioCollector {
            onStreamFinished: if (text !== "") UiState.data.clipPins = [{ text: text }].concat(root.pins)
        }
    }

    function unpin(p) {
        UiState.data.clipPins = pins.filter(x => x !== p && !(x.text !== undefined && x.text === p.text) && !(x.image && x.image === p.image));
        if (p.image)
            Quickshell.execDetached(["rm", "-f", p.image]);
    }

    function remove(item) {
        Quickshell.execDetached(["sh", "-c", "printf '%s\\t%s\\n' \"$1\" \"$2\" | cliphist -db-path \"$3\" delete", "sh", item.id, item.text, root.db]);
        items = items.filter(i => i.id !== item.id);
    }

    function clear() {
        Quickshell.execDetached(["cliphist", "-db-path", root.db, "wipe"]);
        items = [];
    }

    // Copy an item (history row or pin). plain: text only, no rich formats.
    function copy(item, plain) {
        if (item.id !== undefined && !item.image)
            Quickshell.execDetached(["sh", "-c", "cliphist -db-path \"$2\" decode \"$1\" | wl-copy" + (plain ? " --type text/plain" : ""), "sh", item.id, root.db]);
        else if (item.id !== undefined)
            Quickshell.execDetached(["sh", "-c", "cliphist -db-path \"$2\" decode \"$1\" | wl-copy", "sh", item.id, root.db]);
        else if (item.image)
            Quickshell.execDetached(["sh", "-c", "wl-copy < \"$1\"", "sh", item.image]);
        else
            Quickshell.execDetached(["wl-copy", "--type", "text/plain", item.text]);
    }

    // Copy, then paste into the window that had focus before the panel opened.
    // Terminals paste with Ctrl+Shift+V.
    function paste(item, plain) {
        copy(item, plain);
        pasteTimer.restart();
    }
    readonly property var terminals: ["foot", "kitty", "alacritty", "org.wezfurlong.wezterm", "com.mitchellh.ghostty", "konsole", "org.gnome.ptyxis", "xterm"]
    Timer {
        id: pasteTimer
        interval: 180 // the panel releases keyboard focus first
        onTriggered: {
            const cls = (Apps.active?.appId ?? "").toLowerCase();
            Hyprland.dispatch("hl.dsp.send_shortcut({ mods = \"" + (root.terminals.includes(cls) ? "CTRL SHIFT" : "CTRL") + "\", key = \"V\" })");
        }
    }

    // Rows for the panel and Spotlight, filtered by query and kind (all | text | images)
    function filtered(query, kind) {
        const q = query.trim().toLowerCase();
        const match = t => q === "" || (t ?? "").toLowerCase().includes(q);
        const now = Date.now();
        const pinRows = pins.filter(p => kind !== "text" || !p.image).filter(p => kind !== "images" || p.image).filter(p => p.image ? q === "" : match(p.text)).map(p => ({ pin: p, text: p.text ?? "", image: p.image ?? "", pinned: true, section: "Pinned" }));
        const hist = items.filter(i => kind === "all" || (kind === "images") === !!i.image).filter(i => i.image ? q === "" : match(i.text)).filter(i => !isPinned(i)).map(i => ({ item: i, text: i.text, image: i.image, pinned: false, section: sectionFor(stamps[i.id], now) }));
        return pinRows.concat(hist);
    }
}
