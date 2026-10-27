import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Settings → Desktop & Widgets → Screen recording (services/Recorder.qml).
// The toolbar (Super+Shift+5, Control Center → Record) changes the sound,
// pointer and countdown too; they're kept here.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    readonly property string pageTitle: "Screen recording"
    readonly property var rec: Config.options.recorder
    Component.onCompleted: Recorder.refreshMics()

    Group {
        width: parent.width
        visible: !Recorder.gsr
        Row {
            label: Recorder.available ? "Basic recording" : "Can't record yet"
            detail: Recorder.available ? "Install gpu-screen-recorder for hardware encoding, pausing, and the microphone together with system audio: sudo pacman -S gpu-screen-recorder" : "Install gpu-screen-recorder: sudo pacman -S gpu-screen-recorder"
            Icon { name: "info"; size: 20; color: Theme.color.secondary }
        }
    }

    Group {
        width: parent.width
        title: "Video"
        footer: "Recordings are MP4 files named by date, like Screen Recording 2026-10-28 at 09.41.00.mp4."
        Row {
            label: "Frame rate"
            Segmented {
                width: 200
                options: ["30 fps", "60 fps"]
                current: page.rec.fps === 30 ? 0 : 1
                onSelected: i => page.rec.fps = [30, 60][i]
            }
        }
        Row {
            visible: Recorder.gsr
            label: "Format"
            detail: page.rec.codec === "h264" ? "Plays everywhere" : page.rec.codec === "hevc" ? "Smaller files; some players and sites can't play it" : "Smallest files; needs a recent graphics card to record"
            Segmented {
                width: 240
                readonly property var values: ["h264", "hevc", "av1"]
                options: ["H.264", "HEVC", "AV1"]
                current: Math.max(0, values.indexOf(page.rec.codec))
                onSelected: i => page.rec.codec = values[i]
            }
        }
        Row {
            visible: Recorder.gsr
            label: "Quality"
            detail: "Higher quality makes larger files"
            Segmented {
                width: 300
                readonly property var values: ["medium", "high", "very_high", "ultra"]
                options: ["Medium", "High", "Very high", "Ultra"]
                current: Math.max(0, values.indexOf(page.rec.quality))
                onSelected: i => page.rec.quality = values[i]
            }
        }
        Row {
            visible: Recorder.gsr
            label: "Show mouse pointer"
            Switch { checked: page.rec.cursor; onToggled: page.rec.cursor = !checked }
        }
    }

    Group {
        width: parent.width
        title: "Sound"
        footer: Recorder.gsr ? "System audio and the microphone are mixed into one track. You can mute the microphone from the island while recording." : "Without gpu-screen-recorder only one is recorded: the microphone if it's on, else system audio."
        Row {
            label: "System audio"
            detail: "What your apps play"
            Switch { checked: page.rec.systemAudio; onToggled: page.rec.systemAudio = !checked }
        }
        Row {
            label: "Microphone"
            Dropdown {
                width: 240
                options: [{ value: "", label: "Off" }, { value: "default", label: "Default microphone" }].concat(Recorder.mics.map(m => ({ value: m.name, label: m.description })))
                value: page.rec.mic
                onChosen: v => page.rec.mic = v
            }
        }
        Row {
            label: "Countdown"
            detail: "Seconds before recording starts"
            Segmented {
                width: 200
                options: ["None", "3 s", "5 s"]
                current: Math.max(0, [0, 3, 5].indexOf(page.rec.countdown))
                onSelected: i => page.rec.countdown = [0, 3, 5][i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Saving"
        Row {
            label: "Folder"
            detail: "Leave empty for ~/Videos/Screen Recordings"
            TextField {
                width: 260
                text: page.rec.folder
                placeholder: "~/Videos/Screen Recordings"
                onCommitted: t => page.rec.folder = t.trim()
            }
        }
    }
}
