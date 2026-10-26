pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Clipboard history (cliphist) with image previews and pinned items. Pins
// survive restarts in UiState; pinned images are copied into the state dir so
// they outlive cliphist's history.
Singleton {
    id: root

    // [{ id, text, image }] newest first; image = preview path for binary entries
    property var items: []
    readonly property var pins: UiState.data.clipPins // [{ text } | { image }]
    readonly property string previewDir: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-clip"
    readonly property string pinDir: UiState.dir + "/clip-pins"

    function refresh() {
        listProc.running = true;
    }

    Process {
        id: listProc
        command: ["cliphist", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                const rows = text.split("\n").filter(l => l.includes("\t")).slice(0, 200).map(l => {
                    const tab = l.indexOf("\t");
                    const t = l.slice(tab + 1);
                    const m = t.match(/^\[\[ binary data .* (png|jpe?g|webp|gif|bmp) /i);
                    return { id: l.slice(0, tab), text: t, image: m ? root.previewDir + "/" + l.slice(0, tab) + "." + m[1].toLowerCase() : "" };
                });
                root.items = rows;
                // Decode previews for the first images only (cheap, kept in RAM-backed runtime dir)
                const want = rows.filter(r => r.image).slice(0, 24);
                if (want.length > 0) {
                    previewProc.command = ["sh", "-c", "mkdir -p \"$1\"; shift; while [ $# -gt 0 ]; do [ -s \"$2\" ] || cliphist decode \"$1\" > \"$2\"; shift 2; done", "sh", root.previewDir].concat(...want.map(r => [r.id, r.image]));
                    previewProc.running = true;
                }
            }
        }
    }
    property int previewVersion: 0
    Process {
        id: previewProc
        onExited: root.previewVersion++
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
            pinProc.command = ["cliphist", "decode", item.id];
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
        Quickshell.execDetached(["sh", "-c", "printf '%s\\t%s\\n' \"$1\" \"$2\" | cliphist delete", "sh", item.id, item.text]);
        items = items.filter(i => i.id !== item.id);
    }

    function clear() {
        Quickshell.execDetached(["cliphist", "wipe"]);
        items = [];
    }

    // Copy an item (history row or pin). plain: text only, no rich formats.
    function copy(item, plain) {
        if (item.id !== undefined && !item.image)
            Quickshell.execDetached(["sh", "-c", "cliphist decode \"$1\" | wl-copy" + (plain ? " --type text/plain" : ""), "sh", item.id]);
        else if (item.id !== undefined)
            Quickshell.execDetached(["sh", "-c", "cliphist decode \"$1\" | wl-copy", "sh", item.id]);
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
        const pinRows = pins.filter(p => kind !== "text" || !p.image).filter(p => kind !== "images" || p.image).filter(p => p.image ? q === "" : match(p.text)).map(p => ({ pin: p, text: p.text ?? "", image: p.image ?? "", pinned: true }));
        const hist = items.filter(i => kind === "all" || (kind === "images") === !!i.image).filter(i => i.image ? q === "" : match(i.text)).filter(i => !isPinned(i)).map(i => ({ item: i, text: i.text, image: i.image, pinned: false }));
        return pinRows.concat(hist);
    }
}
