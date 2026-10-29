import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        title: "Nearby sharing"
        footer: !LocalSend.available ? "Sharing needs Python 3 and openssl (for encrypted transfers)." : "Works with LocalSend on phones and computers on the same network. Files you accept go to Downloads. If devices don't find each other, allow port 53317 in your firewall (e.g. sudo ufw allow 53317)." + (LocalSend.error ? " " + LocalSend.error : "")
        Row {
            label: "Send files"
            detail: "Choose files, then a device running LocalSend. Receiving can stay off."
            SettingsButton {
                text: "Send files…"
                icon: "paper-plane-tilt"
                enabled: LocalSend.available && !FilePicker.busy
                onClicked: Panels.pickShare()
            }
        }
        Row {
            label: "Receive files from"
            detail: Config.options.sharing.receiving === "off" ? "You can still send; nobody can see you" : "You're asked before anything is saved"
            Segmented {
                width: 320
                readonly property var values: ["off", "10min", "everyone"]
                options: ["No one", "Everyone for 10 min", "Everyone"]
                current: Math.max(0, values.indexOf(Config.options.sharing.receiving))
                onSelected: i => Config.options.sharing.receiving = values[i]
            }
        }
        Row {
            label: "Your name"
            detail: "How you appear on other devices"
            TextField {
                width: 220
                text: Config.options.sharing.name
                placeholder: (Quickshell.env("USER") ?? "") + "'s computer"
                onCommitted: t => Config.options.sharing.name = t.trim()
            }
        }
    }

    Group {
        width: parent.width
        title: "Quick Share / Nearby Share"
        footer: "Uses Packet to send to and receive from Android's built-in Quick Share. Enable Bluetooth and connect both devices to the same Wi-Fi. Device selection, confirmation and progress appear in Packet." + (QuickShare.error ? " " + QuickShare.error : "")
        // One row: what's there, and what to do about it
        Row {
            label: "Quick Share"
            detail: (QuickShare.available ? "Ready, through Packet" : "Needs the Packet app. Install it, then check again.") + " Separate from LocalSend and its receiving setting."
            QQ.Row {
                spacing: 8
                SettingsButton {
                    text: QuickShare.available ? "Open Quick Share" : "Get Packet…"
                    busy: QuickShare.busy
                    onClicked: QuickShare.available ? QuickShare.open() : QuickShare.setup()
                }
                SettingsButton {
                    visible: !QuickShare.available
                    text: "Check again"
                    busy: QuickShare.checking
                    onClicked: QuickShare.refresh()
                }
            }
        }
    }
}
