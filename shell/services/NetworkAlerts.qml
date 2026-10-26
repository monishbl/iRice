pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Island alerts when the network changes: joined or left a Wi-Fi network,
// a VPN connected or disconnected, the hotspot turned on or off, Ethernet
// plugged in. `nmcli monitor` says something changed; then the list of
// active connections is compared with the last one.
Singleton {
    id: root

    readonly property bool on: Island.enabled && Island.wants("network") && Tools.has("nmcli")
    property var last: null // name -> { type, hotspot }

    Process {
        id: monitor
        running: root.on
        command: [Tools.watcher, "nmcli", "monitor"]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: settle.restart()
        }
        onRunningChanged: if (running) list.running = true
    }
    Timer {
        id: settle
        interval: 700 // connecting prints several lines; look once it settles
        onTriggered: list.running = true
    }
    Process {
        id: list
        // NAME:TYPE:DEVICE, then whether each Wi-Fi connection is an access point
        command: ["sh", "-c", "nmcli -t -f NAME,TYPE,DEVICE connection show --active | while IFS= read -r l; do t=$(printf '%s' \"$l\" | awk -F: '{print $(NF-1)}'); m=; [ \"$t\" = 802-11-wireless ] && m=$(nmcli -g 802-11-wireless.mode connection show \"${l%:*:*}\" 2>/dev/null); printf '%s:%s\\n' \"$l\" \"$m\"; done"]
        stdout: StdioCollector {
            onStreamFinished: root.compare(text)
        }
    }

    function kindOf(type) {
        return type === "802-11-wireless" ? "wifi" : type === "vpn" || type === "wireguard" ? "vpn" : type === "802-3-ethernet" ? "ethernet" : "";
    }
    function compare(text) {
        const now = {};
        for (const l of text.split("\n")) {
            // the name may contain ':' (escaped as '\:'); the last three fields are fixed
            const f = l.replace(/\\:/g, "\u0001").split(":");
            if (f.length < 4)
                continue;
            const name = f.slice(0, f.length - 3).join(":").replace(/\u0001/g, ":");
            const kind = kindOf(f[f.length - 3]);
            if (kind && f[f.length - 2] !== "lo")
                now[name] = { kind: kind, hotspot: f[f.length - 1] === "ap" };
        }
        const before = last;
        last = now;
        if (before === null)
            return; // the first look only learns the state
        for (const [name, c] of Object.entries(now))
            if (!before[name])
                announce(name, c, true);
        for (const [name, c] of Object.entries(before))
            if (!now[name])
                announce(name, c, false);
    }
    function announce(name, c, up) {
        const grey = Qt.rgba(1, 1, 1, 0.55);
        if (c.hotspot)
            Island.post({ kind: "network", icon: "broadcast", title: "Personal Hotspot", subtitle: up ? "On · " + name : "Off", color: up ? Theme.accents.green : grey, ripple: up }, 3000);
        else if (c.kind === "wifi")
            Island.post({ kind: "network", icon: up ? "wifi-high" : "wifi-slash", title: name, subtitle: up ? "Wi-Fi connected" : "Wi-Fi disconnected", color: up ? Theme.accents.blue : grey }, up ? 3000 : 2500);
        else if (c.kind === "vpn")
            Island.post({ kind: "network", icon: up ? "shield-check" : "shield-slash", title: name, subtitle: up ? "VPN connected" : "VPN disconnected", color: up ? Theme.accents.green : Theme.accents.orange, ripple: up }, 3000);
        else if (c.kind === "ethernet")
            Island.post({ kind: "network", icon: up ? "plugs-connected" : "network-slash", title: "Ethernet", subtitle: up ? "Connected" : "Unplugged", color: up ? Theme.accents.blue : grey, ripple: up }, 2500);
    }
}
