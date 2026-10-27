import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        footer: "While on, banners are hidden (critical alerts still show). Everything still arrives in Notification Center."
        Row {
            label: "Do Not Disturb"
            Switch { checked: Focus.on; onToggled: checked ? Focus.deactivate() : Focus.activate("dnd", 0, false) }
        }
    }
    Group {
        width: parent.width
        footer: "Without it, notifications and the calendar, to-do and focus widgets are each their own piece of glass, like Control Center's controls."
        Row {
            label: "Notification Center panel"
            Switch { checked: Config.options.notificationCenter.background; onToggled: Config.options.notificationCenter.background = !checked }
        }
    }
    // A sound event: switch, Play, and your own file instead of iRice's
    component SoundRow: Row {
        id: srow
        required property string key
        property string sound: key // what Play plays
        property bool custom: true // can use your own file
        readonly property string file: custom ? Config.options.sounds[key + "File"] : ""
        visible: Config.options.sounds.enabled
        detail: file !== "" ? file.split("/").pop() : ""

        QQ.Row {
            spacing: 8
            SettingsButton { text: "Play"; icon: "play"; onClicked: Sounds.play(srow.sound, true) }
            SettingsButton {
                visible: srow.custom
                text: "Choose…"
                enabled: !FilePicker.busy
                onClicked: FilePicker.pick({ title: "Choose a sound", start: "~/Music", filter: "Sounds|*.ogg *.oga *.wav *.mp3 *.flac" }, path => Config.options.sounds[srow.key + "File"] = path)
            }
            SettingsButton {
                visible: srow.file !== ""
                text: "Reset"
                onClicked: Config.options.sounds[srow.key + "File"] = ""
            }
            Switch { checked: Config.options.sounds[srow.key]; onToggled: Config.options.sounds[srow.key] = !checked }
        }
    }

    Group {
        width: parent.width
        title: "Sound effects"
        footer: "Choose… plays your own file instead of the style's sound."
        Row {
            label: "Play sound effects"
            Switch { checked: Config.options.sounds.enabled; onToggled: Config.options.sounds.enabled = !checked }
        }
        Row {
            visible: Config.options.sounds.enabled
            label: "Style"
            detail: ({ soft: "Low, rounded tones", marimba: "Warm wooden mallets", keys: "A soft electric piano", taps: "Short, low taps", minimal: "Quiet ticks", glass: "Bright glass and pops" })[Config.options.sounds.pack] ?? ""
            Dropdown {
                width: 200
                options: [
                    { value: "marimba", label: "Marimba" },
                    { value: "taps", label: "Taps" },
                    { value: "soft", label: "Soft" },
                    { value: "keys", label: "Keys" },
                    { value: "minimal", label: "Minimal" },
                    { value: "glass", label: "Glass" }
                ]
                value: Config.options.sounds.pack
                onChosen: v => {
                    Config.options.sounds.pack = v;
                    Qt.callLater(() => Sounds.play("notifications", true)); // hear it
                }
            }
        }
        SoundRow { key: "startup"; label: "When you log in" }
        SoundRow { key: "charging"; label: "Plugging in the charger" }
        SoundRow { key: "displays"; label: "Connecting a display" }
        SoundRow { key: "lock"; label: "Locking and unlocking"; custom: false }
        SoundRow { key: "notifications"; label: "Notifications"; custom: false; detail: "Not in a Focus mode, and not for apps that ask for silence" }
        SoundRow { key: "battery"; label: "Low battery"; custom: false }
        SoundRow { key: "volumeKeys"; label: "Volume keys"; custom: false }
        SoundRow { key: "usb"; label: "USB devices"; custom: false }
        SoundRow { key: "received"; label: "Files received"; custom: false }
        SoundRow { key: "recording"; label: "Screen recording"; custom: false }
        SoundRow { key: "error"; label: "Wrong password"; custom: false }
        SoundRow { key: "screenshot"; label: "Screenshots"; custom: false }
        SoundRow { key: "trash"; label: "Emptying the Trash"; custom: false }
        Row {
            visible: Config.options.sounds.enabled
            label: "Volume"
            detail: Math.round(Config.options.sounds.volume * 100) + "%"
            Slider {
                width: 220
                from: 0
                to: 1
                value: Config.options.sounds.volume
                onMoved: v => Config.options.sounds.volume = v
                onPressedChanged: if (!pressed) Sounds.play("charging", true) // hear the level
            }
        }
    }
    Group {
        width: parent.width
        Row {
            label: "Notification history"
            detail: Notifications.list.length + " kept"
            Rectangle {
                width: clearText.implicitWidth + 24
                height: 26
                radius: 13
                color: Theme.color.fill
                opacity: Notifications.list.length > 0 ? 1 : 0.4
                Text { id: clearText; anchors.centerIn: parent; text: "Clear all"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { anchors.fill: parent; enabled: Notifications.list.length > 0; onClicked: Notifications.clearAll() }
            }
        }
    }
}
