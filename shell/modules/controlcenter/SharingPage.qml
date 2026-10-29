import QtQuick
import Quickshell
import qs.common
import qs.services

// Sharing, in detail: who can see this computer, set together (the switch
// and the choices at the top) or separately for LocalSend and Quick Share, the
// name others see, and what's nearby. Settings → Sharing has the rest.
DetailPage {
    id: root

    title: "Sharing"
    hasSwitch: true
    switchOn: Sharing.on
    onSwitchToggled: Sharing.setLevel(Sharing.on ? "off" : "10min")

    // Devices nearby while the page is up
    property bool lookingHere: false
    onVisibleChanged: look(visible)
    Component.onCompleted: look(visible)
    Component.onDestruction: look(false)
    function look(on) {
        if (on !== lookingHere) {
            lookingHere = on;
            Sharing.look(on);
        }
    }

    Segmented {
        width: parent.width
        readonly property var values: ["off", "10min", "everyone"]
        options: ["No one", "10 minutes", "Everyone"]
        current: Math.max(0, values.indexOf(Sharing.level))
        onSelected: i => Sharing.setLevel(values[i])
    }

    Caption { text: "LocalSend" }
    Segmented {
        width: parent.width
        height: 30
        readonly property var values: ["off", "10min", "everyone"]
        options: ["No one", "10 minutes", "Everyone"]
        current: Math.max(0, values.indexOf(Config.options.sharing.receiving))
        onSelected: i => Config.options.sharing.receiving = values[i]
    }
    Hint { text: "Computers and phones with the LocalSend app" }

    Caption { text: "Quick Share" }
    Segmented {
        width: parent.width
        height: 30
        readonly property var values: ["off", "10min", "everyone"]
        options: ["No one", "10 minutes", "Everyone"]
        current: Math.max(0, values.indexOf(Config.options.sharing.quickShare))
        onSelected: i => Config.options.sharing.quickShare = values[i]
    }
    // (Whether Quick Share needs Bluetooth, and if it's on: only once the
    // service says; QuickShare may not have these yet)
    Hint {
        readonly property bool needs: QuickShare.bluetoothNeeded ?? false
        readonly property bool btOn: QuickShare.bluetoothOn ?? true
        text: needs && !btOn ? "Turn on Bluetooth so Android phones can find this computer." : "Android phones nearby"
        color: needs && !btOn ? Theme.color.danger : Theme.color.tertiary
    }

    Caption { text: "Your name" }
    TextField {
        width: parent.width
        height: 30
        placeholder: (Quickshell.env("USER") ?? "iRice") + "'s computer"
        text: Config.options.sharing.name
        onCommitted: t => Config.options.sharing.name = t.trim()
    }

    // Choose files, then a device (the same as Settings → Sharing)
    ListRow {
        icon: "paper-plane-tilt"
        title: "Send files…"
        status: "To a phone or computer nearby"
        enabled: LocalSend.available && !FilePicker.busy
        onClicked: Panels.pickShare()
    }

    Caption { text: "Nearby" }
    Repeater {
        model: Sharing.devices
        ListRow {
            required property var modelData
            readonly property var sending: Sharing.progressFor(modelData)
            icon: modelData.type === "mobile" ? "device-mobile" : modelData.type === "laptop" ? "laptop" : "desktop"
            title: modelData.name
            status: sending ? (sending.waiting ? "Waiting for them to accept…" : Math.round(100 * (sending.total > 0 ? sending.done / sending.total : 0)) + "% sent")
                : modelData.prefer === "quickshare" ? "Quick Share" : "LocalSend"
            trailingIcon: "paper-plane-tilt"
            onClicked: if (!sending) Sharing.pickFor(modelData)
        }
    }
    Hint {
        visible: Sharing.devices.length === 0
        text: "No devices nearby. Make sure they're on the same Wi-Fi with LocalSend open, or Quick Share visible to everyone."
    }
    Hint {
        readonly property string errors: [LocalSend.error, QuickShare.error].filter(e => !!e).join(" ")
        visible: errors !== ""
        text: errors
        color: Theme.color.danger
    }

    component Caption: Text {
        topPadding: 6
        leftPadding: 6
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }
    component Hint: Text {
        width: parent?.width ?? 0
        leftPadding: 6
        rightPadding: 6
        wrapMode: Text.Wrap
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
}
