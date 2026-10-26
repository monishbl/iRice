import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// One screen of the lock: wallpaper, large date and time up top, avatar,
// name and a glass password field at the bottom.
WlSessionLockSurface {
    id: surface

    required property var lockScope
    // Glass host contract (see Glass.qml)
    readonly property var backdrop: backdropItem.texture
    readonly property real screenX: 0
    readonly property real screenY: 0

    color: "black"

    Backdrop {
        id: backdropItem
        screen: surface.screen
    }

    Image {
        id: wallpaper
        anchors.fill: parent
        source: Wallpaper.source
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(width, height)
        asynchronous: true
    }

    property real reveal: 0
    Component.onCompleted: reveal = 1
    Behavior on reveal { NumberAnimation { duration: Theme.motion.large; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }

    SystemClock { id: clock; precision: SystemClock.Minutes }

    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * 0.1 + (1 - surface.reveal) * -20
        opacity: surface.reveal
        spacing: 0

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: Qt.rgba(1, 1, 1, 0.85)
            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold }
        }
        Text {
            id: time
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
        id: login
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: parent.height * 0.1
        opacity: surface.reveal
        spacing: 10

        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64
            height: 64
            radius: 32
            color: Qt.rgba(1, 1, 1, 0.18)
            Icon { anchors.centerIn: parent; name: "user"; fill: true; size: 32; color: Qt.rgba(1, 1, 1, 0.9) }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Quickshell.env("USER") ?? ""
            color: "white"
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
        }

        Item {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 240 + 2 * glass.shadowMargin
            height: 36 + 2 * glass.shadowMargin

            property real shake: 0
            SequentialAnimation {
                id: shakeAnim
                loops: 1
                NumberAnimation { target: fieldBox; property: "shake"; to: 10; duration: 50 }
                NumberAnimation { target: fieldBox; property: "shake"; to: -10; duration: 80 }
                NumberAnimation { target: fieldBox; property: "shake"; to: 6; duration: 70 }
                NumberAnimation { target: fieldBox; property: "shake"; to: 0; duration: 60 }
            }
            Connections {
                target: surface.lockScope
                function onErrorChanged() {
                    if (surface.lockScope.error !== "")
                        shakeAnim.restart();
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
                enabled: !surface.lockScope.busy
                focus: true
                color: "white"
                font { family: Theme.font.family; pixelSize: Theme.font.title; letterSpacing: 1 }
                text: surface.lockScope.password
                onTextChanged: surface.lockScope.password = text
                onAccepted: surface.lockScope.submit()
                Component.onCompleted: forceActiveFocus()

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
                visible: !surface.lockScope.busy
                name: "arrow-circle-right"
                fill: true
                size: 22
                color: Qt.rgba(1, 1, 1, input.text === "" ? 0.35 : 0.9)
                MouseArea { anchors.fill: parent; onClicked: surface.lockScope.submit() }
            }
            Icon {
                x: glass.x + glass.shadowMargin + 240 - 30
                y: glass.shadowMargin + 6
                width: 24
                height: 24
                visible: surface.lockScope.busy
                name: "circle-notch"
                size: 22
                color: Qt.rgba(1, 1, 1, 0.9)
                RotationAnimation on rotation { running: surface.lockScope.busy; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: surface.lockScope.error
            visible: text !== ""
            color: "white"
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
    }
}
