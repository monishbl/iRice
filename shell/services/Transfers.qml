pragma Singleton
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Io
import qs.common

// Downloads and file copies in progress, for the island's live activity.
//   downloads  browsers' partial files in the Downloads folder (.part,
//              .crdownload): the folder is watched, and their sizes are read
//              once a second only while there are some. Browsers don't say
//              how big the file will be, so these show size and speed.
//   copies     cp / mv / rsync / dd and friends, with how far through the
//              current file they are (assets/scripts/transfers.sh)
// (Copies made inside a file manager's own process can't be seen.)
Singleton {
    id: root

    readonly property bool on: Island.enabled && Island.wants("transfers")
    readonly property string downloadsDir: Quickshell.env("XDG_DOWNLOAD_DIR") || (Quickshell.env("HOME") + "/Downloads")

    // [{ id, kind: "download" | "copy" | "send" | "receive", name, done, total (0 = unknown), speed, pid? }]
    // (send / receive: LocalSend, nearby devices)
    readonly property var items: downloads.concat(copies).concat(LocalSend.items)
    readonly property bool active: items.length > 0
    readonly property var first: items[0] ?? null
    // Overall 0..1 when every item's size is known, else -1
    readonly property real progress: {
        if (!active || items.some(i => i.total <= 0))
            return -1;
        const done = items.reduce((a, i) => a + i.done, 0), total = items.reduce((a, i) => a + i.total, 0);
        return total > 0 ? Math.min(1, done / total) : -1;
    }
    readonly property real speed: items.reduce((a, i) => a + (i.speed || 0), 0)

    function size(bytes) {
        const u = ["B", "KB", "MB", "GB", "TB"];
        let i = 0, v = bytes;
        while (v >= 1000 && i < u.length - 1) {
            v /= 1000;
            i++;
        }
        return (i === 0 ? v : v.toFixed(v < 10 ? 1 : 0)) + " " + u[i];
    }
    function label(item) {
        if (!item)
            return "";
        return item.total > 0 ? Math.round(100 * item.done / item.total) + "%" : size(item.done);
    }

    // Stop a copy (downloads are the browser's to cancel)
    function cancel(item) {
        if (item?.kind === "send" || item?.kind === "receive")
            return LocalSend.cancel(item.id);
        if (item?.kind === "copy" && item.pid > 0)
            Quickshell.execDetached(["kill", "-INT", String(item.pid)]);
    }
    function openDownloads() {
        Files.openFolder(downloadsDir);
    }

    // ── Downloads ───────────────────────────────────────────────────────
    property var downloads: []
    property var lastSizes: ({}) // path -> { bytes, at }
    FolderListModel {
        id: partials
        folder: root.on ? "file://" + root.downloadsDir : ""
        nameFilters: ["*.part", "*.crdownload", "*.download"]
        showDirs: false
        showHidden: true
        onCountChanged: if (count > 0) sizes.running = true
    }
    Timer {
        interval: 1000
        repeat: true
        running: root.on && partials.count > 0
        onRunningChanged: if (!running) root.downloads = []
        onTriggered: sizes.running = true
    }
    Process {
        id: sizes
        command: ["sh", "-c", "for f in \"$1\"/*.part \"$1\"/*.crdownload \"$1\"/*.download \"$1\"/.*.part \"$1\"/.*.crdownload; do [ -f \"$f\" ] && printf '%s\\t%s\\n' \"$(stat -c %s \"$f\")\" \"$f\"; done", "sh", root.downloadsDir]
        stdout: StdioCollector {
            onStreamFinished: {
                const now = Date.now(), seen = {}, list = [];
                for (const l of text.split("\n")) {
                    const tab = l.indexOf("\t");
                    if (tab < 0)
                        continue;
                    const bytes = parseInt(l.slice(0, tab)) || 0, path = l.slice(tab + 1);
                    const last = root.lastSizes[path];
                    const speed = last && now > last.at ? Math.max(0, (bytes - last.bytes) * 1000 / (now - last.at)) : 0;
                    seen[path] = { bytes: bytes, at: now };
                    // "film.mkv.part" -> "film.mkv" (Chromium's "Unconfirmed 1234.crdownload" stays as is)
                    const name = path.split("/").pop().replace(/^\./, "").replace(/\.(part|crdownload|download)$/, "");
                    list.push({ id: path, kind: "download", name: name, done: bytes, total: 0, speed: speed });
                }
                root.lastSizes = seen;
                root.downloads = list;
            }
        }
    }

    // ── Copies ──────────────────────────────────────────────────────────
    property var copies: []
    property var lastCopy: ({}) // pid -> { pos, at }
    Process {
        id: copyWatch
        running: root.on
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        command: [Tools.watcher, decodeURIComponent(Qt.resolvedUrl("../assets/scripts/transfers.sh").toString().replace("file://", ""))]
        stdout: SplitParser {
            onRead: line => {
                let list;
                try {
                    list = JSON.parse(line);
                } catch (e) {
                    return;
                }
                const now = Date.now(), seen = {};
                root.copies = list.map(c => {
                    const last = root.lastCopy[c.pid];
                    const speed = last && now > last.at && c.pos >= last.pos ? (c.pos - last.pos) * 1000 / (now - last.at) : 0;
                    seen[c.pid] = { pos: c.pos, at: now };
                    return { id: "copy" + c.pid, kind: "copy", name: c.file.split("/").pop(), done: c.pos, total: c.size, speed: speed, pid: c.pid, cmd: c.cmd };
                });
                root.lastCopy = seen;
            }
        }
        onExited: if (root.on) copyRetry.start()
    }
    Timer {
        id: copyRetry
        interval: 5000
        onTriggered: copyWatch.running = root.on
    }
}
