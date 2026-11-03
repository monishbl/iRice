import QtQuick
import Quickshell
import Quickshell.Wayland
import "../../common/phosphor.js" as Phosphor

// The text login screen, ly's way: the power keys along the top, and in the
// middle a box of three fields in monospace: the session, the login and the
// password. Up and Down (or Tab) move between them, Left and Right choose
// the session or the person, typing goes into the password from anywhere,
// Enter logs in. F1 shut down, F2 restart, F3 sleep: each asks to confirm
// (Enter, or the same key again). A fingerprint reader's words show in the
// box. Everything also works by pointer. The window and the logging in:
// GreeterWindow.
GreeterWindow {
    id: win
    passwordField: input
    color: "#0B0C0E"

    readonly property string mono: "monospace"
    readonly property color fg: "#E6E9EF"
    readonly property color dim: "#8D96A5"
    readonly property color accent: "#9BBCFF"
    readonly property color bad: "#FF9C9C"
    property int row: 2 // 0 session, 1 login, 2 password

    readonly property var powerActions: [["F1", "shutdown", "poweroff"], ["F2", "reboot", "reboot"], ["F3", "sleep", "suspend"]]
    SystemClock { id: clock; precision: SystemClock.Minutes }

    // The power keys, and the time, along the top
    Row {
        visible: win.primary
        x: 16
        spacing: 24
        Repeater {
            model: win.powerActions
            Text {
                required property var modelData
                height: 44
                verticalAlignment: Text.AlignVCenter
                text: modelData[0] + " " + modelData[1]
                color: win.powerChoice === modelData[2] ? win.bad : powerMouse.containsMouse ? win.fg : win.dim
                font { family: win.mono; pixelSize: 14 }
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
    Text {
        visible: win.primary
        anchors { right: parent.right; rightMargin: 16 }
        height: 44
        verticalAlignment: Text.AlignVCenter
        text: Qt.formatDateTime(clock.date, "ddd d MMM  " + (win.h24 ? "HH:mm" : "h:mm AP"))
        color: win.dim
        font { family: win.mono; pixelSize: 14 }
    }

    // The box
    Rectangle {
        id: box
        visible: win.primary
        anchors.centerIn: parent
        width: Math.min(560, parent.width - 32)
        height: column.implicitHeight + 48
        color: "transparent"
        border.width: 1
        border.color: win.dim
        Column {
            id: column
            x: 32
            y: 24
            width: parent.width - 64
            spacing: 6
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                bottomPadding: 14
                text: win.auth.hostname || "iRice"
                color: win.fg
                font { family: win.mono; pixelSize: 16 }
            }
            Field {
                index: 0
                label: "session"
                value: "< " + (win.auth.sessions[win.auth.session]?.name ?? "Loading…") + " >"
                onStep: d => win.nextSession(d)
            }
            Field {
                index: 1
                label: "login"
                value: (win.auth.users.length > 1 ? "< " : "") + (win.auth.users[win.auth.user]?.name ?? "No users") + (win.auth.users.length > 1 ? " >" : "")
                onStep: d => win.nextUser(d)
            }
            // The password: the one text input, holding the keyboard throughout
            Item {
                width: parent.width
                height: 36
                Text {
                    width: 110
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: "password"
                    color: win.row === 2 ? win.accent : win.dim
                    font { family: win.mono; pixelSize: 15 }
                }
                TextInput {
                    id: input
                    x: 130
                    width: parent.width - x
                    height: parent.height
                    verticalAlignment: TextInput.AlignVCenter
                    color: win.fg
                    font { family: win.mono; pixelSize: 15 }
                    echoMode: TextInput.Password
                    passwordCharacter: "*"
                    readOnly: win.auth.busy || win.powerChoice !== ""
                    focus: win.primary
                    cursorVisible: win.row === 2
                    Component.onCompleted: if (win.primary) forceActiveFocus()
                    onAccepted: win.submit()
                    onTextChanged: if (text !== "") win.row = 2
                    Keys.onUpPressed: win.row = Math.max(0, win.row - 1)
                    Keys.onDownPressed: win.row = Math.min(2, win.row + 1)
                    Keys.onTabPressed: win.row = (win.row + 1) % 3
                    Keys.onBacktabPressed: win.row = (win.row + 2) % 3
                    Keys.onLeftPressed: event => {
                        if (win.row === 0) win.nextSession(-1);
                        else if (win.row === 1) win.nextUser(-1);
                        else event.accepted = false;
                    }
                    Keys.onRightPressed: event => {
                        if (win.row === 0) win.nextSession(1);
                        else if (win.row === 1) win.nextUser(1);
                        else event.accepted = false;
                    }
                    Keys.onEscapePressed: win.cancel()
                    Keys.onPressed: event => {
                        const actions = { [Qt.Key_F1]: "poweroff", [Qt.Key_F2]: "reboot", [Qt.Key_F3]: "suspend" };
                        if (actions[event.key] && !win.auth.busy) {
                            win.choosePower(actions[event.key]);
                            event.accepted = true;
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.IBeamCursor
                        onClicked: {
                            win.row = 2;
                            input.forceActiveFocus();
                        }
                    }
                }
            }
            // What's happening: a power choice to confirm, an error, the
            // fingerprint reader, signing in
            Row {
                topPadding: 14
                spacing: 8
                Text {
                    visible: win.auth.finger !== "" && !win.powerChoice
                    anchors.verticalCenter: parent.verticalCenter
                    text: Phosphor.glyphs["fingerprint"] ?? ""
                    color: win.auth.finger === "miss" ? win.bad : win.accent
                    font { family: "Phosphor"; pixelSize: 16 }
                }
                Text {
                    width: column.width - 24
                    wrapMode: Text.Wrap
                    text: win.powerChoice ? win.powerChoice + ": Enter (or the same key again) to confirm, Esc to cancel"
                        : win.auth.error || (win.auth.busy ? "logging in…" : win.auth.message || (win.auth.mock ? "preview: the password is irice, Esc quits" : "Enter to log in"))
                    color: win.auth.error || win.powerChoice ? win.bad : win.dim
                    font { family: win.mono; pixelSize: 13 }
                }
            }
        }
    }

    // A field: its label, its value; the arrows (or a click either side)
    // change it
    component Field: Item {
        id: field
        required property int index
        property string label
        property string value
        signal step(int d)
        width: parent.width
        height: 36
        Text {
            width: 110
            height: parent.height
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignRight
            text: field.label
            color: win.row === field.index ? win.accent : win.dim
            font { family: win.mono; pixelSize: 15 }
        }
        Text {
            x: 130
            height: parent.height
            verticalAlignment: Text.AlignVCenter
            text: field.value
            color: win.fg
            font { family: win.mono; pixelSize: 15 }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: m => {
                win.row = field.index;
                field.step(m.x < 130 + (width - 130) / 2 ? -1 : 1);
                input.forceActiveFocus();
            }
        }
    }
}
