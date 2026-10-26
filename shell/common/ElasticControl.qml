import QtQuick

// Elastic feel for a small control, on press: it squishes, and while held it
// follows the pointer a little (rubber-banded), springing back on release.
// Nothing happens on hover. Drop it inside the control:
//   ElasticControl { id: elastic }  and  transform: elastic.effects
//   (optionally set `pressed` from the control's MouseArea)
Item {
    id: root

    anchors.fill: parent
    property bool pressed: false
    readonly property var effects: [translate, scale]

    // Passive: sees presses without taking them from the control's MouseArea
    PointHandler { id: point; acceptedButtons: Qt.LeftButton }
    readonly property bool held: point.active || pressed
    readonly property real limit: Theme.motion.controlPull * 1.6
    function rubber(d) {
        return Math.sign(d) * limit * (1 - Math.exp(-Math.abs(d) / (limit * 5)));
    }
    property real px: point.active ? rubber(point.point.scenePosition.x - point.point.scenePressPosition.x) : 0
    property real py: point.active ? rubber(point.point.scenePosition.y - point.point.scenePressPosition.y) : 0
    property real s: held ? Theme.motion.squish : 1
    Behavior on px { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }
    Behavior on py { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping; epsilon: 0.05 } }
    Behavior on s { SpringAnimation { spring: 6; damping: Theme.motion.damping + 0.05; epsilon: 0.002 } }

    Translate { id: translate; x: root.px; y: root.py }
    Scale { id: scale; origin.x: root.width / 2; origin.y: root.height / 2; xScale: root.s; yScale: root.s }
}
