import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Dictation (services/Dictation): what it needs, the speech model, the
// language, and the sounds
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    Component.onCompleted: Dictation.refresh()

    Group {
        width: parent.width
        footer: "Press Super+H, speak, then press it again. Or hold Super+H while you speak and let go when you're done. The words are typed where your cursor is. Everything stays on this computer."

        // What it still needs
        Row {
            visible: !Dictation.hasBinary
            label: "Speech recognition"
            detail: Dictation.installError !== "" ? Dictation.installError : "Install whisper.cpp to turn speech into text on this computer. It asks for your password once."
            labelColor: Dictation.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: Dictation.installing ? "Installing…" : "Install"
                icon: "download-simple"
                primary: true
                busy: Dictation.installing
                enabled: !Updates.running
                onClicked: Dictation.install()
            }
        }
        Row {
            visible: Dictation.hasBinary && Tools.ready && !Tools.has("wtype")
            label: "Typing into apps needs wtype"
            detail: "Without it, what you say goes on the clipboard to paste. It asks for your password once."
            SettingsButton {
                text: Dictation.installing ? "Installing…" : "Install"
                busy: Dictation.installing
                enabled: !Updates.running
                onClicked: Dictation.install()
            }
        }
        Row {
            visible: Dictation.hasBinary
            label: Dictation.ready ? "Ready" : "Download a speech model to start"
            detail: Dictation.ready ? "Super+H to dictate" : "Speech recognition is installed"
            labelColor: Dictation.ready ? Theme.color.label : Theme.color.secondary
            Icon {
                name: Dictation.ready ? "check-circle" : "info"
                fill: Dictation.ready
                size: 18
                color: Dictation.ready ? Theme.accents.green : Theme.color.secondary
            }
        }
    }

    Group {
        width: parent.width
        title: "Speech model"
        footer: "Bigger models make fewer mistakes but take longer after you stop talking. Models are downloaded once and kept in ~/.local/share/irice/whisper."

        Repeater {
            model: Dictation.models
            Row {
                id: modelRow
                required property var modelData
                readonly property bool chosen: Dictation.model === modelData.value
                readonly property bool have: Dictation.info.models?.[modelData.value] ?? false
                readonly property bool loading: Dictation.downloading === modelData.value
                label: modelData.label + (chosen ? " · in use" : "")
                detail: loading ? "Downloading… " + Math.round(Dictation.downloadProgress * 100) + "%"
                    : Dictation.downloadError !== "" && Dictation.downloading === "" && Dictation.lastDownload === modelData.value && !have ? Dictation.downloadError
                    : modelData.detail + " · " + (modelData.mb >= 1000 ? (modelData.mb / 1000).toFixed(1) + " GB" : modelData.mb + " MB") + (have ? " · downloaded" : "")
                labelColor: Theme.color.label
                QQ.Row {
                    spacing: 8
                    SettingsButton {
                        visible: modelRow.have && !modelRow.chosen
                        text: "Use"
                        onClicked: Config.options.dictation.model = modelRow.modelData.value
                    }
                    SettingsButton {
                        visible: modelRow.have
                        text: "Remove"
                        danger: true
                        onClicked: Dictation.removeModel(modelRow.modelData.value)
                    }
                    SettingsButton {
                        visible: !modelRow.have
                        text: modelRow.loading ? "Cancel" : "Download"
                        icon: modelRow.loading ? "" : "download-simple"
                        primary: !modelRow.loading && modelRow.chosen && Dictation.hasBinary
                        enabled: modelRow.loading || Dictation.downloading === ""
                        onClicked: {
                            if (modelRow.loading) {
                                Dictation.cancelDownload();
                            } else {
                                Dictation.download(modelRow.modelData.value);
                            }
                        }
                    }
                }
            }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Language"
            detail: "The language you speak. Automatic works for most, but may mistake a short sentence for another language."
            Dropdown {
                width: 220
                options: [
                    { value: "", label: "Same as the system" },
                    { value: "auto", label: "Automatic" },
                    { value: "en", label: "English" },
                    { value: "hi", label: "Hindi" },
                    { value: "es", label: "Spanish" },
                    { value: "fr", label: "French" },
                    { value: "de", label: "German" },
                    { value: "it", label: "Italian" },
                    { value: "pt", label: "Portuguese" },
                    { value: "nl", label: "Dutch" },
                    { value: "ru", label: "Russian" },
                    { value: "ar", label: "Arabic" },
                    { value: "zh", label: "Chinese" },
                    { value: "ja", label: "Japanese" },
                    { value: "ko", label: "Korean" },
                    { value: "ta", label: "Tamil" },
                    { value: "te", label: "Telugu" },
                    { value: "bn", label: "Bengali" },
                    { value: "ur", label: "Urdu" },
                    { value: "tr", label: "Turkish" },
                    { value: "pl", label: "Polish" },
                    { value: "uk", label: "Ukrainian" }
                ]
                value: Config.options.dictation.language
                onChosen: v => Config.options.dictation.language = v
            }
        }
        Row {
            label: "Play sounds"
            detail: "A sound as it starts and stops listening"
            Switch { checked: Config.options.dictation.sounds; onToggled: Config.options.dictation.sounds = !checked }
        }
    }
}
