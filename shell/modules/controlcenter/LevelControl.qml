import QtQuick
import qs.common

// A capsule with three stops (Off · 10 min · On), like the Control Center
// sliders: a white thumb slides to the chosen one. Click a stop, or drag the
// thumb along. Used by the Sharing module and its page.
//   LevelControl { value: Sharing.level; onPicked: v => Sharing.setLevel(v) }
Item {
    id: root

    property string value: "off"
    property var values: ["off", "10min", "everyone"]
    property var labels: ["Off", "10 min", "On"]
    property bool enabled_: true
    signal picked(string value)

    readonly property int index: Math.max(0, values.indexOf(value))
    readonly property real stop: width / values.length

    implicitWidth: 180
    implicitHeight: 36
    opacity: enabled_ ? 1 : 0.5

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: Theme.color.toggleOff
    }
    Rectangle {
        id: thumb
        x: drag.active ? Math.max(2, Math.min(root.width - width - 2, dragX)) : root.index * root.stop + 2
        y: 2
        width: root.stop - 4
        height: root.height - 4
        radius: height / 2
        color: Theme.accentOnFill
        property real dragX: 0
        Behavior on x {
            enabled: !drag.active
            SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.06; epsilon: 0.3 }
        }
    }
    Repeater {
        model: root.labels
        Text {
            required property string modelData
            required property int index
            x: index * root.stop
            width: root.stop
            height: root.height
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            text: modelData
            elide: Text.ElideRight
            // On the white thumb: the accent (or near-black, as Theme decides)
            color: index === root.index ? Theme.accentOnGlyph : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: index === root.index ? Font.DemiBold : Font.Normal }
        }
    }
    MouseArea {
        anchors.fill: parent
        enabled: root.enabled_
        cursorShape: Qt.PointingHandCursor
        preventStealing: true // a scrolling page around it mustn't take the drag
        onClicked: m => root.pick(Math.floor(m.x / root.stop))
    }
    DragHandler {
        id: drag
        target: null
        enabled: root.enabled_
        xAxis.enabled: true
        yAxis.enabled: false
        onActiveChanged: {
            if (active)
                thumb.dragX = root.index * root.stop + 2;
            else
                root.pick(Math.round((thumb.dragX - 2) / root.stop));
        }
        onTranslationChanged: thumb.dragX = root.index * root.stop + 2 + translation.x
    }
    function pick(i) {
        const v = values[Math.max(0, Math.min(values.length - 1, i))];
        if (v !== value)
            picked(v);
    }

    Keys.onLeftPressed: pick(index - 1)
    Keys.onRightPressed: pick(index + 1)
    activeFocusOnTab: true
}
