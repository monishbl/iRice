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

    implicitHeight: kind === "music" ? 52 : 36

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
            text: root.kind === "music" ? (root.player?.trackTitle || "Playing") : root.kind === "timer" ? (Timers.phase === "focus" ? "Focus" : "Break") : root.kind === "stopwatch" ? "Stopwatch" : "Recording"
            elide: Text.ElideRight
            color: "#F5F5F7"
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            text: root.kind === "music" ? (root.player?.trackArtist ?? "") : root.kind === "timer" ? Timers.format(Timers.remaining) + " left" : root.kind === "stopwatch" ? Timers.format(Math.floor(Timers.swElapsed / 1000)) : Timers.format(Math.floor((clock.date - ScreenTools.recordingStarted) / 1000))
            elide: Text.ElideRight
            color: Qt.rgba(1, 1, 1, 0.6)
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
            SystemClock { id: clock; precision: SystemClock.Seconds; enabled: root.kind === "recording" && root.shown }
        }
    }

    Row {
        id: controls
        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
        spacing: 2
        Repeater {
            model: root.kind === "music" ? [
                { icon: "skip-back", fill: true, act: () => root.player?.previous() },
                { icon: root.player?.isPlaying ? "pause" : "play", fill: true, act: () => root.player?.togglePlaying() },
                { icon: "skip-forward", fill: true, act: () => root.player?.next() }
            ] : root.kind === "timer" ? [
                { icon: Timers.running ? "pause" : "play", fill: true, act: () => Timers.toggle() },
                { icon: "skip-forward", fill: true, act: () => Timers.skip() }
            ] : root.kind === "stopwatch" ? [
                { icon: "flag", fill: false, act: () => Timers.swLap() },
                { icon: Timers.swRunning ? "pause" : "play", fill: true, act: () => Timers.swToggle() }
            ] : [
                { icon: "stop", fill: true, act: () => ScreenTools.stopRecording() }
            ]
            IconButton {
                required property var modelData
                width: 32
                height: 32
                icon: modelData.icon
                fill: modelData.fill
                iconSize: 16
                iconColor: "#F5F5F7"
                onClicked: modelData.act()
            }
        }
    }
}
