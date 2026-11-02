import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.common
import qs.services

// Under the island's volume, microphone or brightness bar, opened with the
// button at its end:
//   volume      the outputs (ticked: the one playing), App volumes › (each
//               app playing, with its own level), Sound settings
//   mic         the inputs, Sound settings
//   brightness  each display with its own level, Display settings
// Outputs and inputs show their device's picture when there is one.
// Material: the current row on tertiaryContainer with round ends, levels on
// Material's slider.
Column {
    id: root

    property string kind: "volume" // volume | mic | brightness
    property string page: "list" // list | apps
    // (white on the island; the capsule HUD passes the theme's)
    property color ink: "#F5F5F7"
    property color ink2: Qt.alpha(ink, 0.6)
    spacing: 2

    PwObjectTracker {
        objects: root.kind === "volume" ? AudioStreams.outputs.concat(AudioStreams.allStreams) : root.kind === "mic" ? AudioStreams.inputs : []
    }

    // A row: a picture or icon, a name, a tick or an arrow
    component Choice: Item {
        id: row
        property string picture: ""
        property string icon: ""
        property string text: ""
        property bool current: false
        property bool chevron: false
        signal picked
        width: root.width
        height: 32
        Rectangle {
            anchors.fill: parent
            radius: Theme.material && row.current ? height / 2 : 10
            color: Theme.material && row.current ? Qt.tint(Theme.m3.tertiaryContainer, Qt.alpha(Theme.m3.onTertiaryContainer, rowMouse.pressed ? 0.1 : rowMouse.containsMouse ? 0.08 : 0))
                : rowMouse.pressed ? Qt.alpha(root.ink, Theme.material ? 0.1 : 0.14) : rowMouse.containsMouse ? Qt.alpha(root.ink, 0.08) : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }
        Image {
            x: 6
            anchors.verticalCenter: parent.verticalCenter
            width: 22
            height: 22
            visible: row.picture !== ""
            source: row.picture
            sourceSize: Qt.size(44, 44)
            fillMode: Image.PreserveAspectFit
            smooth: true
        }
        Icon {
            x: 9
            anchors.verticalCenter: parent.verticalCenter
            visible: row.picture === "" && row.icon !== ""
            name: row.icon
            fill: true
            size: 16
            color: row.content
        }
        readonly property color content: Theme.material && row.current ? Theme.m3.onTertiaryContainer : root.ink
        Text {
            anchors { left: parent.left; leftMargin: 36; right: mark.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            text: row.text
            elide: Text.ElideRight
            color: row.content
            font { family: Theme.font.family; pixelSize: 13; weight: row.current ? Font.DemiBold : Font.Normal }
        }
        Icon {
            id: mark
            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            visible: row.current || row.chevron
            name: row.chevron ? "caret-right" : "check"
            bold: true
            size: 13
            color: Theme.material && row.current ? row.content : root.ink2
        }
        MouseArea {
            id: rowMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: row.picked()
        }
    }

    // A level in the island's own style: click or drag along it, or scroll
    component Level: Item {
        id: lv
        property real value: 0
        signal moved(real v)
        height: Theme.material ? 32 : 20
        // Material: its slider
        Loader {
            active: Theme.material
            anchors.fill: parent
            sourceComponent: Slider {
                value: lv.value
                onMoved: v => lv.moved(v)
            }
        }
        Rectangle {
            visible: !Theme.material
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width
            height: 6
            radius: 3
            color: Qt.alpha(root.ink, 0.16)
            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, lv.value))
                height: parent.height
                radius: 3
                color: root.ink
            }
        }
        MouseArea {
            enabled: !Theme.material
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            preventStealing: true
            onPressed: e => lv.moved(Math.max(0, Math.min(1, e.x / width)))
            onPositionChanged: e => {
                if (pressed)
                    lv.moved(Math.max(0, Math.min(1, e.x / width)));
            }
        }
        WheelHandler {
            onWheel: e => lv.moved(Math.max(0, Math.min(1, lv.value + (e.angleDelta.y > 0 ? 0.05 : -0.05))))
        }
    }

    // ── Outputs or inputs ───────────────────────────────────────────────
    Repeater {
        model: ScriptModel { values: root.page !== "list" ? [] : root.kind === "volume" ? AudioStreams.outputs : root.kind === "mic" ? AudioStreams.inputs : [] }
        Choice {
            required property var modelData
            picture: AudioStreams.picture(modelData)
            icon: root.kind === "mic" ? AudioStreams.inputIcon(modelData) : AudioStreams.outputIcon(modelData)
            text: AudioStreams.outputName(modelData)
            current: root.kind === "mic" ? Pipewire.defaultAudioSource === modelData : Pipewire.defaultAudioSink === modelData
            onPicked: {
                if (root.kind === "mic")
                    Pipewire.preferredDefaultAudioSource = modelData;
                else
                    Pipewire.preferredDefaultAudioSink = modelData;
            }
        }
    }

    // ── Displays, each with its level ───────────────────────────────────
    Repeater {
        model: ScriptModel { values: root.kind === "brightness" ? Brightness.displays : [] }
        Item {
            id: disp
            required property var modelData
            width: root.width
            height: 34
            Image {
                x: 6
                anchors.verticalCenter: parent.verticalCenter
                width: 22
                height: 22
                visible: disp.modelData.builtIn
                source: disp.modelData.builtIn ? Devices.pictureFor("computer", "") : ""
                sourceSize: Qt.size(44, 44)
                fillMode: Image.PreserveAspectFit
            }
            Icon {
                x: 9
                anchors.verticalCenter: parent.verticalCenter
                visible: !disp.modelData.builtIn
                name: "monitor"
                fill: true
                size: 16
                color: root.ink
            }
            Text {
                id: dispName
                anchors { left: parent.left; leftMargin: 36; verticalCenter: parent.verticalCenter }
                width: Math.min(implicitWidth, root.width * 0.4)
                text: disp.modelData.name
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 13 }
            }
            Level {
                anchors { left: dispName.right; leftMargin: 12; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                value: disp.modelData.value
                onMoved: v => Brightness.setFor(disp.modelData.id, v, true)
            }
        }
    }

    // ── App volumes ─────────────────────────────────────────────────────
    Choice {
        visible: root.kind === "volume" && root.page === "list" && AudioStreams.streams.length > 0
        icon: "sliders-horizontal"
        text: "App volumes"
        chevron: true
        onPicked: root.page = "apps"
    }
    Choice {
        visible: root.page === "apps"
        icon: "caret-left"
        text: "App volumes"
        onPicked: root.page = "list"
    }
    Repeater {
        model: ScriptModel { values: root.page === "apps" ? AudioStreams.streams : [] }
        Item {
            id: app
            required property var modelData
            readonly property bool muted: modelData.audio?.muted ?? false
            width: root.width
            height: 34
            AppIcon {
                id: appIcon
                x: 7
                anchors.verticalCenter: parent.verticalCenter
                size: 20
                source: AudioStreams.icon(app.modelData)
                opacity: app.muted ? 0.35 : 1
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: if (app.modelData.audio) app.modelData.audio.muted = !app.muted
                }
            }
            Text {
                id: appName
                anchors { left: parent.left; leftMargin: 36; verticalCenter: parent.verticalCenter }
                width: Math.min(implicitWidth, root.width * 0.35)
                text: AudioStreams.name(app.modelData)
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 13 }
            }
            Level {
                anchors { left: appName.right; leftMargin: 12; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                value: app.muted ? 0 : Math.min(1, app.modelData.audio?.volume ?? 0)
                onMoved: v => {
                    if (!app.modelData.audio)
                        return;
                    app.modelData.audio.muted = false;
                    app.modelData.audio.volume = v;
                }
            }
        }
    }

    // ── Settings ────────────────────────────────────────────────────────
    Choice {
        visible: root.page === "list"
        icon: "gear"
        text: root.kind === "brightness" ? "Display settings" : "Sound settings"
        onPicked: Panels.openSettings(root.kind === "brightness" ? "displays" : "sound")
    }
}
