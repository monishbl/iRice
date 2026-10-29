import QtQuick
import QtQuick as QQ
import Quickshell.Bluetooth
import qs.common
import qs.modules.controlcenter
import qs.modules.settings
import qs.services

// Settings → Bluetooth: on and off, your devices (connect, disconnect,
// forget) and devices nearby to pair, looked for while this page is open.
// The lists are the Bluetooth menu's (BluetoothList), right-click menu
// included.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    readonly property string pageTitle: "Bluetooth"
    readonly property var adapter: Bluetooth.defaultAdapter

    Group {
        width: parent.width
        footer: !page.adapter ? "No Bluetooth adapter found. If this computer has one, check that it isn't switched off with a key or in its firmware settings, and that the bluetooth service is running (systemctl enable --now bluetooth)." : ""
        Row {
            label: "Bluetooth"
            detail: !page.adapter ? "Not available" : page.adapter.enabled ? "On · this computer appears as “" + (page.adapter.name || "this computer") + "”" : "Off"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: page.adapter?.enabled ?? false
                    text: "Look again"
                    icon: "arrow-clockwise"
                    onClicked: nearby.rescan()
                }
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.adapter !== null
                    checked: page.adapter?.enabled ?? false
                    onToggled: if (page.adapter) page.adapter.enabled = !page.adapter.enabled
                }
            }
        }
    }

    Group {
        visible: page.adapter?.enabled ?? false
        width: parent.width
        title: "My devices"
        footer: "Click a device to connect or disconnect it. Right-click for more: connect automatically, forget, copy its address."
        Item {
            width: parent.width
            height: mine.implicitHeight + 16
            BluetoothList {
                id: mine
                x: 8
                y: 8
                width: parent.width - 16
                which: "paired"
                discover: false
                inSettings: true
                maxListHeight: 100000 // (the page scrolls)
            }
        }
    }

    Group {
        visible: page.adapter?.enabled ?? false
        width: parent.width
        title: "Nearby devices"
        footer: "Looking for devices while this page is open. Put the device in pairing mode to see it here, then click it to pair."
        Item {
            width: parent.width
            height: nearby.implicitHeight + 16
            BluetoothList {
                id: nearby
                x: 8
                y: 8
                width: parent.width - 16
                which: "nearby"
                inSettings: true
                maxListHeight: 100000
            }
        }
    }
}
