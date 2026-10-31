import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Services.Pipewire
import qs.common
import qs.modules.settings
import qs.services

// Sound: where sound plays and comes from, their volumes, AirPlay speakers
// (services/AirPlay), and a way to the sound effects (Notifications)
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property var outputs: AudioStreams.outputs
    readonly property var inputs: Pipewire.nodes.values.filter(n => !n.isSink && !n.isStream && n.audio && String(n.properties?.["media.class"] ?? "").startsWith("Audio/Source"))

    PwObjectTracker {
        objects: page.outputs.concat(page.inputs)
    }
    Component.onCompleted: AirPlay.check()

    component Level: QQ.Row {
        id: level
        property var node: null
        property bool mic: false
        spacing: 10
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: level.mic ? (level.node?.audio?.muted ? "microphone-slash" : "microphone") : (level.node?.audio?.muted ? "speaker-x" : "speaker-high")
            tooltip: level.node?.audio?.muted ? "Unmute" : "Mute"
            onClicked: if (level.node?.audio) level.node.audio.muted = !level.node.audio.muted
        }
        Slider {
            anchors.verticalCenter: parent.verticalCenter
            width: 200
            value: level.node?.audio?.volume ?? 0
            accessibleName: level.mic ? "Input volume" : "Output volume"
            onMoved: v => {
                if (level.node?.audio) {
                    level.node.audio.muted = false;
                    level.node.audio.volume = v;
                }
            }
        }
    }

    // ── Output ──────────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Output"
        footer: AirPlay.speakers.length > 0 ? "AirPlay speakers play about 2 seconds behind: fine for music, not for videos or calls." : ""
        Row {
            label: "Volume"
            detail: AudioStreams.outputName(Pipewire.defaultAudioSink)
            Level { node: Pipewire.defaultAudioSink }
        }
        Repeater {
            model: ScriptModel { values: page.outputs }
            Row {
                required property var modelData
                label: AudioStreams.outputName(modelData)
                detail: AirPlay.isAirPlay(modelData) ? "AirPlay" : ""
                labelColor: Theme.color.label
                SettingsButton {
                    visible: Pipewire.defaultAudioSink !== parent.parent.modelData
                    text: "Use"
                    onClicked: Pipewire.preferredDefaultAudioSink = parent.parent.modelData
                }
                Icon {
                    visible: Pipewire.defaultAudioSink === parent.parent.modelData
                    name: "check"
                    size: 16
                    color: Theme.accent
                }
            }
        }
    }

    // ── Input ───────────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Input"
        Row {
            visible: page.inputs.length === 0
            label: "No microphone found"
            detail: "Connect one, or check that it's turned on"
            labelColor: Theme.color.secondary
        }
        Row {
            visible: page.inputs.length > 0
            label: "Input volume"
            detail: Pipewire.defaultAudioSource?.description ?? ""
            Level { node: Pipewire.defaultAudioSource; mic: true }
        }
        Repeater {
            model: ScriptModel { values: page.inputs.length > 1 ? page.inputs : [] }
            Row {
                required property var modelData
                label: modelData.description || modelData.name
                SettingsButton {
                    visible: Pipewire.defaultAudioSource !== parent.parent.modelData
                    text: "Use"
                    onClicked: Pipewire.preferredDefaultAudioSource = parent.parent.modelData
                }
                Icon {
                    visible: Pipewire.defaultAudioSource === parent.parent.modelData
                    name: "check"
                    size: 16
                    color: Theme.accent
                }
            }
        }
    }

    // ── AirPlay ─────────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "AirPlay"
        footer: "HomePods, Apple TVs and AirPlay speakers on your network appear as outputs above. Some AirPlay 2-only speakers and ones with a password may not accept the sound."
        Row {
            visible: Needs.checked && !Needs.pipewire
            label: "AirPlay needs PipeWire"
            detail: "Your sound isn't going through PipeWire (Arch's default sound server), which AirPlay speakers come in through."
        }
        Row {
            visible: Needs.checked && Needs.pipewire && !Needs.avahi
            label: "Network discovery is off"
            detail: "AirPlay speakers are found with avahi, which isn't running. It asks for your password once."
            SettingsButton {
                text: Needs.installing || Health.fixing === "avahi" ? "Turning on…" : "Turn On"
                busy: Needs.installing || Health.fixing === "avahi"
                onClicked: Needs.turnOnAvahi()
            }
        }
        Row {
            visible: !Needs.checked || Needs.pipewire
            label: "AirPlay speakers"
            detail: !AirPlay.checked ? ""
                : AirPlay.installError !== "" ? AirPlay.installError
                : !AirPlay.installed ? "Needs pipewire-zeroconf. Install it, then turn this on. It asks for your password once."
                : AirPlay.error !== "" ? AirPlay.error
                : !AirPlay.wanted ? "Off"
                : AirPlay.speakers.length === 0 ? "Looking for speakers on your network…"
                : AirPlay.speakers.length === 1 ? "1 speaker found" : AirPlay.speakers.length + " speakers found"
            labelColor: AirPlay.installError !== "" || AirPlay.error !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                visible: AirPlay.checked && !AirPlay.installed
                text: AirPlay.installing ? "Installing…" : "Install"
                icon: "download-simple"
                busy: AirPlay.installing
                enabled: !Updates.running
                onClicked: AirPlay.install()
            }
            Switch {
                visible: AirPlay.installed
                checked: Config.options.audio.airplay
                onToggled: Config.options.audio.airplay = !checked
            }
        }
    }

    Group {
        width: parent.width
        SubpageLink {
            icon: "bell-simple-ringing"
            label: "Sound effects"
            detail: "Alerts, the startup sound, charging and more"
            value: Config.options.sounds.enabled ? "On" : "Off"
            page: "notifications"
        }
    }
}
