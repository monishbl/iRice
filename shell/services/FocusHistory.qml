pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Windows in the order they were last focused, across every space and
// monitor (for the Alt+Tab switcher). Kept from Hyprland's activewindowv2 and
// closewindow events, so it's up to date the moment focus changes; windows
// focused before the shell started are ranked by Hyprland's own
// focusHistoryID. Addresses are stored without "0x", lowercased.
Singleton {
    id: root

    property var order: [] // addresses, most recently focused first
    readonly property int limit: 200

    function key(address) {
        return String(address ?? "").replace(/^0x/, "").toLowerCase();
    }

    function touch(address) {
        const k = key(address);
        if (k === "" || order[0] === k)
            return;
        order = [k].concat(order.filter(a => a !== k)).slice(0, limit);
    }
    function forget(address) {
        const k = key(address);
        if (order.includes(k))
            order = order.filter(a => a !== k);
    }

    // The given HyprlandToplevels, most recently focused first. Windows the
    // history hasn't seen follow, by Hyprland's focusHistoryID (0 = latest),
    // then in Hyprland's order.
    function sorted(toplevels) {
        const rank = new Map(order.map((a, i) => [a, i]));
        return toplevels.map((t, i) => ({ t: t, i: i, r: rank.get(key(t.address)), h: t.lastIpcObject?.focusHistoryID ?? Infinity }))
            .sort((a, b) => {
                if (a.r !== undefined || b.r !== undefined)
                    return (a.r ?? Infinity) - (b.r ?? Infinity) || a.h - b.h || a.i - b.i;
                return a.h - b.h || a.i - b.i;
            })
            .map(x => x.t);
    }

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            // activewindowv2>>ADDRESS (empty or "," when nothing is focused)
            if (event.name === "activewindowv2") {
                const a = event.data.trim();
                if (a !== "" && a !== ",")
                    root.touch(a);
            } else if (event.name === "closewindow") {
                root.forget(event.data.trim());
            }
        }
    }

    // After a shell restart: start from the window that has focus now
    Component.onCompleted: {
        const t = Hyprland.activeToplevel;
        if (t?.address)
            touch(t.address);
    }
}
