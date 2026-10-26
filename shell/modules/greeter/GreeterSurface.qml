import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass

// The login screen, styled like the lock screen: date and time up top; user,
// password and session at the bottom; sleep, restart and shut down in the
// corner. The wallpaper and glass backdrop come from the Background window
// underneath. Keyboard input goes to the primary screen.
GlassWindow {
    id: win

    required property var auth
    property bool primary: false
    name: "greeter"

    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property var current: auth.users[auth.user] ?? null
    property real reveal: 0
    Component.onCompleted: reveal = 1
    Behavior on reveal { NumberAnimation { duration: Theme.motion.large; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }

    SystemClock { id: clock; precision: SystemClock.Minutes }

    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * 0.1 + (1 - win.reveal) * -20
        opacity: win.reveal
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: Qt.rgba(1, 1, 1, 0.85)
            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatTime(clock.date, "h:mm")
            color: Qt.rgba(1, 1, 1, 0.82)
            font { family: Theme.font.family; pixelSize: 150; weight: Font.Bold; letterSpacing: -4; features: { "tnum": 1 } }
            layer.enabled: true
            layer.effect: MultiEffect {
                shadowEnabled: true
                shadowColor: Qt.rgba(0, 0, 0, 0.3)
                shadowBlur: 0.6
                shadowVerticalOffset: 2
            }
        }
    }

    Column {
        visible: win.primary
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: parent.height * 0.1
        opacity: win.reveal
        spacing: 10

        // Avatar: initials on a quiet disc
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64
            height: 64
            radius: 32
            color: Qt.rgba(1, 1, 1, 0.18)
            Text {
                anchors.centerIn: parent
                text: (win.current?.full ?? "?").split(/\s+/).map(w => w.charAt(0)).slice(0, 2).join("").toUpperCase()
                color: Qt.rgba(1, 1, 1, 0.92)
                font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold }
            }
        }

        // Name; click (or Up/Down) to switch user
        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: nameRow.implicitWidth
            height: nameRow.implicitHeight
            Row {
                id: nameRow
                spacing: 6
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: win.current?.full ?? "No users"
                    color: "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: win.auth.users.length > 1
                    name: "caret-up-down"
                    bold: true
                    size: 12
                    color: Qt.rgba(1, 1, 1, 0.7)
                }
            }
            MouseArea {
                anchors.fill: parent
                enabled: win.auth.users.length > 1
                cursorShape: Qt.PointingHandCursor
                onClicked: win.auth.user = (win.auth.user + 1) % win.auth.users.length
            }
        }

        Item {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 240 + 2 * glass.shadowMargin
            height: 36 + 2 * glass.shadowMargin

            property real shake: 0
            SequentialAnimation {
                id: shakeAnim
                NumberAnimation { target: fieldBox; property: "shake"; to: 10; duration: 50 }
                NumberAnimation { target: fieldBox; property: "shake"; to: -10; duration: 80 }
                NumberAnimation { target: fieldBox; property: "shake"; to: 6; duration: 70 }
                NumberAnimation { target: fieldBox; property: "shake"; to: 0; duration: 60 }
            }
            Connections {
                target: win.auth
                function onErrorChanged() {
                    if (win.auth.error !== "") {
                        shakeAnim.restart();
                        input.text = "";
                    }
                }
            }

            Glass {
                id: glass
                x: fieldBox.shake
                width: parent.width
                height: parent.height
                radius: 18
                bevel: 14
                shadowMargin: 16
                onXChanged: updatePosition()
            }

            TextInput {
                id: input
                x: glass.x + glass.shadowMargin + 16
                y: glass.shadowMargin
                width: 240 - 32 - 24
                height: 36
                verticalAlignment: TextInput.AlignVCenter
                echoMode: TextInput.Password
                passwordCharacter: "●"
                enabled: !win.auth.busy
                focus: win.primary
                color: "white"
                font { family: Theme.font.family; pixelSize: Theme.font.title; letterSpacing: 1 }
                onAccepted: win.auth.login(text)
                Keys.onUpPressed: win.auth.user = (win.auth.user + win.auth.users.length - 1) % Math.max(1, win.auth.users.length)
                Keys.onDownPressed: win.auth.user = (win.auth.user + 1) % Math.max(1, win.auth.users.length)
                Component.onCompleted: if (win.primary) forceActiveFocus()

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    text: "Enter Password"
                    color: Qt.rgba(1, 1, 1, 0.55)
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }

            Icon {
                x: glass.x + glass.shadowMargin + 240 - 30
                y: glass.shadowMargin + 6
                width: 24
                height: 24
                name: win.auth.busy ? "circle-notch" : "arrow-circle-right"
                fill: !win.auth.busy
                size: 22
                color: Qt.rgba(1, 1, 1, win.auth.busy || input.text !== "" ? 0.9 : 0.35)
                RotationAnimation on rotation { running: win.auth.busy; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
                MouseArea { anchors.fill: parent; onClicked: win.auth.login(input.text) }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: win.auth.error
            visible: text !== ""
            color: "white"
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: win.auth.mock && win.auth.error === ""
            text: "Preview mode: the password is “irice”"
            color: Qt.rgba(1, 1, 1, 0.6)
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }

    // Bottom corners: session on the left, power on the right
    Row {
        visible: win.primary
        anchors { left: parent.left; bottom: parent.bottom; margins: 24 }
        opacity: win.reveal
        spacing: 8
        Rectangle {
            height: 32
            width: sessionRow.implicitWidth + 24
            radius: 16
            color: sessionMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.22) : Qt.rgba(1, 1, 1, 0.12)
            Row {
                id: sessionRow
                anchors.centerIn: parent
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "desktop"; size: 14; color: "white" }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: win.auth.sessions[win.auth.session]?.name ?? "Session"
                    color: "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                Icon { anchors.verticalCenter: parent.verticalCenter; visible: win.auth.sessions.length > 1; name: "caret-up-down"; bold: true; size: 11; color: Qt.rgba(1, 1, 1, 0.7) }
            }
            MouseArea {
                id: sessionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: win.auth.session = (win.auth.session + 1) % Math.max(1, win.auth.sessions.length)
            }
        }
    }

    Row {
        visible: win.primary
        anchors { right: parent.right; bottom: parent.bottom; margins: 24 }
        opacity: win.reveal
        spacing: 12
        Repeater {
            model: [
                { icon: "moon", label: "Sleep", cmd: ["systemctl", "suspend"] },
                { icon: "arrow-clockwise", label: "Restart", cmd: ["systemctl", "reboot"] },
                { icon: "power", label: "Shut Down", cmd: ["systemctl", "poweroff"] }
            ]
            Column {
                required property var modelData
                spacing: 6
                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 44
                    height: 44
                    radius: 22
                    color: powerMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : powerMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.22) : Qt.rgba(1, 1, 1, 0.12)
                    Icon { anchors.centerIn: parent; name: parent.parent.modelData.icon; size: 20; color: "white" }
                    MouseArea {
                        id: powerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: win.auth.mock ? console.log("greeter (mock):", parent.parent.modelData.cmd.join(" ")) : Quickshell.execDetached(parent.parent.modelData.cmd)
                    }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: parent.modelData.label
                    color: Qt.rgba(1, 1, 1, 0.85)
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                }
            }
        }
    }
}
