import QtQuick
import qs.common
import qs.services

// One of Resources' stats with its last minute: the name and current value
// above its graph, or (`compact`) one line with a small graph between them.
//   ResourceStat { key: "cpu"; width: 240; graphHeight: 48 }
// Showing one keeps sampling at once a second (Resources.graphUsers).
Item {
    id: root

    property string key: "cpu"
    property bool compact: false
    property int graphHeight: 32
    property bool showDetail: false // e.g. "6.1 of 15.4 GB" after the value
    readonly property var stat: Resources.stat(key)
    readonly property color tone: Theme.accents[stat.tone] ?? Theme.accent

    property bool counted: false
    function count() {
        const want = visible;
        if (want !== counted) {
            Resources.graphUsers += want ? 1 : -1;
            counted = want;
        }
    }
    onVisibleChanged: count()
    Component.onCompleted: count()
    Component.onDestruction: if (counted) Resources.graphUsers--

    implicitWidth: 200
    implicitHeight: compact ? 20 : head.height + 6 + graphHeight

    Item {
        id: head
        width: parent.width
        height: 16
        anchors.verticalCenter: root.compact ? parent.verticalCenter : undefined
        Text {
            id: name
            anchors.verticalCenter: parent.verticalCenter
            width: root.compact ? Math.min(implicitWidth, 84) : implicitWidth
            elide: Text.ElideRight
            text: root.stat.label
            color: root.compact ? Theme.color.secondary : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: root.compact ? Font.Normal : Font.DemiBold }
        }
        Text {
            id: value
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            text: root.stat.text + (root.showDetail && root.stat.detail !== "" ? "  ·  " + root.stat.detail : "")
            color: root.compact ? Theme.color.label : Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
        }
        // Compact: the graph between the name and the value
        ResourceGraph {
            visible: root.compact
            anchors { left: parent.left; leftMargin: 88; right: parent.right; rightMargin: 72; verticalCenter: parent.verticalCenter }
            height: 14
            lineWidth: 1.25
            values: root.stat.history
            maximum: root.stat.max
            color: root.tone
        }
    }
    ResourceGraph {
        visible: !root.compact
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: root.graphHeight
        values: root.stat.history
        maximum: root.stat.max
        color: root.tone
    }
}
