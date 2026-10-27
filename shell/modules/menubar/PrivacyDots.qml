import QtQuick
import qs.common
import qs.services

// The menu bar's privacy indicator, like macOS: an orange dot while the
// microphone is in use, green for the camera, purple while the screen is
// shared. Click for which apps.
MenuBarItem {
    id: root

    visible: Privacy.any
    menu: "privacyMenu"
    text: " " // the dots are drawn below; this keeps the item's height and padding
    implicitWidth: dots.implicitWidth + 16

    Row {
        id: dots
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            model: [
                { on: Privacy.camera, color: "#30D158" },
                { on: Privacy.mic, color: "#FF9F0A" },
                { on: Privacy.screenShared, color: "#BF5AF2" }
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
}
