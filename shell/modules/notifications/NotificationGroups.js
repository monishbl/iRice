.pragma library
// Notification Center's list, in both looks: the notifications grouped by
// app, newest group first. The island's alerts (not notifications: those
// are listed already) are one more group, Tessera, placed by its newest.
//   byApp(Notifications.list, Notifications.received, Island.history)
function byApp(list, received, history) {
    const out = [];
    for (const n of list.slice().reverse()) {
        const app = n.appName || "Notification";
        let g = out.find(x => x.app === app && !x.island);
        if (!g)
            out.push(g = { app: app, items: [], at: received[n.id]?.getTime() ?? 0 });
        g.items.push(n);
    }
    if (history.length > 0) {
        const i = out.findIndex(g => g.at < history[0].at);
        out.splice(i < 0 ? out.length : i, 0, { app: "Tessera", island: true, items: history, at: history[0].at });
    }
    return out;
}

// What kind of alert an island alert is, as its card's small caption
const kinds = { device: "Bluetooth", usb: "USB", display: "Display", audio: "Sound output", charging: "Power", battery: "Battery", temperature: "Temperature", resources: "System", network: "Network", transfer: "Downloads", timer: "Timer", recording: "Screen recording", focus: "Focus", sysupdate: "Software update", custom: "Tessera" };
function kindOf(alert) {
    return kinds[alert?.kind ?? ""] ?? "Tessera";
}
