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

    // Material: a Material You clock, a scalloped cookie in the primary
    // container by day and the tertiary one at night, the hour hand in the
    // container's accent, the minute hand in its ink, a tertiary pivot
    readonly property color face: Theme.material ? (night ? Theme.m3.tertiaryContainer : Theme.m3.primaryContainer) : night ? "#1C1C1E" : "#F5F5F7"
    readonly property color ink: Theme.material ? (night ? Theme.m3.onTertiaryContainer : Theme.m3.onPrimaryContainer) : night ? "#F5F5F7" : "#1C1C1E"
    readonly property color hourInk: Theme.material ? (night ? Theme.m3.tertiary : Theme.m3.primary) : ink

    Rectangle {
        visible: !Theme.material
        anchors.fill: parent
        radius: width / 2
        color: root.face
    }
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: MaterialShape {
            shape: "cookie12"
            color: root.face
        }
    }
    // Hour ticks
    Repeater {
        model: 12
        Item {
            required property int index
            anchors.fill: parent
            rotation: index * 30
            // (Material: a dot at each quarter, inside the scallops)
            visible: !Theme.material || index % 3 === 0
            Rectangle {
                x: (root.width - width) / 2
                y: Theme.material ? root.width * 0.13 : root.width * 0.06
                width: Theme.material ? Math.max(3, root.width * 0.04) : index % 3 === 0 ? 2.5 : 1.5
                height: Theme.material ? width : root.width * (index % 3 === 0 ? 0.09 : 0.06)
                radius: Theme.material ? width / 2 : 1
                color: Qt.alpha(root.ink, Theme.material ? 0.6 : index % 3 === 0 ? 0.85 : 0.45)
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
            y: Theme.material ? root.height * 0.27 - width / 2 : root.height * 0.25
            width: Theme.material ? Math.max(5, root.width * 0.085) : Math.max(3, root.width * 0.045)
            height: Theme.material ? root.height * 0.23 + width : root.height * 0.27
            radius: width / 2
            color: root.hourInk
        }
    }
    Item {
        anchors.fill: parent
        rotation: root.minutes * 6
        Behavior on rotation { RotationAnimator { duration: 400; direction: RotationAnimator.Clockwise; easing.type: Easing.OutBack } }
        Rectangle {
            x: (root.width - width) / 2
            y: Theme.material ? root.height * 0.16 : root.height * 0.1
            width: Theme.material ? Math.max(3, root.width * 0.05) : Math.max(2, root.width * 0.03)
            height: Theme.material ? root.height * 0.34 + width / 2 : root.height * 0.42
            radius: width / 2
            color: root.ink
        }
    }
    Rectangle {
        anchors.centerIn: parent
        width: Theme.material ? Math.max(6, root.width * 0.06) : Math.max(6, root.width * 0.07)
        height: width
        radius: width / 2
        color: Theme.material ? (root.night ? Theme.m3.primary : Theme.m3.tertiary) : Theme.accent
        border.width: Theme.material ? 0 : 1.5
        border.color: root.face
    }
}
