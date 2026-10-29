import QtQuick
import Quickshell
import Quickshell.Wayland

// A quiet, keyboard-first login prompt. Authentication still goes through greetd.
PanelWindow {
    id: win
    required property var auth
    property bool primary: false
    color: "#101215"
    WlrLayershell.namespace: "irice-greeter"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }
    readonly property string mono: "monospace"
    property string powerChoice: ""
    function nextUser(step) {
        if (auth.busy || !auth.users.length) return;
        auth.user = (auth.user + auth.users.length + step) % auth.users.length;
        input.text = "";
    }
    function nextSession() {
        if (!auth.busy && auth.sessions.length) auth.session = (auth.session + 1) % auth.sessions.length;
    }
    // Keyboard first, but everything also works by pointer: Sign in, and the
    // power actions (choosing one asks to confirm: Enter, or the same again)
    function submit() {
        if (powerChoice) {
            const action = powerChoice;
            powerChoice = "";
            if (auth.mock) console.log("greeter (mock):", action);
            else Quickshell.execDetached(["systemctl", action]);
        } else if (!auth.busy) {
            auth.login(input.text);
        }
    }
    function choosePower(action) {
        if (auth.busy) return;
        if (powerChoice === action) submit();
        else powerChoice = action;
        input.forceActiveFocus();
    }
    readonly property var powerActions: [["F2", "sleep", "suspend"], ["F3", "restart", "reboot"], ["F4", "shut down", "poweroff"]]
    onPrimaryChanged: if (primary) input.forceActiveFocus()
    Connections {
        target: win.auth
        function onErrorChanged() { if (win.auth.error) input.text = ""; }
        function onUserChanged() { input.text = ""; }
    }
    Column {
        visible: win.primary
        anchors.centerIn: parent
        width: Math.min(520, parent.width - 64)
        spacing: 18
        Text { text: "iRice / login"; color: "#e6e9ef"; font { family: win.mono; pixelSize: 22 } }
        Text {
            text: "user     " + (win.auth.users[win.auth.user]?.name ?? "No users")
            color: "#c6cbd5"; font { family: win.mono; pixelSize: 15 }
            MouseArea { anchors.fill: parent; onClicked: win.nextUser(1) }
        }
        Text {
            text: "session  " + (win.auth.sessions[win.auth.session]?.name ?? "Loading…")
            color: "#c6cbd5"; font { family: win.mono; pixelSize: 15 }
            MouseArea { anchors.fill: parent; onClicked: win.nextSession() }
        }
        Row {
            width: parent.width
            spacing: 12
            Text { text: "password >"; color: "#9bbcff"; font { family: win.mono; pixelSize: 15 } }
            TextInput {
                id: input
                width: Math.max(80, parent.width - 130)
                color: "#e6e9ef"
                font { family: win.mono; pixelSize: 15 }
                echoMode: TextInput.Password
                readOnly: win.auth.busy || win.powerChoice !== ""
                focus: win.primary
                Component.onCompleted: if (win.primary) forceActiveFocus()
                onAccepted: win.submit()
                Keys.onUpPressed: win.nextUser(-1)
                Keys.onDownPressed: win.nextUser(1)
                Keys.onTabPressed: win.nextSession()
                Keys.onEscapePressed: { win.powerChoice = ""; text = ""; }
                Keys.onPressed: event => {
                    const actions = { [Qt.Key_F2]: "suspend", [Qt.Key_F3]: "reboot", [Qt.Key_F4]: "poweroff" };
                    if (actions[event.key] && !win.auth.busy) {
                        win.powerChoice = actions[event.key];
                        event.accepted = true;
                    }
                }
            }
        }
        Text {
            width: parent.width; wrapMode: Text.Wrap
            text: win.powerChoice ? win.powerChoice + ": Enter (or click it again) to confirm · Esc to cancel"
                : win.auth.error || (win.auth.busy ? "Signing in…" : win.auth.mock ? "Preview password: irice" : "Enter to sign in")
            color: win.auth.error ? "#ff9c9c" : "#8d96a5"
            font { family: win.mono; pixelSize: 13 }
        }
        // Sign in, for pointer users (Enter does the same)
        Text {
            visible: !win.powerChoice
            height: 44
            verticalAlignment: Text.AlignVCenter
            text: win.auth.busy ? "[ signing in… ]" : "[ sign in ]"
            color: signInMouse.containsMouse && !win.auth.busy ? "#e6e9ef" : "#9bbcff"
            font { family: win.mono; pixelSize: 15 }
            MouseArea {
                id: signInMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: win.auth.busy ? Qt.ArrowCursor : Qt.PointingHandCursor
                onClicked: win.submit()
            }
        }
        Text {
            width: parent.width; wrapMode: Text.Wrap
            text: (win.auth.users.length > 1 ? "↑↓ user · " : "") + "Tab session"
            color: "#8d96a5"
            font { family: win.mono; pixelSize: 12 }
        }
        // F2 sleep · F3 restart · F4 shut down, each a 44 px target too
        Row {
            spacing: 18
            Repeater {
                model: win.powerActions
                Text {
                    required property var modelData
                    height: 44
                    verticalAlignment: Text.AlignVCenter
                    text: modelData[0] + " " + modelData[1]
                    color: win.powerChoice === modelData[2] ? "#ff9c9c" : powerMouse.containsMouse ? "#c6cbd5" : "#8d96a5"
                    font { family: win.mono; pixelSize: 12 }
                    MouseArea {
                        id: powerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: win.choosePower(parent.modelData[2])
                    }
                }
            }
        }
    }
}
