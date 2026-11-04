import QtQuick
import Quickshell
import Quickshell.Io
import "session-exec.js" as SessionExec
import Quickshell.Services.Greetd

// Login logic for the greeter: users from /etc/passwd, sessions from
// wayland-sessions, and the greetd conversation. The conversation starts as
// soon as a user is chosen, not when Enter is pressed, so a fingerprint
// reader (pam_fprintd in greetd's PAM file: irice-root fingerprint on login)
// listens straight away: its words ("Place your finger…") are `message`,
// and a finger logs in by itself. A password typed meanwhile waits for
// PAM to ask for it; typing is never held up. Without greetd (or with
// IRICE_GREETER_MOCK=1) it runs a mock that accepts the password "irice",
// for trying the greeter in a normal session (Escape quits it there).
// The name can also be typed (other, otherName): any account, listed or
// not; a typed name that's on the list becomes that person again.
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
    // The conversation: open (a user's chosen), and asked for the password
    property bool open: false
    property bool asked: false
    // What PAM says that isn't a question (the fingerprint reader's words)
    property string message: ""
    // A username typed in instead of a person chosen (always, with no one
    // listed)
    property bool other: false
    property string otherName: ""
    // Who logs in: the chosen person's username, or the one typed in
    readonly property string userName: other ? otherName.trim() : (users[user]?.name ?? "")
    // The reader: "" (none heard of) | waiting | miss
    property string finger: ""

    readonly property string cacheFile: (Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache") + "/irice-greeter.json"

    function login(password) {
        if (busy)
            return;
        if (userName === "") {
            error = "Type your username first.";
            return;
        }
        error = "";
        busy = true;
        if (mock) {
            mockTimer.password = password;
            mockTimer.restart();
            return;
        }
        if (asked) {
            asked = false;
            Greetd.respond(password);
        } else {
            pending = password;
            begin();
        }
    }
    // Opens the conversation for the chosen user (again after a failure, so
    // the reader listens again)
    function begin() {
        if (mock || open || userName === "")
            return;
        open = true;
        asked = false;
        Greetd.createSession(userName);
    }
    function restart() {
        if (mock)
            return;
        if (open)
            Greetd.cancelSession();
        open = false;
        asked = false;
        message = "";
        finger = "";
        beginTimer.restart();
    }
    onUserChanged: restart()
    onOtherChanged: restart()
    onOtherNameChanged: if (other) restart()
    // Typing a name: starts from the chosen person's
    function editName() {
        if (busy || other)
            return;
        otherName = users[user]?.name ?? "";
        other = true;
    }
    // Done typing: a listed name is that person again
    function pickTyped() {
        const i = users.findIndex(x => x.name === otherName.trim());
        if (!other || i < 0)
            return;
        user = i;
        other = false;
    }
    // Back to the list (when there is one)
    function cancelName() {
        if (users.length > 0)
            other = false;
    }
    property Timer beginTimer: Timer {
        interval: 300
        onTriggered: root.begin()
    }

    function launch() {
        const s = sessions[session];
        let command;
        try { command = SessionExec.parse(s?.exec ?? "", s?.name ?? ""); }
        catch (e) { // (a field the parser can't expand: say what to do, not the parser's words)
            console.warn("greeter:", e);
            error = "This session's launch command isn't supported. Choose another with Tab.";
            busy = false;
            restart();
            return;
        }
        remember.setText(JSON.stringify({ user: userName, session: s?.name ?? "" }));
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
                if (root.pending !== "") {
                    Greetd.respond(root.pending);
                    root.pending = "";
                } else {
                    root.asked = true;
                    root.prompt = message || "Password";
                }
            } else if (/finger|swipe|touch/i.test(message)) {
                // (pam_fprintd: listening, or the finger didn't match)
                root.finger = isError ? "miss" : "waiting";
                root.message = message;
            } else if (isError) {
                root.error = message;
            } else {
                root.message = message;
            }
        }
        function onAuthFailure(message) {
            root.busy = false;
            root.pending = "";
            root.error = "Incorrect password. Try again.";
            root.restart();
        }
        function onReadyToLaunch() {
            root.busy = false;
            root.open = false;
            root.launch();
        }
        function onError(message) {
            root.busy = false;
            root.pending = "";
            root.error = message || "Login failed. Try again.";
            root.restart();
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
    // A preview (mock) ends with Escape, or a moment after a "login"
    function quitPreview() {
        if (mock)
            Qt.quit();
    }
    property Timer mockDone: Timer {
        running: root.mock && root.success
        interval: 1200
        onTriggered: Qt.quit()
    }

    // Users with a login shell and a normal uid
    property FileView passwd: FileView {
        path: "/etc/passwd"
        onLoaded: {
            root.users = text().split("\n").map(l => l.split(":")).filter(f => f.length >= 7 && +f[2] >= 1000 && +f[2] < 60000 && !/(nologin|false)$/.test(f[6])).map(f => ({ name: f[0], full: (f[4] ?? "").split(",")[0] || f[0] }));
            if (root.users.length === 0)
                root.other = true;
            root.restoreLast();
            root.beginTimer.restart();
        }
    }

    // Sessions: Name and Exec from each .desktop file
    property Process sessionList: Process {
        running: true
        command: ["sh", "-c", "for f in /usr/share/wayland-sessions/*.desktop; do [ -f \"$f\" ] || continue; n=$(grep -m1 '^Name=' \"$f\" | cut -d= -f2-); e=$(grep -m1 '^Exec=' \"$f\" | cut -d= -f2-); printf '%s\\t%s\\n' \"$n\" \"$e\"; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                const list = text.split("\n").filter(l => l.includes("\t")).map(l => ({ name: l.split("\t")[0], exec: l.split("\t")[1] }));
                // iRice's own session first, then Hyprland (what iRice runs
                // on): a first login starts iRice, not a bare Hyprland
                const rank = s => /irice/i.test(s.name) ? 0 : /hyprland/i.test(s.name) ? 1 : 2;
                list.sort((a, b) => (rank(a) - rank(b)) || a.name.localeCompare(b.name));
                root.sessions = list.length > 0 ? list : [{ name: "Hyprland", exec: "Hyprland" }];
                root.restoreLast();
            }
        }
    }

    // This computer's name (the text style's title)
    property string hostname: ""
    property FileView hostFile: FileView {
        path: "/etc/hostname"
        onLoaded: root.hostname = text().trim()
    }
    // A user's picture the greeter may read: AccountsService's (your
    // ~/.face it can't)
    function avatar(name) {
        return name ? "file:///var/lib/AccountsService/icons/" + name : "";
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
            if (u >= 0) {
                user = u;
            } else if (j.user) { // (last time, a username typed in)
                otherName = j.user;
                other = true;
            }
            if (s >= 0)
                session = s;
        } catch (e) {}
    }
}
