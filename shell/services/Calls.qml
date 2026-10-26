pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common

// A call or meeting in progress, for the island's live activity: an app made
// for calls is using the microphone (Discord, Zoom, Teams, Slack...), or a
// browser is, with a meeting open in one of its windows (Google Meet, Zoom,
// Teams, Jitsi...). Mute is the system microphone; Leave closes the call's
// own window where the app has one (the app then asks or hangs up); a call in
// a browser tab offers Open instead, so other tabs are never closed.
Singleton {
    id: root

    readonly property bool on: Island.enabled && Island.wants("calls")

    readonly property var callApps: [
        { re: /discord|vesktop|webcord/i, name: "Discord", window: /discord|vesktop/i },
        { re: /^zoom|zoom\.real|zoomwebview/i, name: "Zoom", window: /^zoom$/i, callTitle: /^Zoom Meeting|^Meeting$/i },
        { re: /teams/i, name: "Teams", window: /teams/i },
        { re: /slack/i, name: "Slack", window: /slack/i, callTitle: /huddle/i },
        { re: /skype/i, name: "Skype", window: /skype/i },
        { re: /signal/i, name: "Signal", window: /signal/i },
        { re: /telegram/i, name: "Telegram", window: /telegram/i },
        { re: /element/i, name: "Element", window: /element/i },
        { re: /whatsapp|zapzap/i, name: "WhatsApp", window: /whatsapp|zapzap/i },
        { re: /webex/i, name: "Webex", window: /webex/i }
    ]
    readonly property var browsers: /firefox|librewolf|zen|floorp|waterfox|chrom|brave|vivaldi|edge|opera|epiphany|qutebrowser/i
    readonly property var meetings: [
        { re: /Google Meet|^Meet [-–]|meet\.google/i, name: "Google Meet" },
        { re: /Zoom/i, name: "Zoom" },
        { re: /Microsoft Teams|\| Teams/i, name: "Teams" },
        { re: /Jitsi/i, name: "Jitsi Meet" },
        { re: /Discord/i, name: "Discord" },
        { re: /Whereby/i, name: "Whereby" },
        { re: /Webex/i, name: "Webex" },
        { re: /Slack/i, name: "Slack" },
        { re: /WhatsApp/i, name: "WhatsApp" }
    ]

    // { name, window (HyprlandToplevel or null), leave (true: closing its window ends it) }
    readonly property var call: {
        if (!on)
            return null;
        const wins = Hyprland.toplevels.values;
        for (const s of Privacy.micStreams) {
            const id = (s.bin + " " + s.name).trim();
            const app = callApps.find(a => a.re.test(id));
            if (app) {
                const own = wins.filter(t => app.window.test(t.lastIpcObject?.class ?? ""));
                const callWin = app.callTitle ? own.find(t => app.callTitle.test(t.title ?? "")) : null;
                return { name: app.name, window: callWin ?? own[0] ?? null, leave: !!callWin };
            }
            if (browsers.test(id)) {
                for (const t of wins) {
                    if (!browsers.test(t.lastIpcObject?.class ?? ""))
                        continue;
                    const m = meetings.find(x => x.re.test(t.title ?? ""));
                    if (m)
                        return { name: m.name, window: t, leave: false };
                }
            }
        }
        return null;
    }
    readonly property bool active: call !== null
    property real started: 0
    onActiveChanged: started = active ? Date.now() : 0

    function open() {
        const t = call?.window;
        if (!t)
            return;
        const id = t.workspace?.id ?? 0;
        if (id > 0 && id !== Hyprland.focusedMonitor?.activeWorkspace?.id)
            Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
        Qt.callLater(() => t.wayland?.activate());
    }
    function leave() {
        const t = call?.window;
        if (!t || !call.leave)
            return open();
        Hyprland.dispatch("hl.dsp.window.close({ window = \"address:0x" + t.address.replace(/^0x/, "") + "\" })");
    }
}
