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
    // What on this computer stands in sharing's way (sharing-check.py):
    // a firewall, Bluetooth asking phones to pair
    property var system: ({}) // { firewall: { kind, allowed, blocks }, pairing: { risk } }
    property string fixing: "" // "firewall" | "pairing" while its password prompt and fix run
    property string fixError: ""
    readonly property string checkScript: decodeURIComponent(Qt.resolvedUrl("../../../assets/scripts/sharing-check.py").toString().replace("file://", ""))
    function checkSystem() {
        checkProc.command = ["python3", "-I", checkScript, "status"];
        checkProc.running = true;
    }
    function fixSystem(what) {
        fixing = what;
        fixError = "";
        fixProc.command = ["python3", "-I", checkScript, "fix", what];
        fixProc.running = true;
    }
    Process {
        id: checkProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    page.system = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Process {
        id: fixProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const r = JSON.parse(text);
                    page.fixError = r.ok ? "" : r.error;
                } catch (e) {
                    page.fixError = "It didn't work; try again.";
                }
                page.fixing = "";
                page.checkSystem();
            }
        }
    }
    Component.onCompleted: {
        fm(["status"]);
        checkSystem();
    }
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
            visible: page.system.firewall?.blocks ?? false
            label: "Firewall"
            labelColor: Theme.color.danger
            detail: (page.system.firewall?.allowed === false ? "Your firewall (" + page.system.firewall.kind + ") blocks Quick Share and LocalSend"
                : "Your firewall (" + (page.system.firewall?.kind ?? "") + ") may block Quick Share and LocalSend")
                + ": phones can't connect to this computer. Allowing ports 53317–53319 and mDNS asks for your password."
                + (page.fixError !== "" && page.fixing === "" ? " " + page.fixError : "")
            SettingsButton {
                text: "Allow through firewall"
                icon: "shield-check"
                busy: page.fixing === "firewall"
                enabled: page.fixing === ""
                onClicked: page.fixSystem("firewall")
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
                : QuickShare.bluetoothOn ? "On: phones without Wi-Fi find this computer too, then transfers move to Wi-Fi when both are on the same network"
                : "Turn on Bluetooth so phones can find this computer without Wi-Fi"
            SettingsButton {
                visible: QuickShare.bluetoothAvailable && !QuickShare.bluetoothOn
                text: "Turn on Bluetooth"
                icon: "bluetooth"
                onClicked: QuickShare.turnOnBluetooth()
            }
        }
        Row {
            visible: page.system.pairing?.risk ?? false
            label: "Phones ask to pair"
            detail: "Some of Bluetooth's experimental features (Ranging and LE Audio controls) make phones ask to pair when they send with Quick Share; phones never need to. Turning them off asks for your password and restarts Bluetooth (headphones reconnect, and still show their battery)."
            SettingsButton {
                text: "Stop pairing prompts"
                icon: "bluetooth"
                busy: page.fixing === "pairing"
                enabled: page.fixing === ""
                onClicked: page.fixSystem("pairing")
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
