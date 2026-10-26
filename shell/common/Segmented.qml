import QtQuick

// Segmented control: a track with a sliding selection pill.
Rectangle {
    id: root

    property var options: [] // labels
    property int current: 0
    signal selected(int index)

    implicitHeight: 30
    radius: height / 2
    color: Theme.color.toggleOff

    readonly property real segment: (width - 4) / Math.max(1, options.length)

    Rectangle {
        x: 2 + root.current * root.segment
        y: 2
        width: root.segment
        height: root.height - 4
        radius: height / 2
        color: Theme.dark ? Qt.rgba(1, 1, 1, 0.2) : "white"
        Behavior on x { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
    }

    Row {
        x: 2
        Repeater {
            model: root.options
            Item {
                required property string modelData
                required property int index
                width: root.segment
                height: root.height
                Text {
                    anchors.centerIn: parent
                    text: parent.modelData
                    color: parent.index === root.current ? Theme.color.label : Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.selected(parent.index)
                }
            }
        }
    }
}
