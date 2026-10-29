import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    // File managers' right-click "Send to a nearby device…" (file-manager-share.py)
    property var managers: [] // [{ id, name, found, installed, note }]
    property string fmError: ""
    readonly property string fmScript: decodeURIComponent(Qt.resolvedUrl("../../../assets/scripts/file-manager-share.py").toString().replace("file://", ""))
    function fm(args) {
        fmProc.command = ["python3", "-I", fmScript].concat(args);
        fmProc.running = true;
    }
    Component.onCompleted: fm(["status"])
    Process {
        id: fmProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    page.managers = JSON.parse(text);
                    page.fmError = "";
                } catch (e) {}
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim() !== "") page.fmError = text.trim()
        }
    }

    Group {
        width: parent.width
        title: "Nearby sharing"
        footer: !LocalSend.available ? "Sharing needs Python 3 and openssl (for encrypted transfers)." : "Works with LocalSend on phones and computers on the same network. Files you accept go to Downloads. If devices don't find each other, allow port 53317 in your firewall (e.g. sudo ufw allow 53317)." + (LocalSend.error ? " " + LocalSend.error : "")
        Row {
            label: "Send files"
            detail: "Choose files, then a nearby device: one running LocalSend, or an Android phone with Quick Share. Receiving can stay off."
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

    // Android's Quick Share, built in (services/QuickShare.qml)
    Group {
        width: parent.width
        title: "Android Quick Share"
        footer: (!QuickShare.available ? "Quick Share needs Python 3 with python-cryptography and python-protobuf."
            : "Phones on the same Wi-Fi find this computer in their Quick Share sheet as “" + QuickShare.name + "”, and you're asked before anything is saved, with the PIN the phone shows. To send, use Share on a file and pick the phone (its Quick Share open, visible to Everyone). If they don't find each other, allow port 53318 and mDNS (port 5353) in your firewall.")
            + (QuickShare.error ? " " + QuickShare.error : "")
        Row {
            label: "Visible to phones"
            detail: Config.options.sharing.quickShare === "off" ? "You can still send; phones can't see this computer" : "Phones nearby can offer you files, links and Wi-Fi networks"
            Segmented {
                width: 320
                readonly property var values: ["off", "10min", "everyone"]
                options: ["No one", "Everyone for 10 min", "Everyone"]
                current: Math.max(0, values.indexOf(Config.options.sharing.quickShare))
                onSelected: i => Config.options.sharing.quickShare = values[i]
            }
        }
        Row {
            label: "Bluetooth"
            detail: !QuickShare.bluetoothAvailable ? "No Bluetooth adapter: phones find this computer only on the same network"
                : QuickShare.bluetoothOn ? "On: phones without Wi-Fi find this computer too, then transfers move to Wi-Fi Direct"
                : "Turn on Bluetooth so phones can find this computer without Wi-Fi"
            SettingsButton {
                visible: QuickShare.bluetoothAvailable && !QuickShare.bluetoothOn
                text: "Turn on Bluetooth"
                icon: "bluetooth"
                onClicked: QuickShare.turnOnBluetooth()
            }
        }
        Row {
            label: "Save files to"
            PathField {
                folder: true
                value: Config.options.sharing.quickShareFolder
                placeholder: "~/Downloads"
                start: "~"
                onPicked: p => Config.options.sharing.quickShareFolder = p
            }
        }
    }

    // Sending from anywhere: file managers' menus, the selection
    Group {
        width: parent.width
        title: "Share from anywhere"
        footer: page.fmError !== "" ? page.fmError
            : page.managers.some(m => m.found) ? "Right-click files in these file managers to send them to a nearby device. The menus can be removed any time; nothing else in them changes."
            : "No supported file manager found (Files, Dolphin, Thunar or Nemo). iRice's own desktop and Downloads stack have Share… either way."
        Repeater {
            model: page.managers.filter(m => m.found || m.installed)
            Row {
                required property var modelData
                label: modelData.name
                detail: modelData.installed ? "“Send to a nearby device…” is in its right-click menu" + (modelData.note ? ". " + modelData.note : "") : "Not added"
                SettingsButton {
                    text: modelData.installed ? "Remove menu" : "Add menu"
                    icon: modelData.installed ? "minus" : "plus"
                    busy: fmProc.running
                    onClicked: page.fm([modelData.installed ? "remove" : "install", modelData.id])
                }
            }
        }
        Row {
            label: "Share the selection"
            detail: "Sends the selected text (or what you copied) to a nearby device. The shortcut is Super+Ctrl+S; change it in Keyboard Shortcuts."
            SettingsButton {
                text: "Share selection"
                icon: "share-network"
                onClicked: Sharing.shareSelection()
            }
        }
    }
}
