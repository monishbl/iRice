import QtQuick
import Quickshell
import Quickshell.Io
import "session-exec.js" as SessionExec
import Quickshell.Services.Greetd

// Login logic for the greeter: users from /etc/passwd, sessions from
// wayland-sessions, and the greetd conversation. Without greetd (or with
// IRICE_GREETER_MOCK=1) it runs a mock that accepts the password "irice",
// for trying the greeter in a normal session.
QtObject {
    id: root

    readonly property bool mock: Quickshell.env("IRICE_GREETER_MOCK") === "1" || !Greetd.available
    property var users: [] // [{ name, full }]
    property var sessions: [] // [{ name, exec }]
    property int user: 0
    property int session: 0
    property bool busy: false
    property string error: ""
    property string prompt: "Password"
    property bool success: false
    property string pending: "" // password waiting for greetd's prompt

    readonly property string cacheFile: (Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache") + "/irice-greeter.json"

    function login(password) {
        if (busy || users.length === 0)
            return;
        error = "";
        busy = true;
        if (mock) {
            mockTimer.password = password;
            mockTimer.restart();
            return;
        }
        pending = password;
        Greetd.createSession(users[user].name);
    }

    function launch() {
        const s = sessions[session];
        let command;
        try { command = SessionExec.parse(s?.exec ?? "", s?.name ?? ""); }
        catch (e) { error = String(e); busy = false; if (!mock) Greetd.cancelSession(); return; }
        remember.setText(JSON.stringify({ user: users[user]?.name ?? "", session: s?.name ?? "" }));
        success = true;
        if (mock) {
            console.log("greeter (mock): would launch", s?.exec);
            return;
        }
        Greetd.launch(command, [], true);
    }

    property Connections greetd: Connections {
        target: root.mock ? null : Greetd
        function onAuthMessage(message, isError, responseRequired, echoResponse) {
            if (responseRequired) {
                Greetd.respond(root.pending);
                root.pending = "";
            } else if (isError) {
                root.error = message;
            }
        }
        function onAuthFailure(message) {
            root.busy = false;
            root.error = "Incorrect password. Try again.";
            Greetd.cancelSession();
        }
        function onReadyToLaunch() {
            root.busy = false;
            root.launch();
        }
        function onError(message) {
            root.busy = false;
            root.error = message || "Login failed. Try again.";
        }
    }

    property Timer mockTimer: Timer {
        property string password
        interval: 600
        onTriggered: {
            root.busy = false;
            if (password === "irice")
                root.launch();
            else
                root.error = "Incorrect password. Try again.";
        }
    }

    // Users with a login shell and a normal uid
    property FileView passwd: FileView {
        path: "/etc/passwd"
        onLoaded: {
            root.users = text().split("\n").map(l => l.split(":")).filter(f => f.length >= 7 && +f[2] >= 1000 && +f[2] < 60000 && !/(nologin|false)$/.test(f[6])).map(f => ({ name: f[0], full: (f[4] ?? "").split(",")[0] || f[0] }));
            root.restoreLast();
        }
    }

    // Sessions: Name and Exec from each .desktop file
    property Process sessionList: Process {
        running: true
        command: ["sh", "-c", "for f in /usr/share/wayland-sessions/*.desktop; do [ -f \"$f\" ] || continue; n=$(grep -m1 '^Name=' \"$f\" | cut -d= -f2-); e=$(grep -m1 '^Exec=' \"$f\" | cut -d= -f2-); printf '%s\\t%s\\n' \"$n\" \"$e\"; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                const list = text.split("\n").filter(l => l.includes("\t")).map(l => ({ name: l.split("\t")[0], exec: l.split("\t")[1] }));
                // Hyprland first: it's what iRice runs on
                list.sort((a, b) => (/hyprland/i.test(b.name) - /hyprland/i.test(a.name)) || a.name.localeCompare(b.name));
                root.sessions = list.length > 0 ? list : [{ name: "Hyprland", exec: "Hyprland" }];
                root.restoreLast();
            }
        }
    }

    property FileView remember: FileView {
        path: root.cacheFile
        onLoaded: root.restoreLast()
    }
    function restoreLast() {
        try {
            const j = JSON.parse(remember.text());
            const u = users.findIndex(x => x.name === j.user);
            const s = sessions.findIndex(x => x.name === j.session);
            if (u >= 0)
                user = u;
            if (s >= 0)
                session = s;
        } catch (e) {}
    }
}
