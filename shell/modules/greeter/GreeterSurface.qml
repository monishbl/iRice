import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import Quickshell.Wayland
import "../../common/phosphor.js" as Phosphor

// The Glass login screen, as macOS's: your wallpaper, softly blurred, the
// date and a large time at the top; at the bottom your picture, your name
// and a frosted password field (the fingerprint glyph at its end while a
// reader listens: "Touch ID or Enter Password"), the session under it; Sleep,
// Restart and Shut Down along the foot, each asking to confirm (a second
// click, or Enter). Up and Down choose the person, Tab the session. Kept
// light: plain translucent fills, no glass engine (it needs your session).
// Authentication goes through greetd (GreeterAuth).
PanelWindow {
    id: win
    // 24-hour time (Settings → Appearance), passed in by greeter-start.sh
    readonly property bool h24: Quickshell.env("IRICE_GREETER_CLOCK24") === "1"
    required property var auth
    property bool primary: false
    color: "black"
    WlrLayershell.namespace: "irice-greeter"
    WlrLayershell.layer: auth.mock ? WlrLayer.Overlay : WlrLayer.Top // (a preview covers the menu bar too)
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property string sans: "Inter"
    readonly property var person: auth.users[auth.user] ?? null
    readonly property string wallpaper: Quickshell.env("IRICE_GREETER_WALLPAPER") || "/usr/share/irice/wallpaper.jpg"
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

    // No wallpaper: a clean dark background (the lock screen, once you're
    // in, keeps your wallpaper)
    Rectangle {
        anchors.fill: parent
        color: "#0E0E10"
    }

    // The date and the time
    Column {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: Math.round(win.height * 0.16) }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: Qt.rgba(1, 1, 1, 0.75)
            font { family: win.sans; pixelSize: 24; weight: Font.DemiBold }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: (win.h24 ? String(clock.date.getHours()).padStart(2, "0") : (clock.date.getHours() + 11) % 12 + 1) + ":" + String(clock.date.getMinutes()).padStart(2, "0")
            color: Qt.rgba(1, 1, 1, 0.85)
            font { family: win.sans; pixelSize: win.primary ? 150 : 120; weight: Font.Bold; letterSpacing: -5; features: { "tnum": 1 } }
        }
    }

    // You, and the field
    Column {
        visible: win.primary
        anchors { horizontalCenter: parent.horizontalCenter; bottom: powers.top; bottomMargin: Math.round(win.height * 0.06) }
        spacing: 10

        ClippingRectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 72
            height: 72
            radius: 36
            color: Qt.rgba(1, 1, 1, 0.2)
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.25)
            Text {
                anchors.centerIn: parent
                visible: picture.status !== Image.Ready
                text: win.glyph("user")
                color: "white"
                font { family: "Phosphor-Fill"; pixelSize: 34 }
            }
            Image {
                id: picture
                anchors.fill: parent
                source: win.auth.avatar(win.person?.name ?? "")
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(144, 144)
                asynchronous: true
                visible: status === Image.Ready
            }
        }
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 4
            Chevron { visible: win.auth.users.length > 1; name: "caret-left"; onClicked: win.nextUser(-1) }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: win.person?.full ?? "No users"
                color: "white"
                font { family: win.sans; pixelSize: 16; weight: Font.DemiBold }
            }
            Chevron { visible: win.auth.users.length > 1; name: "caret-right"; onClicked: win.nextUser(1) }
        }
        // The frosted field
        Rectangle {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 240
            height: 34
            radius: 17
            color: Qt.rgba(1, 1, 1, 0.18)
            border.width: 1
            border.color: win.auth.error ? Qt.rgba(1, 0.55, 0.55, 0.8) : Qt.rgba(1, 1, 1, 0.25)
            property real dx: 0
            transform: Translate { x: fieldBox.dx }
            SequentialAnimation {
                id: shake
                NumberAnimation { target: fieldBox; property: "dx"; to: 12; duration: 50 }
                NumberAnimation { target: fieldBox; property: "dx"; to: -10; duration: 70 }
                NumberAnimation { target: fieldBox; property: "dx"; to: 5; duration: 70 }
                NumberAnimation { target: fieldBox; property: "dx"; to: 0; duration: 60 }
            }
            TextInput {
                id: input
                x: 14
                width: parent.width - 14 - 36
                height: parent.height
                verticalAlignment: TextInput.AlignVCenter
                color: "white"
                font { family: win.sans; pixelSize: 14; letterSpacing: 2 }
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
                    text: win.auth.busy ? "Logging in…" : win.auth.finger !== "" ? "Touch ID or Enter Password" : "Enter Password"
                    color: Qt.rgba(1, 1, 1, 0.6)
                    font { family: win.sans; pixelSize: 13 }
                }
                Accessible.name: "Password"
            }
            // The fingerprint while the reader listens; else the arrow, once
            // there's something to send
            Rectangle {
                anchors { right: parent.right; rightMargin: 5; verticalCenter: parent.verticalCenter }
                width: 24
                height: 24
                radius: 12
                visible: win.auth.finger !== "" || input.text !== ""
                color: input.text !== "" ? Qt.rgba(1, 1, 1, sendMouse.containsMouse ? 0.45 : 0.3) : "transparent"
                Text {
                    anchors.centerIn: parent
                    text: win.glyph(input.text !== "" ? "arrow-right" : "fingerprint")
                    color: input.text === "" && win.auth.finger === "miss" ? "#FF9C9C" : "white"
                    font { family: input.text !== "" ? "Phosphor-Bold" : "Phosphor"; pixelSize: input.text !== "" ? 13 : 18 }
                }
                MouseArea {
                    id: sendMouse
                    anchors.fill: parent
                    enabled: input.text !== ""
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: win.submit()
                }
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 320
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            visible: text !== ""
            text: win.powerChoice ? ({ suspend: "Sleep", reboot: "Restart", poweroff: "Shut Down" })[win.powerChoice] + ": click it again or press Return. Esc cancels."
                : win.auth.error || win.auth.message || (win.auth.mock ? "Preview: the password is irice. Esc quits." : "")
            color: win.auth.error || win.powerChoice ? "#FFB4B4" : Qt.rgba(1, 1, 1, 0.75)
            font { family: win.sans; pixelSize: 12; weight: Font.Medium }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            height: 28
            verticalAlignment: Text.AlignVCenter
            text: (win.auth.sessions[win.auth.session]?.name ?? "") + (win.auth.sessions.length > 1 ? "  ▾" : "")
            color: Qt.rgba(1, 1, 1, sessionMouse.containsMouse ? 0.9 : 0.6)
            font { family: win.sans; pixelSize: 12 }
            MouseArea {
                id: sessionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: win.nextSession()
            }
        }
    }

    // Sleep, Restart, Shut Down along the foot
    Row {
        id: powers
        visible: win.primary
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 32 }
        spacing: 36
        Repeater {
            model: [["moon", "suspend", "Sleep"], ["arrow-clockwise", "reboot", "Restart"], ["power", "poweroff", "Shut Down"]]
            Column {
                id: power
                required property var modelData
                readonly property bool chosen: win.powerChoice === modelData[1]
                spacing: 6
                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 44
                    height: 44
                    radius: 22
                    color: power.chosen ? Qt.rgba(1, 0.4, 0.4, 0.5) : Qt.rgba(1, 1, 1, powerMouse.containsMouse ? 0.3 : 0.18)
                    border.width: 1
                    border.color: Qt.rgba(1, 1, 1, 0.22)
                    Text {
                        anchors.centerIn: parent
                        text: win.glyph(power.modelData[0])
                        color: "white"
                        font { family: "Phosphor-Bold"; pixelSize: 20 }
                    }
                    MouseArea {
                        id: powerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: win.choosePower(power.modelData[1])
                    }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: power.modelData[2]
                    color: Qt.rgba(1, 1, 1, 0.8)
                    font { family: win.sans; pixelSize: 12; weight: Font.Medium }
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
        width: 28
        height: 28
        radius: 14
        color: chevMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.18) : "transparent"
        Text {
            anchors.centerIn: parent
            text: win.glyph(chev.name)
            color: Qt.rgba(1, 1, 1, 0.8)
            font { family: "Phosphor"; pixelSize: 14 }
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
