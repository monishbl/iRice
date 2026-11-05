import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import "../../common/phosphor.js" as Phosphor

// The Glass login screen, as macOS's: your wallpaper, softly blurred, the
// date and a large time at the top; at the bottom your picture, your name
// and a frosted password field (the fingerprint glyph at its end while a
// reader listens: "Use Fingerprint or Enter Password"), the session under it; Sleep,
// Restart and Shut Down along the foot, each asking to confirm (a second
// click, or Enter). Up and Down choose the person, Tab the session. Kept
// light: plain translucent fills, no glass engine (it needs your session).
// The window and the logging in: GreeterWindow.
GreeterWindow {
    id: win
    passwordField: input
    nameField: nameInput
    onRefused: shake.restart()
    readonly property string sans: "Inter"

    function glyph(name) {
        return Phosphor.glyphs[name] ?? "";
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
                source: win.auth.avatar(win.pictureName)
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(144, 144)
                asynchronous: true
                visible: status === Image.Ready
            }
        }
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 4
            // (as wide as Switch user, so the name stays centred)
            Item { width: 28; height: 28 }
            Chevron { visible: win.auth.users.length > 1 && !win.auth.other; name: "caret-left"; onClicked: win.nextUser(-1) }
            GreeterNameLabel {
                win: win
                anchors.verticalCenter: parent.verticalCenter
                visible: !win.auth.other
                text: win.person?.full ?? ""
                textColor: "white"
                font { family: win.sans; pixelSize: 16; weight: Font.DemiBold }
                hoverFill: Qt.rgba(1, 1, 1, 0.12)
            }
            // A typed username
            Rectangle {
                visible: win.auth.other
                anchors.verticalCenter: parent.verticalCenter
                width: 240 - 2 * (28 + 4)
                height: 34
                radius: 17
                color: Qt.rgba(1, 1, 1, 0.18)
                border.width: nameInput.activeFocus ? 1 : 0
                border.color: Qt.rgba(1, 1, 1, 0.25)
                GreeterNameInput {
                    id: nameInput
                    win: win
                    x: 14
                    width: parent.width - 2 * 14
                    color: "white"
                    font { family: win.sans; pixelSize: 14 }
                    placeholderColor: Qt.rgba(1, 1, 1, 0.6)
                    placeholderSize: 13
                }
            }
            Chevron { visible: win.auth.users.length > 1 && !win.auth.other; name: "caret-right"; onClicked: win.nextUser(1) }
            Chevron { name: win.auth.other ? "x" : "user-switch"; onClicked: win.auth.other ? win.cancelName() : win.editName() }
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
                Keys.onEscapePressed: win.cancel()
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    text: win.auth.busy ? "Logging in…" : win.auth.finger !== "" ? "Use Fingerprint or Enter Password" : "Enter Password"
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
                : win.auth.error || win.auth.message || (win.auth.mock ? "Preview: the password is tessera. Esc quits." : "")
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
        Accessible.role: Accessible.Button
        Accessible.name: ({ "caret-left": "Previous person", "caret-right": "Next person", "user-switch": "Switch user", "x": "Back to the list" })[name] ?? name
        Accessible.onPressAction: chev.clicked()
    }
}
