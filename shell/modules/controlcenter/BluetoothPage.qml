import QtQuick
import Quickshell.Bluetooth
import qs.common

// Bluetooth devices: paired ones first. Discovery runs while the page is open.
DetailPage {
    id: root

    readonly property var adapter: Bluetooth.defaultAdapter
    function sortedNow() {
        return (adapter?.devices.values ?? []).filter(d => d.paired || d.name).slice().sort((a, b) => (b.connected - a.connected) || (b.paired - a.paired) || (a.name ?? "").localeCompare(b.name ?? "")).slice(0, 12);
    }
    // The rows shown: a snapshot (see WifiPage), refreshed every few seconds
    // unless the pointer is over the list, and with the refresh button
    property var devices: []
    function refresh(force) {
        const next = sortedNow();
        if (!force && next.length === devices.length && next.every((d, i) => d === devices[i]))
            return;
        devices = next;
    }
    Component.onCompleted: refresh(true) // (a page created already showing gets no visibility change)
    onVisibleChanged: if (visible) refresh(true)
    Timer {
        running: root.visible
        interval: 4000
        repeat: true
        onTriggered: if (!root.hovered) root.refresh(false)
    }
    Connections {
        target: root.adapter?.devices ?? null
        ignoreUnknownSignals: true
        function onValuesChanged() {
            const live = root.adapter?.devices.values ?? [];
            if (root.devices.length === 0 || root.devices.some(d => !d || !live.includes(d)))
                root.refresh(false);
        }
    }
    hasRefresh: true
    onRefreshRequested: {
        if (adapter?.enabled) { // look again
            adapter.discovering = false;
            adapter.discovering = true;
        }
        refresh(true);
    }

    title: "Bluetooth"
    hasSwitch: true
    switchOn: adapter?.enabled ?? false
    onSwitchToggled: if (adapter) adapter.enabled = !adapter.enabled

    Binding {
        target: root.adapter
        property: "discovering"
        value: true
        when: root.visible && (root.adapter?.enabled ?? false)
        restoreMode: Binding.RestoreBindingOrValue
    }

    function deviceIcon(d) {
        const i = d.icon ?? "";
        return i.includes("headset") || i.includes("headphone") ? "headphones" : i.includes("phone") ? "device-mobile" : i.includes("keyboard") ? "keyboard" : i.includes("mouse") ? "mouse" : i.includes("audio") ? "speaker-hifi" : "bluetooth";
    }

    Text {
        visible: !root.adapter?.enabled || root.devices.length === 0
        width: parent.width
        topPadding: 12
        bottomPadding: 12
        horizontalAlignment: Text.AlignHCenter
        text: !root.adapter ? "No Bluetooth adapter" : !root.adapter.enabled ? "Bluetooth is off" : "Looking for devices…"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    Repeater {
        model: root.adapter?.enabled ? root.devices : []

        ListRow {
            required property var modelData
            icon: root.deviceIcon(modelData)
            highlighted: modelData.connected
            title: modelData.name || modelData.address
            status: modelData.connected ? (modelData.batteryAvailable ? "Connected · " + Math.round(modelData.battery * 100) + "%" : "Connected") : modelData.pairing ? "Pairing…" : modelData.paired ? "Not connected" : ""
            busy: modelData.state === BluetoothDeviceState.Connecting || modelData.state === BluetoothDeviceState.Disconnecting || modelData.pairing
            onClicked: {
                if (modelData.connected)
                    modelData.disconnect();
                else if (modelData.paired)
                    modelData.connect();
                else
                    modelData.pair();
            }
        }
    }
}
