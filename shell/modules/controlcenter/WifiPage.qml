import QtQuick
import Quickshell.Networking
import qs.common

// Wi-Fi in the menu bar's Wi-Fi menu and Control Center: the switch, the
// refresh button and the networks (WifiList, shared with Settings → Network).
DetailPage {
    id: root

    property alias list: networks

    title: "Wi-Fi"
    hasSwitch: true
    switchOn: Networking.wifiEnabled
    onSwitchToggled: Networking.wifiEnabled = !Networking.wifiEnabled
    hasRefresh: true
    onRefreshRequested: networks.rescan()

    WifiList {
        id: networks
        width: parent.width
        maxListHeight: 360
    }
}
