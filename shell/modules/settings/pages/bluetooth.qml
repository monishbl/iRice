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

    // Not working (Backends.btState): why, and the fix when there's one
    readonly property bool broken: Backends.btState !== "ok" && Backends.btState !== ""
    Timer { // (rfkill and the service change without BlueZ telling)
        running: page.visible && page.broken
        interval: 3000
        repeat: true
        onTriggered: Backends.recheck()
    }

    Group {
        width: parent.width
        footer: Backends.btFixError
        Row {
            label: page.broken ? Backends.btTitle : "Bluetooth"
            detail: page.broken ? Backends.btReason : !page.adapter ? "Not available" : page.adapter.enabled ? "On · this computer appears as “" + (page.adapter.name || "this computer") + "”" : "Off"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: page.broken && Backends.btFix !== ""
                    primary: true
                    text: Backends.btFixing ? "Working…" : Backends.btFixLabel
                    busy: Backends.btFixing
                    onClicked: Backends.fixBluetooth()
                }
                SettingsButton {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !page.broken && (page.adapter?.enabled ?? false)
                    text: "Look again"
                    icon: "arrow-clockwise"
                    onClicked: nearby.rescan()
                }
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !page.broken
                    enabled: page.adapter !== null
                    checked: page.adapter?.enabled ?? false
                    onToggled: if (page.adapter) page.adapter.enabled = !page.adapter.enabled
                }
            }
        }
    }

    Group {
        visible: !page.broken && (page.adapter?.enabled ?? false)
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
        visible: !page.broken && (page.adapter?.enabled ?? false)
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

    // Fast Pair (services/FastPair)
    Group {
        width: parent.width
        footer: "Earbuds and headphones with Google Fast Pair (Pixel Buds, many Sony, JBL, Bose, OnePlus and others): open the case near the computer to connect them in one click, and to see their batteries. Looks only while Bluetooth is on."
        Row {
            label: "Show earbuds nearby"
            Switch { checked: Config.options.fastPair.enabled; onToggled: Config.options.fastPair.enabled = !checked }
        }
    }
}
