import QtQuick
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
        footer: !LocalSend.available ? "Sharing needs Python 3." : "Works with LocalSend on phones and computers on the same network. Files you accept go to Downloads. If devices don't find each other, allow port 53317 in your firewall (e.g. sudo ufw allow 53317)." + (LocalSend.error ? " " + LocalSend.error : "")
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
}
