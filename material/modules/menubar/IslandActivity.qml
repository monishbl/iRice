import QtQuick
import Quickshell
import qs.common
import qs.modules.captions
import qs.services

// One live activity in the expanded island, with its controls. Fades and
// slides in after `delay` ms, so rows stagger as the island opens.
// Material: the icon in a primary circle (error when urgent), the island's
// colours (IslandInk), wavy progress.
Item {
    id: root

    required property var activity
    property int delay: 0
    property bool shown: false
    readonly property string kind: activity.kind
    readonly property var player: Island.player

    readonly property bool hasTimeline: kind === "music" && (player?.lengthSupported ?? false) && (player?.length ?? 0) > 0
    implicitHeight: kind === "music" ? (hasTimeline ? 60 : 52) : kind === "captions" ? Math.max(36, info.implicitHeight + 8) : LiveInfo.progress(kind) >= 0 ? 42 : 36

    // MPRIS doesn't push the position; ask for it while the row shows
    Timer {
        running: root.visible && root.hasTimeline && Island.musicPlaying
        repeat: true
        interval: 1000
        onTriggered: root.player?.positionChanged()
    }

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
    Squircle {
        id: lead
        width: root.kind === "music" ? 48 : 30
        height: width
        anchors.verticalCenter: parent.verticalCenter
        radius: root.kind === "music" ? 10 : width / 2
        color: root.kind === "music" ? IslandInk.dim(0.12) : Theme.material ? IslandInk.tint(root.activity.color) : Qt.alpha(root.activity.color, 0.22)
        clip: true
        Image {
            anchors.fill: parent
            visible: root.kind === "music" && Media.artUrl(root.player) !== ""
            source: root.kind === "music" ? Media.artUrl(root.player) : ""
            retainWhileLoading: true
            fillMode: Image.PreserveAspectCrop
            sourceSize: Qt.size(96, 96)
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !(root.kind === "music" && Media.artUrl(root.player) !== "")
            name: root.activity.icon
            fill: true
            size: 15
            color: Theme.material ? (root.kind === "music" ? IslandInk.ink() : IslandInk.onTint(root.activity.color)) : root.activity.color
        }
    }

    Column {
        id: info
        anchors { left: lead.right; leftMargin: 12; right: controls.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        spacing: 1
        Text {
            width: parent.width
            text: root.kind === "captions" ? "Live captions" : LiveInfo.title(root.kind)
            elide: Text.ElideRight
            color: IslandInk.ink()
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Theme.material ? Font.Medium : Font.DemiBold }
        }
        // Captions: the last words, a few lines of them
        CaptionLines {
            visible: root.kind === "captions"
            width: parent.width
            height: visible ? implicitHeight : 0
            ink: IslandInk.ink()
            size: Theme.font.small
            lines: 3
        }
        Text {
            visible: root.kind !== "captions"
            width: parent.width
            text: LiveInfo.detail(root.kind)
            elide: Text.ElideRight
            color: IslandInk.dim(0.6)
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
        }
        // Where the song is (click to seek)
        Item { visible: root.hasTimeline; width: 1; height: 5 }
        // (Material: wavy, a click seeks)
        Loader {
            active: Theme.material && root.hasTimeline
            visible: active
            width: parent.width
            sourceComponent: WavyProgress {
                value: Math.max(0, Math.min(1, (root.player?.position ?? 0) / Math.max(1, root.player?.length ?? 1)))
                paused: !(root.player?.isPlaying ?? false)
                MouseArea {
                    anchors { fill: parent; topMargin: -4; bottomMargin: -4 }
                    enabled: root.player?.canSeek ?? false
                    cursorShape: Qt.PointingHandCursor
                    onClicked: m => {
                        root.player.position = Math.max(0, Math.min(1, m.x / width)) * root.player.length;
                        root.player.positionChanged();
                    }
                }
            }
        }
        // Timer and transfer progress
        Loader {
            active: Theme.material && LiveInfo.progress(root.kind) >= 0 && root.kind !== "music"
            visible: active
            width: parent.width
            sourceComponent: WavyProgress {
                value: Math.max(0, LiveInfo.progress(root.kind))
                color: IslandInk.tint(root.activity.color)
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
                iconColor: modelData.danger ? (Theme.material ? Theme.m3.error : Theme.accents.red) : IslandInk.ink()
                enabled: modelData.on
                opacity: enabled ? 1 : 0.4
                onClicked: modelData.act()
            }
        }
    }
}
