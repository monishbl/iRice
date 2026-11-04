import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Accessibility: live captions (services/Captions), and a link to Shake to
// find the pointer (its home is Mouse & Trackpad)
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
        // One choice: off, or where they show
        Row {
            label: "Live captions"
            detail: !Captions.hasBinary ? "Install speech recognition above first" : !Captions.hasModel ? "Download the Tiny model above first" : Captions.error !== "" ? Captions.error
                : !Captions.enabled ? "Off"
                : Captions.placement === "island" ? "In the island at the top: the last words, a few lines when it's open"
                : Captions.placement === "strip" ? "Across the bottom of the screen; windows make room for them"
                : Captions.placement === "video" ? "Over the bottom of the window that's playing, up to three lines, like a film's subtitles"
                : "In a bar above the Dock. Drag it anywhere."
            labelColor: Captions.error !== "" ? Theme.color.danger : Theme.color.label
            Segmented {
                width: 380
                enabled: Captions.ready || Captions.enabled // (until it's installed: the row says what's missing)
                readonly property var values: ["off", "bar", "video", "island", "strip"]
                options: ["Off", "Floating", "Over video", "Island", "Bottom"]
                current: !Captions.enabled ? 0 : Math.max(1, values.indexOf(Captions.placement))
                onSelected: i => {
                    if (i > 0)
                        Captions.cfg.placement = values[i];
                    Captions.cfg.enabled = i > 0;
                }
            }
        }
    }

    // ── The pointer: set in Mouse & Trackpad (one home per setting) ─────
    Group {
        width: parent.width
        title: "Pointer"
        SubpageLink {
            icon: "cursor"
            label: "Shake to find the pointer"
            detail: "In Mouse & Trackpad"
            value: !Config.options.findPointer.enabled || Config.options.findPointer.style === "off" ? "Off" : Config.options.findPointer.style === "grow" ? "Bigger" : "Ring"
            page: "mouse"
            target: "Shake to find the pointer"
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            title: "Live captions"
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
    }
}
