pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Things also running that do iRice's job twice (assets/scripts/conflicts.py):
// another notification daemon, another bar or shell, a second hypridle or
// clipboard recorder, user services that start those. Looked at once, 10 s
// after the shell starts (once the session's autostart has settled), and
// whenever System health checks. The first look posts ONE island alert
// naming them ("Waybar and mako · Also running…") with Review (System
// health, which lists each with how to stop it) and Don't show again,
// remembered per thing in state.json (UiState conflictsMuted).
Singleton {
    id: root

    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/conflicts.py").toString().replace("file://", ""))
    property var found: [] // [{ id, name, short, what, fix }]
    property bool checked: false
    readonly property var muted: UiState.data.conflictsMuted ?? []
    readonly property var fresh: found.filter(c => !muted.includes(c.id))
    property bool announced: false

    function check() {
        if (!proc.running && Tools.has("python3"))
            proc.running = true;
    }
    Timer {
        running: Tools.ready && UiState.loaded
        interval: 10000
        onTriggered: root.check()
    }
    Connections {
        target: Health
        function onCheckingChanged() {
            if (Health.checking)
                root.check();
        }
    }
    Process {
        id: proc
        command: ["python3", "-I", root.script]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.found = JSON.parse(text);
                } catch (e) {
                    root.found = [];
                }
                root.checked = true;
                if (!root.announced) {
                    root.announced = true;
                    root.announce();
                }
            }
        }
    }

    // "waybar", "waybar and mako", "waybar, mako and hypridle"
    function names(list) {
        const n = list.map(c => c.name).filter((x, i, a) => a.indexOf(x) === i);
        return n.length <= 1 ? (n[0] ?? "") : n.slice(0, -1).join(", ") + " and " + n[n.length - 1];
    }
    function announce() {
        const list = fresh;
        if (list.length === 0)
            return;
        // Title: the names ("Waybar and mako"); subtitle: what that means
        const running = list.filter(c => !c.id.startsWith("unit:"));
        const title = names(list);
        const ids = list.map(c => c.id);
        Island.post({
            kind: "custom",
            icon: "warning",
            title: title.charAt(0).toUpperCase() + title.slice(1),
            subtitle: list.length === 1 ? list[0].short : running.length > 0 ? "Also running, doing what iRice does" : "Start with your session, doing what iRice does",
            color: Theme.accents.orange,
            action: "Review",
            onAction: () => root.review(),
            more: [{ text: "Don't show again", act: () => root.mute(ids) }] // (a row of its own: the names get the room)
        }, 12000);
    }
    function review() {
        Panels.openSettings("about/health");
    }
    function mute(ids) {
        UiState.data.conflictsMuted = muted.concat(ids.filter(i => !muted.includes(i)));
    }
    function unmute(id) {
        UiState.data.conflictsMuted = muted.filter(i => i !== id);
    }

    IpcHandler {
        target: "conflicts"
        // qs ipc call conflicts check | announce (again, for testing)
        function check(): void { root.announced = false; root.check(); }
        function announce(): void { root.announce(); }
    }
}
