import QtQuick
import Quickshell.Services.Pipewire
import qs.common
import qs.services

// Output device picker and a per-app volume mixer.
DetailPage {
    id: root

    readonly property var outputs: Pipewire.nodes.values.filter(n => n.isSink && !n.isStream && n.audio)
    readonly property var streams: Pipewire.nodes.values.filter(n => n.isSink && n.isStream && n.audio)

    title: "Sound"

    PwObjectTracker {
        objects: root.streams
    }

    Text {
        text: "Output"
        leftPadding: 6
        bottomPadding: 2
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }

    Repeater {
        model: root.outputs
        ListRow {
            required property var modelData
            icon: modelData.description.toLowerCase().includes("headphone") ? "headphones" : "speaker-hifi"
            highlighted: Pipewire.defaultAudioSink === modelData
            title: modelData.description || modelData.name
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
        model: root.streams
        CCSlider {
            required property var modelData
            width: parent.width
            label: modelData.properties["application.name"] ?? modelData.description ?? modelData.name
            icon: modelData.audio.muted ? "speaker-x" : "speaker-high"
            value: modelData.audio.muted ? 0 : modelData.audio.volume
            onMoved: v => {
                modelData.audio.muted = false;
                modelData.audio.volume = v;
            }
        }
    }
}
