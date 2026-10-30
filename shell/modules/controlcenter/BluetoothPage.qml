import QtQuick
import Quickshell.Bluetooth
import qs.common
import qs.services

// Bluetooth in the menu bar's Bluetooth menu and Control Center: the switch,
// the refresh button and the devices (BluetoothList, shared with Settings →
// Bluetooth).
DetailPage {
    id: root

    readonly property var adapter: Bluetooth.defaultAdapter

    title: "Bluetooth"
    // (Bluetooth not working: the list says why instead)
    hasSwitch: Backends.btState === "ok"
    switchOn: adapter?.enabled ?? false
    onSwitchToggled: if (adapter) adapter.enabled = !adapter.enabled
    hasRefresh: Backends.btState === "ok"
    onRefreshRequested: devices.rescan()

    BluetoothList {
        id: devices
        width: parent.width
        maxListHeight: 360
    }
}
