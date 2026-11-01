import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.common
import qs.services

// The microphone: its level (the icon mutes) and the inputs to pick from,
// each with its device's picture. In Control Center's Microphone page and,
// with Settings → Sound → "Microphone in the Sound menu", under the Sound
// menu's outputs.
Column {
    id: root
    spacing: 2

    PwObjectTracker {
        objects: AudioStreams.inputs
    }

    property bool showTitle: false
    Text {
        visible: root.showTitle
        text: "Microphone"
        leftPadding: 6
        topPadding: 8
        bottomPadding: 4
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }
    CCSlider {
        width: parent.width
        visible: AudioStreams.inputs.length > 0
        label: ""
        icon: Audio.micMuted ? "microphone-slash" : "microphone"
        value: Audio.micMuted ? 0 : Audio.micVolume
        onMoved: v => Audio.setMicVolume(v, true)
    }
    Text {
        visible: AudioStreams.inputs.length === 0
        width: parent.width
        topPadding: 12
        bottomPadding: 12
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: "No microphone found. Connect one, or check that it's turned on."
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
    Item {
        width: 1
        height: 4
        visible: AudioStreams.inputs.length > 0
    }
    Repeater {
        model: ScriptModel { values: AudioStreams.inputs }
        ListRow {
            required property var modelData
            icon: AudioStreams.inputIcon(modelData)
            picture: AudioStreams.picture(modelData)
            highlighted: Pipewire.defaultAudioSource === modelData
            title: AudioStreams.outputName(modelData)
            trailingIcon: Pipewire.defaultAudioSource === modelData ? "check" : ""
            onClicked: Pipewire.preferredDefaultAudioSource = modelData
        }
    }
}
