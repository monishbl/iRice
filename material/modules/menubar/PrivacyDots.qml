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
    implicitWidth: chip.width + 16

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
