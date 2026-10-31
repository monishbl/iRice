pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.common

// Dock badges: unread counts apps report over D-Bus (the Unity LauncherEntry
// API: Telegram, Discord, Thunderbird, Signal, Element...), and a dot for
// apps whose window asks for attention (urgent).
Singleton {
    id: root

    // desktop entry id -> count (only visible counts > 0)
    property var counts: ({})
    // window classes that want attention
    property var urgent: ({})

    function countFor(entryId) {
        return counts[entryId] ?? 0;
    }
    function urgentFor(entryId) {
        return Object.keys(urgent).some(cls => urgent[cls] && (cls === entryId || Apps.entryFor(cls)?.id === entryId));
    }

    // ── LauncherEntry signals, read from dbus-monitor's text ────────────
    //   signal ... interface=com.canonical.Unity.LauncherEntry; member=Update
    //      string "application://org.telegram.desktop.desktop"
    //      array [ dict entry( string "count"  variant int64 3 ) ... ]
    property string app: ""
    property string key: ""
    property var pending: null
    function flush() {
        if (!app || !pending)
            return;
        const id = app.replace(/^application:\/\//, "").replace(/\.desktop$/, "");
        const c = Object.assign({}, counts);
        const prev = c[id] ?? 0;
        const visible = pending.visible ?? (prev > 0 || pending.count > 0);
        const n = pending.count ?? prev;
        if (visible && n > 0)
            c[id] = n;
        else
            delete c[id];
        counts = c;
        pending = null;
    }
    KeptProcess {
        id: monitor
        wanted: Tools.has("dbus-monitor")
        stdinEnabled: true
        command: [Tools.watcher, "dbus-monitor", "--session", "type='signal',interface='com.canonical.Unity.LauncherEntry',member='Update'"]
        stdout: SplitParser {
            onRead: line => {
                const t = line.trim();
                if (t.startsWith("signal ")) {
                    root.flush();
                    root.app = "";
                    root.pending = t.includes("LauncherEntry") ? {} : null;
                    return;
                }
                if (root.pending === null)
                    return;
                let m = t.match(/^string "(.*)"$/);
                if (m) {
                    if (!root.app)
                        root.app = m[1];
                    else
                        root.key = m[1];
                    return;
                }
                m = t.match(/^variant\s+(\w+)\s+(.*)$/);
                if (m) {
                    if (root.key === "count")
                        root.pending.count = parseInt(m[2]) || 0;
                    else if (root.key === "count-visible")
                        root.pending.visible = m[2] === "true";
                    root.key = "";
                    return;
                }
                if (t === "]")
                    root.flush();
            }
        }
    }

    // ── Urgent windows ──────────────────────────────────────────────────
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "urgent") {
                const t = Hyprland.toplevels.values.find(w => w.address.replace(/^0x/, "") === event.data.replace(/^0x/, ""));
                const cls = t?.lastIpcObject?.class ?? "";
                if (cls)
                    root.urgent = Object.assign({}, root.urgent, { [cls]: true });
            } else if (event.name === "activewindow") {
                // Looking at the app clears its dot
                const cls = event.data.split(",")[0];
                if (root.urgent[cls]) {
                    const u = Object.assign({}, root.urgent);
                    delete u[cls];
                    root.urgent = u;
                }
            }
        }
    }
}
