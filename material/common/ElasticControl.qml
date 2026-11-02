import QtQuick

// A small control's press in the Material look: a slight squish on press,
// no rubber-band follow (Material doesn't pull; presses change shape). The
// same interface as the Glass one (common/ElasticControl.qml), whose rubber
// band divides by the pull: at 0 a pressed control's position became NaN.
//   ElasticControl { id: elastic }  and  transform: elastic.effects
Item {
    id: root

    anchors.fill: parent
    property bool pressed: false
    readonly property var effects: [translate, scale]

    PointHandler { id: point; acceptedButtons: Qt.LeftButton }
    readonly property bool held: point.active || pressed
    readonly property real px: 0
    readonly property real py: 0
    property real s: held ? Theme.motion.squish : 1
    Behavior on s { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }

    Translate { id: translate }
    Scale { id: scale; origin.x: root.width / 2; origin.y: root.height / 2; xScale: root.s; yScale: root.s }
}
