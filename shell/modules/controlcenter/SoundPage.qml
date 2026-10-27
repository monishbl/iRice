import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.common
import qs.services

// Output device picker and a per-app volume mixer: each app playing sound
// gets its icon (click to mute), a volume slider and, with more than one
// output, a chooser for the device it plays to.
DetailPage {
    id: root

    readonly property var outputs: AudioStreams.outputs
    readonly property var streams: AudioStreams.streams

    title: "Sound"

    // Binds the streams so their volume, mute and app details stay live
    PwObjectTracker {
        objects: AudioStreams.allStreams
    }

    Text {
        text: "Output"
        leftPadding: 6
        bottomPadding: 2
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }

    Repeater {
        model: ScriptModel { values: root.outputs } // (keeps rows, and a slider being dragged, when another device comes or goes)
        ListRow {
            required property var modelData
            icon: AudioStreams.outputIcon(modelData)
            highlighted: Pipewire.defaultAudioSink === modelData
            title: AudioStreams.outputName(modelData)
            onClicked: Pipewire.preferredDefaultAudioSink = modelData
        }
    }

    Text {
        visible: root.streams.length > 0
        text: "Applications"
        leftPadding: 6
        topPadding: 8
        bottomPadding: 4
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }

    Repeater {
        model: ScriptModel { values: root.streams } // (keeps rows when another app starts playing)

        Column {
            id: app
            required property var modelData
            readonly property bool muted: modelData.audio?.muted ?? false
            readonly property var output: AudioStreams.outputOf(modelData)
            // Shown straight away on a pick, until the stream really moves
            property var pendingOutput: null
            readonly property var shownOutput: pendingOutput ?? output
            property bool choosing: false
            property bool failed: false
            readonly property bool canChoose: AudioStreams.canMove && root.outputs.length > 1

            width: parent.width
            spacing: 6
            bottomPadding: 6

            onOutputChanged: {
                if (pendingOutput && output === pendingOutput) {
                    pendingOutput = null;
                    moveTimeout.stop();
                }
            }
            Timer {
                id: moveTimeout
                interval: 3000
                onTriggered: {
                    app.pendingOutput = null;
                    app.failed = true;
                }
            }

            Item {
                width: parent.width
                height: 28

                // App icon: click to mute or unmute
                Rectangle {
                    id: iconButton
                    x: 2
                    width: 28
                    height: 28
                    radius: Theme.radius.item
                    color: iconMouse.pressed ? Theme.color.fillPressed : iconMouse.containsMouse ? Theme.color.fill : "transparent"
                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

                    AppIcon {
                        anchors.centerIn: parent
                        size: 22
                        source: AudioStreams.icon(app.modelData)
                        opacity: app.muted ? 0.35 : 1
                        Behavior on opacity { OpacityAnimator { duration: Theme.motion.hover } }
                    }
                    // Muted badge
                    Rectangle {
                        visible: app.muted
                        anchors { right: parent.right; bottom: parent.bottom; rightMargin: -1; bottomMargin: -1 }
                        width: 14
                        height: 14
                        radius: 7
                        color: Theme.glass.solid
                        border.width: 1
                        border.color: Theme.color.separator
                        Icon { anchors.centerIn: parent; name: "speaker-x"; fill: true; size: 9; color: Theme.color.label }
                    }
                    MouseArea {
                        id: iconMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (app.modelData.audio) app.modelData.audio.muted = !app.muted
                    }
                }

                Text {
                    anchors { left: iconButton.right; leftMargin: 8; right: outputButton.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    text: AudioStreams.name(app.modelData)
                    elide: Text.ElideRight
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                }

                // The device it plays to; a chooser with more than one
                Rectangle {
                    id: outputButton
                    visible: root.outputs.length > 1 && app.shownOutput !== null
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    width: visible ? Math.min(150, outputLabel.implicitWidth + (app.canChoose ? 30 : 16)) : 0
                    height: 24
                    radius: Theme.radius.control - 2
                    color: !app.canChoose ? "transparent" : outputMouse.pressed ? Theme.color.fillPressed : outputMouse.containsMouse || app.choosing ? Theme.color.fillHover : Theme.color.fill

                    Text {
                        id: outputLabel
                        anchors { left: parent.left; leftMargin: 8; right: caret.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
                        text: AudioStreams.outputName(app.shownOutput)
                        elide: Text.ElideRight
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                    Icon {
                        id: caret
                        visible: app.canChoose
                        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        width: visible ? implicitWidth : 0
                        name: app.choosing ? "caret-up" : "caret-down"
                        bold: true
                        size: 10
                        color: Theme.color.secondary
                    }
                    MouseArea {
                        id: outputMouse
                        anchors.fill: parent
                        enabled: app.canChoose
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            app.failed = false;
                            app.choosing = !app.choosing;
                        }
                    }
                }
            }

            CCSlider {
                width: parent.width
                icon: app.muted ? "speaker-x" : "speaker-high"
                value: app.muted ? 0 : Math.min(1, app.modelData.audio?.volume ?? 0)
                onMoved: v => {
                    if (!app.modelData.audio)
                        return;
                    app.modelData.audio.muted = false;
                    app.modelData.audio.volume = v;
                }
            }

            Text {
                visible: app.failed
                width: parent.width
                leftPadding: 6
                wrapMode: Text.Wrap
                text: "Couldn't switch the output. Try again, or check that pipewire-pulse is running."
                color: Theme.color.danger
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }

            // Output chooser, inline so the panel grows instead of clipping a menu
            Column {
                visible: app.choosing
                width: parent.width
                spacing: 2
                Repeater {
                    model: ScriptModel { values: app.choosing ? root.outputs : [] }
                    ListRow {
                        required property var modelData
                        icon: AudioStreams.outputIcon(modelData)
                        highlighted: app.shownOutput === modelData
                        title: AudioStreams.outputName(modelData)
                        trailingIcon: app.shownOutput === modelData ? "check" : ""
                        onClicked: {
                            app.choosing = false;
                            if (modelData === app.shownOutput)
                                return;
                            app.failed = false;
                            app.pendingOutput = modelData;
                            moveTimeout.restart();
                            AudioStreams.move(app.modelData, modelData);
                        }
                    }
                }
            }
        }
    }
}
