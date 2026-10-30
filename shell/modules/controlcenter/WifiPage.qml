import QtQuick
import Quickshell.Networking
import qs.common
import qs.services

// Wi-Fi in the menu bar's Wi-Fi menu and Control Center: the switch, the
// refresh button and the networks (WifiList, shared with Settings → Network).
DetailPage {
    id: root

    property alias list: networks

    title: "Wi-Fi"
    // (without NetworkManager, the list says who manages Wi-Fi instead)
    hasSwitch: !Backends.wifiUnmanaged
    switchOn: Networking.wifiEnabled
    onSwitchToggled: Networking.wifiEnabled = !Networking.wifiEnabled
    hasRefresh: !Backends.wifiUnmanaged
    onRefreshRequested: networks.rescan()

    WifiList {
        id: networks
        width: parent.width
        maxListHeight: 360
    }
}
