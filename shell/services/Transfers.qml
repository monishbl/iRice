pragma Singleton
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Io
import qs.common
import "DownloadTracker.js" as Tracker

// Downloads and file copies in progress, for the island's live activity.
//   downloads  browsers' partial files in the Downloads folder (.part,
//              .crdownload): the folder is watched, and their sizes are read
//              once a second while one is growing, every 10 s while they
//              only sit there. Browsers don't say how big the file will be,
//              so these show size and speed. One that stops growing isn't
//              shown, and when one finishes the island says so
//              (DownloadTracker.js decides).
//   copies     cp / mv / rsync / dd and friends, with how far through the
//              current file they are (assets/scripts/transfers.sh)
// (Copies made inside a file manager's own process can't be seen.)
Singleton {
    id: root

    readonly property bool on: Island.enabled && Island.wants("transfers")
    readonly property string downloadsDir: Quickshell.env("XDG_DOWNLOAD_DIR") || (Quickshell.env("HOME") + "/Downloads")

    // [{ id, kind: "download" | "copy" | "send" | "receive", name, done, total (0 = unknown), speed, pid? }]
    // (send / receive: nearby devices, LocalSend or Quick Share: `via`)
    readonly property var items: downloads.concat(copies).concat(LocalSend.items).concat(QuickShare.items)
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
            return item.via === "quickshare" ? QuickShare.cancel(item.id) : LocalSend.cancel(item.id);
        if (item?.kind === "copy" && item.pid > 0 && item.start)
            Quickshell.execDetached(["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/process-identity.py").toString().replace("file://", "")), "interrupt", String(item.pid), String(item.start)]);
    }
    function openDownloads() {
        Files.openFolder(downloadsDir);
    }

    // ── Downloads ───────────────────────────────────────────────────────
    property var downloads: []
    property var tracked: ({}) // DownloadTracker's memory between looks
    FolderListModel {
        id: partials
        folder: root.on ? "file://" + root.downloadsDir : ""
        nameFilters: ["*.part", "*.crdownload", "*.download"]
        showDirs: false
        showHidden: true
        onCountChanged: if (count > 0) sizes.running = true
    }
    Timer {
        interval: root.downloads.length > 0 ? 1000 : 10000
        repeat: true
        running: root.on && partials.count > 0
        // The last partial went: one more look, so a finished download is
        // seen to finish (and nothing stays on show)
        onRunningChanged: if (!running) sizes.running = true
        onTriggered: sizes.running = true
    }
    Process {
        id: sizes
        // Every second while something downloads: find, not a python3 start
        // each time. "size<TAB>path", NUL-separated (any file name is safe)
        command: ["find", root.downloadsDir, "-maxdepth", "1", "-type", "f", "(", "-name", "*.part", "-o", "-name", "*.crdownload", "-o", "-name", "*.download", ")", "-printf", "%s\t%p\0"]
        stdout: StdioCollector {
            onStreamFinished: {
                const entries = [];
                for (const entry of text.split("\0")) {
                    const tab = entry.indexOf("\t");
                    if (tab > 0)
                        entries.push({ bytes: Number(entry.slice(0, tab)), path: entry.slice(tab + 1) });
                }
                const r = Tracker.update(root.tracked, root.on ? entries : [], Date.now());
                root.tracked = r.tracked;
                root.downloads = r.items;
                if (r.gone.length > 0 && root.on) {
                    root.finishing = root.finishing.concat(r.gone);
                    recent.running = true;
                }
            }
        }
    }

    // Downloads that went away after growing: finished (renamed to their
    // real name) or cancelled (deleted). The files changed in the last
    // minute say which; a finished one gets "Downloaded <name>".
    property var finishing: []
    Process {
        id: recent
        command: ["find", root.downloadsDir, "-maxdepth", "1", "-type", "f", "-mmin", "-2", "-printf", "%s\t%T@\t%p\0"]
        stdout: StdioCollector {
            onStreamFinished: {
                const files = [];
                for (const entry of text.split("\0")) {
                    const f = entry.split("\t");
                    if (f.length >= 3)
                        files.push({ bytes: Number(f[0]), mtime: Number(f[1]) * 1000, path: f.slice(2).join("\t") });
                }
                const done = root.finishing;
                root.finishing = [];
                for (const g of done) {
                    const path = Tracker.finalFile(g, files);
                    if (path !== "")
                        root.announce(path);
                }
            }
        }
    }
    function announce(path) {
        Island.post({
            kind: "transfer", icon: "download-simple", color: Theme.accents.green,
            title: "Downloaded " + path.split("/").pop(), subtitle: "In " + downloadsDir.replace(Quickshell.env("HOME"), "~"),
            action: "Open", onAction: () => Files.open(path),
            action2: "Show in Folder", onAction2: () => Files.reveal(path)
        }, 6000);
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
                    return { id: "copy" + c.pid, kind: "copy", name: c.file.split("/").pop(), done: c.pos, total: c.size, speed: speed, pid: c.pid, start: c.start, cmd: c.cmd };
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
