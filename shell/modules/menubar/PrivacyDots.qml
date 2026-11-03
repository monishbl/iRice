import QtQuick
import qs.common
import qs.services

// The menu bar's privacy indicator, like macOS: an orange dot while the
// microphone is in use, green for the camera, blue for your location,
// purple while the screen is shared (when Settings → Menu Bar puts it here
// and not only in the island). Click for which apps. Material: Android's
// privacy chip instead, a pill holding the icons of what's in use.
MenuBarItem {
    id: root

    visible: Privacy.any
    menu: "privacyMenu"
    text: " " // the dots are drawn below; this keeps the item's height and padding
    implicitWidth: (Theme.material ? chip.width : dots.implicitWidth) + 16

    Row {
        id: dots
        visible: !Theme.material
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            model: Theme.material ? [] : [
                { on: Privacy.camera, color: "#30D158" },
                { on: Privacy.mic, color: "#FF9F0A" },
                { on: Privacy.location, color: "#0A84FF" },
                { on: Privacy.shareDot, color: "#BF5AF2" }
            ].filter(d => d.on)
            Rectangle {
                required property var modelData
                width: 7
                height: 7
                radius: 3.5
                color: modelData.color
                // Appears with a small pop
                scale: 0
                Component.onCompleted: scale = 1
                // (a NumberAnimation: started as it's created, a ScaleAnimator may
                // never run and leave the dot invisible)
                Behavior on scale { NumberAnimation { duration: 200; easing.type: Easing.OutBack } }
            }
        }
    }

    Rectangle {
        id: chip
        visible: Theme.material
        anchors.centerIn: parent
        width: chipIcons.implicitWidth + 12
        height: 20
        radius: 10
        color: Theme.material ? Theme.m3.tertiaryContainer : "transparent"
        Row {
            id: chipIcons
            anchors.centerIn: parent
            spacing: 2
            Repeater {
                model: Theme.material ? [
                    { on: Privacy.camera, icon: "video-camera" },
                    { on: Privacy.mic, icon: "microphone" },
                    { on: Privacy.location, icon: "navigation-arrow" },
                    { on: Privacy.shareDot, icon: "screencast" }
                ].filter(d => d.on) : []
                Icon {
                    required property var modelData
                    name: modelData.icon
                    fill: true
                    size: 12
                    color: Theme.m3.onTertiaryContainer
                }
            }
        }
    }
}
