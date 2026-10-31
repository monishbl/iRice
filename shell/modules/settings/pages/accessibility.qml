import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Accessibility: live captions (services/Captions) and finding the pointer
// (services/FindPointer)
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    Component.onCompleted: Dictation.refresh()

    // ── Live captions ───────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Live captions"
        footer: "Subtitles for what this computer plays: videos, calls, podcasts. Made on this computer with the Tiny speech model, about 2 seconds behind; nothing is saved or sent anywhere. The microphone is never captioned. Super+Ctrl+L turns them on and off."

        // What it needs first
        Row {
            visible: !Captions.hasBinary
            label: "Speech recognition"
            detail: Dictation.installError !== "" ? Dictation.installError : "Install whisper.cpp to turn speech into text on this computer. It asks for your password once."
            labelColor: Dictation.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: Dictation.installing ? "Installing…" : "Install"
                icon: "download-simple"
                primary: true
                busy: Dictation.installing
                enabled: !Updates.running
                onClicked: Captions.install()
            }
        }
        Row {
            visible: Captions.hasBinary && !Captions.hasModel
            label: "Tiny speech model"
            detail: Captions.downloading ? "Downloading… " + Math.round(Dictation.downloadProgress * 100) + "%"
                : Dictation.downloadError !== "" && Dictation.lastDownload === "tiny" ? Dictation.downloadError
                : "75 MB, downloaded once. Fast enough to keep up with speech."
            SettingsButton {
                text: Captions.downloading ? "Cancel" : "Download"
                icon: Captions.downloading ? "" : "download-simple"
                primary: !Captions.downloading
                enabled: Captions.downloading || Dictation.downloading === ""
                onClicked: Captions.downloading ? Dictation.cancelDownload() : Captions.download()
            }
        }
        Row {
            label: "Live captions"
            detail: !Captions.hasBinary ? "Install speech recognition above first" : !Captions.hasModel ? "Download the Tiny model above first" : Captions.error !== "" ? Captions.error : Captions.enabled ? "On" : "Off"
            labelColor: Captions.error !== "" ? Theme.color.danger : Theme.color.label
            Switch {
                enabled: Captions.ready || Captions.enabled
                checked: Captions.enabled
                onToggled: Captions.cfg.enabled = !checked
            }
        }
        Row {
            label: "Show them"
            detail: Captions.placement === "island" ? "In the island at the top: the last words, one line"
                : Captions.placement === "strip" ? "Across the bottom of the screen; windows make room for them"
                : "In a bar above the Dock. Drag it anywhere."
            Segmented {
                width: 240
                options: ["Floating", "Island", "Bottom"]
                current: Captions.placement === "island" ? 1 : Captions.placement === "strip" ? 2 : 0
                onSelected: i => Captions.cfg.placement = ["bar", "island", "strip"][i]
            }
        }
        Row {
            label: "Language"
            detail: "Automatic suits videos in different languages; choosing one is more accurate"
            Dropdown {
                width: 220
                options: [{ value: "auto", label: "Automatic" }].concat(Dictation.languages.filter(l => l.value !== "" && l.value !== "auto"))
                value: Captions.cfg.language || "auto"
                onChosen: v => Captions.cfg.language = v
            }
        }
        Row {
            visible: Captions.placement === "bar" && (Captions.cfg.barX >= 0 || Captions.cfg.barY >= 0)
            label: "The bar's place"
            detail: "Moved by you"
            SettingsButton {
                text: "Put It Back"
                onClicked: {
                    Captions.cfg.barX = -1;
                    Captions.cfg.barY = -1;
                }
            }
        }
    }

    // ── The pointer ─────────────────────────────────────────────────────
    Group {
        width: parent.width
        title: "Pointer"
        Row {
            label: "Shake to find the pointer"
            detail: "Move the mouse quickly back and forth: the pointer grows for a moment, with a ring around it"
            Switch { checked: Config.options.findPointer.enabled; onToggled: Config.options.findPointer.enabled = !checked }
        }
    }
}
