import QtQuick
import qs.common

// An analog clock face: hour ticks, hour and minute hands. Time comes from
// `hours` and `minutes`; nothing redraws between minutes.
Item {
    id: root

    property int hours: 0
    property int minutes: 0
    property bool night: hours < 6 || hours >= 18 // dark face at night, like macOS
    implicitWidth: 120
    implicitHeight: 120

    readonly property color face: night ? "#1C1C1E" : "#F5F5F7"
    readonly property color ink: night ? "#F5F5F7" : "#1C1C1E"

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: root.face
    }
    // Hour ticks
    Repeater {
        model: 12
        Item {
            required property int index
            anchors.fill: parent
            rotation: index * 30
            Rectangle {
                x: (root.width - width) / 2
                y: root.width * 0.06
                width: index % 3 === 0 ? 2.5 : 1.5
                height: root.width * (index % 3 === 0 ? 0.09 : 0.06)
                radius: 1
                color: Qt.alpha(root.ink, index % 3 === 0 ? 0.85 : 0.45)
            }
        }
    }
    // Hands
    Item {
        anchors.fill: parent
        rotation: (root.hours % 12) * 30 + root.minutes * 0.5
        Behavior on rotation { RotationAnimator { duration: 400; direction: RotationAnimator.Clockwise; easing.type: Easing.OutBack } }
        Rectangle {
            x: (root.width - width) / 2
            y: root.height * 0.25
            width: Math.max(3, root.width * 0.045)
            height: root.height * 0.27
            radius: width / 2
            color: root.ink
        }
    }
    Item {
        anchors.fill: parent
        rotation: root.minutes * 6
        Behavior on rotation { RotationAnimator { duration: 400; direction: RotationAnimator.Clockwise; easing.type: Easing.OutBack } }
        Rectangle {
            x: (root.width - width) / 2
            y: root.height * 0.1
            width: Math.max(2, root.width * 0.03)
            height: root.height * 0.42
            radius: width / 2
            color: root.ink
        }
    }
    Rectangle {
        anchors.centerIn: parent
        width: Math.max(6, root.width * 0.07)
        height: width
        radius: width / 2
        color: Theme.accent
        border.width: 1.5
        border.color: root.face
    }
}
