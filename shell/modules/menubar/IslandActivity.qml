import QtQuick
import Quickshell
import qs.common
import qs.services

// One live activity in the expanded island, with its controls. Fades and
// slides in after `delay` ms, so rows stagger as the island opens.
Item {
    id: root

    required property var activity
    property int delay: 0
    property bool shown: false
    readonly property string kind: activity.kind
    readonly property var player: Island.player

    implicitHeight: kind === "music" ? 52 : LiveInfo.progress(kind) >= 0 ? 42 : 36

    property real appear: shown ? 1 : 0
    Behavior on appear {
        SequentialAnimation {
            PauseAnimation { duration: root.shown ? root.delay : 0 }
            NumberAnimation { duration: 280; easing.type: Easing.OutCubic }
        }
    }
    opacity: appear
    transform: Translate { y: (1 - root.appear) * 8 }

    // Leading art or icon
    Rectangle {
        id: lead
        width: root.kind === "music" ? 48 : 30
        height: width
        anchors.verticalCenter: parent.verticalCenter
        radius: root.kind === "music" ? 10 : width / 2
        color: root.kind === "music" ? Qt.rgba(1, 1, 1, 0.12) : Qt.alpha(root.activity.color, 0.22)
        clip: true
        Image {
            anchors.fill: parent
            visible: root.kind === "music" && (root.player?.trackArtUrl ?? "") !== ""
            source: root.kind === "music" ? (root.player?.trackArtUrl ?? "") : ""
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(96, 96)
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !(root.kind === "music" && (root.player?.trackArtUrl ?? "") !== "")
            name: root.activity.icon
            fill: true
            size: 15
            color: root.activity.color
        }
    }

    Column {
        anchors { left: lead.right; leftMargin: 12; right: controls.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        spacing: 1
        Text {
            width: parent.width
            text: LiveInfo.title(root.kind)
            elide: Text.ElideRight
            color: "#F5F5F7"
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            text: LiveInfo.detail(root.kind)
            elide: Text.ElideRight
            color: Qt.rgba(1, 1, 1, 0.6)
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
        }
        // Timer and transfer progress
        Rectangle {
            readonly property real p: LiveInfo.progress(root.kind)
            visible: p >= 0 && root.kind !== "music"
            width: parent.width
            height: 3
            radius: 1.5
            color: Qt.rgba(1, 1, 1, 0.16)
            Rectangle {
                width: parent.width * Math.max(0, parent.p)
                height: parent.height
                radius: 1.5
                color: root.activity.color
                Behavior on width { NumberAnimation { duration: 900 } }
            }
        }
    }

    Row {
        id: controls
        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
        spacing: 2
        Repeater {
            model: LiveInfo.controls(root.kind).filter(b => b.piece !== "reset")
            IconButton {
                required property var modelData
                width: 32
                height: 32
                icon: modelData.icon
                fill: modelData.fill
                iconSize: 16
                iconColor: modelData.danger ? Theme.accents.red : "#F5F5F7"
                enabled: modelData.on
                opacity: enabled ? 1 : 0.4
                onClicked: modelData.act()
            }
        }
    }
}
