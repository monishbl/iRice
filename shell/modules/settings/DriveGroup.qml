import QtQuick as QQ
import qs.common
import qs.services

// A cloud drive as a folder (services/CloudDrives): Settings → Google and
// Settings → Microsoft 365 each have one
Group {
    id: root
    required property string drive // gdrive | onedrive
    readonly property string name: CloudDrives.names[drive]
    readonly property var d: CloudDrives.drive(drive)
    readonly property bool working: CloudDrives.busy.endsWith(" " + drive)
    readonly property string error: CloudDrives.errors[drive] ?? ""

    title: name + " folder"
    footer: error !== "" ? error
        : CloudDrives.busy === "connect " + drive ? "Sign in in your browser and allow access, then come back here…"
        : d.connected ? "Files open on demand from the cloud; ones you use are kept on this computer (up to 2 GB) and changes upload in the background. It comes back at every login."
        : name + " as a folder in Files, file pickers and the terminal. It signs in separately from the account above (through rclone)."

    Row {
        visible: !CloudDrives.hasRclone
        label: name + " as a folder"
        detail: CloudDrives.installError !== "" ? CloudDrives.installError : "Needs rclone. It asks for your password once."
        labelColor: CloudDrives.installError !== "" ? Theme.color.danger : Theme.color.label
        SettingsButton {
            text: CloudDrives.installing ? "Installing…" : "Install"
            icon: "download-simple"
            busy: CloudDrives.installing
            enabled: !Updates.running
            onClicked: CloudDrives.install()
        }
    }
    Row {
        visible: CloudDrives.hasRclone
        label: root.d.connected ? "~/" + root.d.folder.split("/").pop() : root.name + " as a folder"
        detail: root.working ? "Working…" : root.d.connected ? (root.d.mounted ? "Connected" : "Connected · not mounted") : "Not connected"
        QQ.Row {
            spacing: 8
            SettingsButton {
                visible: root.d.connected && root.d.mounted
                text: "Open"
                icon: "folder-open"
                onClicked: CloudDrives.open(root.drive)
            }
            SettingsButton {
                visible: root.d.connected && !root.d.mounted
                text: "Mount"
                busy: root.working
                enabled: CloudDrives.busy === ""
                onClicked: CloudDrives.act("mount", root.drive)
            }
            SettingsButton {
                visible: root.d.connected
                text: "Disconnect"
                danger: true
                enabled: CloudDrives.busy === ""
                onClicked: CloudDrives.act("disconnect", root.drive)
            }
            SettingsButton {
                visible: !root.d.connected
                text: root.working ? "Waiting for the browser…" : "Connect " + root.name
                busy: root.working
                enabled: CloudDrives.busy === ""
                onClicked: CloudDrives.act("connect", root.drive)
            }
        }
    }
}
