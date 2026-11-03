import QtQuick
import Quickshell
import Quickshell.Widgets
import Quickshell.Wayland
import "../../common/phosphor.js" as Phosphor

// The plain login screen: black, the time small at the top; in the middle
// your picture in a circle, your name, and the password field, the
// fingerprint glyph at its end when a reader listens (its words under it).
// Up and Down (or the arrows by the name) choose the person, Tab the
// session (named under the field), Enter logs in. Sleep, restart and shut
// down sit in the bottom corner, each asking to confirm (a second click, or
// Enter). Authentication goes through greetd (GreeterAuth).
PanelWindow {
    id: win
    required property var auth
    property bool primary: false
    color: "black"
    WlrLayershell.namespace: "irice-greeter"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property string sans: "Inter"
    readonly property color fg: "#F2F2F3"
    readonly property color dim: "#9A9AA2"
    readonly property color field: "#1C1C1F"
    readonly property color bad: "#FF8A8A"
    readonly property color accent: "#A5B4FC"
    readonly property var person: auth.users[auth.user] ?? null
    property string powerChoice: ""

    function glyph(name) {
        return Phosphor.glyphs[name] ?? "";
    }
    function nextUser(step) {
        if (auth.busy || auth.users.length < 2)
            return;
        auth.user = (auth.user + auth.users.length + step) % auth.users.length;
        input.text = "";
    }
    function nextSession() {
        if (!auth.busy && auth.sessions.length)
            auth.session = (auth.session + 1) % auth.sessions.length;
    }
    function submit() {
        if (powerChoice) {
            const action = powerChoice;
            powerChoice = "";
            if (auth.mock)
                console.log("greeter (mock):", action);
            else
                Quickshell.execDetached(["systemctl", action]);
        } else if (!auth.busy) {
            auth.login(input.text);
        }
    }
    function choosePower(action) {
        if (auth.busy)
            return;
        if (powerChoice === action)
            submit();
        else
            powerChoice = action;
        input.forceActiveFocus();
    }
    onPrimaryChanged: if (primary) input.forceActiveFocus()
    Connections {
        target: win.auth
        function onErrorChanged() {
            if (win.auth.error) {
                input.text = "";
                shake.restart();
            }
        }
        function onUserChanged() { input.text = ""; }
    }
    SystemClock { id: clock; precision: SystemClock.Minutes }

    Text {
        visible: win.primary
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 24 }
        text: Qt.formatDateTime(clock.date, "dddd d MMMM   h:mm")
        color: win.dim
        font { family: win.sans; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
    }

    Column {
        visible: win.primary
        anchors.centerIn: parent
        spacing: 16

        // Your picture (AccountsService's), else your initial
        ClippingRectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 96
            height: 96
            radius: 48
            color: "#2A2A2E"
            Text {
                anchors.centerIn: parent
                visible: picture.status !== Image.Ready
                text: (win.person?.full ?? "?").charAt(0).toUpperCase()
                color: win.fg
                font { family: win.sans; pixelSize: 40; weight: Font.DemiBold }
            }
            Image {
                id: picture
                anchors.fill: parent
                source: win.auth.avatar(win.person?.name ?? "")
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(192, 192)
                asynchronous: true
                visible: status === Image.Ready
            }
        }
        // Your name, with arrows to the others
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 4
            Chevron { visible: win.auth.users.length > 1; name: "caret-left"; onClicked: win.nextUser(-1) }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: win.person?.full ?? "No users"
                color: win.fg
                font { family: win.sans; pixelSize: 20; weight: Font.DemiBold }
            }
            Chevron { visible: win.auth.users.length > 1; name: "caret-right"; onClicked: win.nextUser(1) }
        }
        // The field, the fingerprint at its end
        Rectangle {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 300
            height: 44
            radius: 22
            color: win.field
            border.width: input.activeFocus ? 1 : 0
            border.color: win.auth.error ? win.bad : "#3A3A40"
            property real dx: 0
            transform: Translate { x: fieldBox.dx }
            SequentialAnimation {
                id: shake
                NumberAnimation { target: fieldBox; property: "dx"; to: 10; duration: 50 }
                NumberAnimation { target: fieldBox; property: "dx"; to: -8; duration: 70 }
                NumberAnimation { target: fieldBox; property: "dx"; to: 4; duration: 70 }
                NumberAnimation { target: fieldBox; property: "dx"; to: 0; duration: 60 }
            }
            TextInput {
                id: input
                x: 18
                width: parent.width - 18 - (finger.visible ? 44 : 18)
                height: parent.height
                verticalAlignment: TextInput.AlignVCenter
                color: win.fg
                font { family: win.sans; pixelSize: 16; letterSpacing: 2 }
                echoMode: TextInput.Password
                passwordCharacter: "•"
                readOnly: win.auth.busy || win.powerChoice !== ""
                focus: win.primary
                clip: true
                Component.onCompleted: if (win.primary) forceActiveFocus()
                onAccepted: win.submit()
                Keys.onUpPressed: win.nextUser(-1)
                Keys.onDownPressed: win.nextUser(1)
                Keys.onTabPressed: win.nextSession()
                Keys.onEscapePressed: {
                    if (win.powerChoice !== "")
                        win.powerChoice = "";
                    else if (win.auth.mock)
                        win.auth.quitPreview();
                    else
                        text = "";
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    text: win.auth.busy ? "Logging in…" : "Password"
                    color: "#6B6B74"
                    font { family: win.sans; pixelSize: 15 }
                }
                Accessible.name: "Password"
            }
            Text {
                id: finger
                visible: win.auth.finger !== ""
                anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                text: win.glyph("fingerprint")
                color: win.auth.finger === "miss" ? win.bad : win.accent
                font { family: "Phosphor"; pixelSize: 22 }
                Accessible.role: Accessible.StaticText
                Accessible.name: win.auth.message || "Fingerprint"
            }
        }
        // Errors, the reader's words, a power choice to confirm
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 320
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: win.powerChoice ? ({ suspend: "Sleep", reboot: "Restart", poweroff: "Shut down" })[win.powerChoice] + ": click it again or press Enter. Esc cancels."
                : win.auth.error || win.auth.message || (win.auth.mock ? "Preview: the password is irice. Esc quits." : "")
            color: win.auth.error || win.powerChoice ? win.bad : win.dim
            font { family: win.sans; pixelSize: 13 }
        }
        // The session (Tab, or a click, for the next)
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            height: 32
            verticalAlignment: Text.AlignVCenter
            text: (win.auth.sessions[win.auth.session]?.name ?? "") + (win.auth.sessions.length > 1 ? "  ▾" : "")
            color: sessionMouse.containsMouse ? win.fg : win.dim
            font { family: win.sans; pixelSize: 13 }
            MouseArea {
                id: sessionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: win.nextSession()
            }
        }
    }

    // Sleep, restart, shut down
    Row {
        visible: win.primary
        anchors { right: parent.right; bottom: parent.bottom; margins: 24 }
        spacing: 8
        Repeater {
            model: [["moon", "suspend", "Sleep"], ["arrow-clockwise", "reboot", "Restart"], ["power", "poweroff", "Shut down"]]
            Rectangle {
                id: power
                required property var modelData
                width: 44
                height: 44
                radius: 22
                color: win.powerChoice === modelData[1] ? "#3A1F22" : powerMouse.containsMouse ? "#232327" : "transparent"
                Text {
                    anchors.centerIn: parent
                    text: win.glyph(power.modelData[0])
                    color: win.powerChoice === power.modelData[1] ? win.bad : win.dim
                    font { family: "Phosphor"; pixelSize: 20 }
                }
                MouseArea {
                    id: powerMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: win.choosePower(power.modelData[1])
                }
                Accessible.role: Accessible.Button
                Accessible.name: modelData[2]
                Accessible.onPressAction: win.choosePower(modelData[1])
            }
        }
    }

    component Chevron: Rectangle {
        id: chev
        property string name
        signal clicked
        anchors.verticalCenter: parent.verticalCenter
        width: 32
        height: 32
        radius: 16
        color: chevMouse.containsMouse ? "#232327" : "transparent"
        Text {
            anchors.centerIn: parent
            text: win.glyph(chev.name)
            color: win.dim
            font { family: "Phosphor"; pixelSize: 16 }
        }
        MouseArea {
            id: chevMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: chev.clicked()
        }
    }
}
