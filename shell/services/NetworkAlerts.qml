pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Island alerts when the network changes: joined or left a Wi-Fi network,
// a VPN connected or disconnected, the hotspot turned on or off, Ethernet
// plugged in. Something says that it changed; then what's connected is
// compared with the last look.
//   - NetworkManager running: `nmcli monitor`, then its active connections
//   - otherwise (iwd, ConnMan, systemd-networkd…): `ip monitor link address`,
//     then the kernel's interfaces: Wi-Fi (with its network name when `iw`
//     is installed), Ethernet, and VPN tunnels (wg*, tailscale*, tun*, …)
Singleton {
    id: root

    // nm | ip | "" (nothing to watch with)
    readonly property string mode: Backends.nmActive && Tools.has("nmcli") ? "nm" : Tools.has("ip") ? "ip" : ""
    readonly property bool canWatch: !Tools.ready || mode !== ""
    // For Settings, when it can't: why
    readonly property string reason: canWatch ? "" : "Needs NetworkManager, or the ip command (install iproute2)"
    readonly property bool on: Island.enabled && Island.wants("network") && mode !== "" && Backends.ready
    property var last: null // name -> { kind, hotspot }
    onModeChanged: last = null // (another way of looking: learn the state again)

    Process {
        id: monitor
        running: root.on
        command: root.mode === "nm" ? [Tools.watcher, "nmcli", "monitor"] : [Tools.watcher, "ip", "monitor", "link", "address"]
        stdinEnabled: true // the watcher ends when this closes (the shell quit)
        stdout: SplitParser {
            onRead: settle.restart()
        }
        onRunningChanged: if (running) root.look()
    }
    Timer {
        id: settle
        interval: 700 // connecting prints several lines; look once it settles
        onTriggered: root.look()
    }
    function look() {
        if (mode === "nm")
            list.running = true;
        else if (mode === "ip")
            links.running = true;
    }
    Process {
        id: list
        // NAME:TYPE:DEVICE, then whether each Wi-Fi connection is an access point
        command: ["sh", "-c", "nmcli -t -f NAME,TYPE,DEVICE connection show --active | while IFS= read -r l; do t=$(printf '%s' \"$l\" | awk -F: '{print $(NF-1)}'); m=; [ \"$t\" = 802-11-wireless ] && m=$(nmcli -g 802-11-wireless.mode connection show \"${l%:*:*}\" 2>/dev/null); printf '%s:%s\\n' \"$l\" \"$m\"; done"]
        stdout: StdioCollector {
            onStreamFinished: root.compare(root.parseNm(text))
        }
    }
    // Without NetworkManager: each interface that's up and has an address,
    // as kind<TAB>interface<TAB>Wi-Fi network name<TAB>access point (1)
    Process {
        id: links
        command: ["sh", "-c", "for d in /sys/class/net/*; do n=${d##*/}; [ \"$n\" = lo ] && continue; " +
            "s=$(cat \"$d/operstate\" 2>/dev/null); [ \"$s\" = down ] || [ \"$s\" = lowerlayerdown ] && continue; " +
            "[ -n \"$(ip -o addr show dev \"$n\" scope global 2>/dev/null)\" ] || continue; ssid=; ap=; " +
            "if [ -d \"$d/wireless\" ] || [ -e \"$d/phy80211\" ]; then k=wifi; " +
            "if command -v iw >/dev/null; then i=$(iw dev \"$n\" info 2>/dev/null); ssid=$(printf '%s\\n' \"$i\" | sed -n 's/^[[:space:]]*ssid //p' | head -n 1); printf '%s' \"$i\" | grep -q 'type AP' && ap=1; fi; " +
            "else case \"$n\" in wg*|tailscale*|tun*|tap*|ppp*|nordlynx*|proton*|mullvad*|zt*|ipsec*|vti*) k=vpn ;; *) if [ -e \"$d/device\" ]; then k=ethernet; else continue; fi ;; esac; fi; " +
            "printf '%s\\t%s\\t%s\\t%s\\n' \"$k\" \"$n\" \"$ssid\" \"$ap\"; done"]
        stdout: StdioCollector {
            onStreamFinished: root.compare(root.parseLinks(text))
        }
    }

    function kindOf(type) {
        return type === "802-11-wireless" ? "wifi" : type === "vpn" || type === "wireguard" ? "vpn" : type === "802-3-ethernet" ? "ethernet" : "";
    }
    function parseNm(text) {
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
        return now;
    }
    // A VPN interface's name, as people know it
    function vpnName(n) {
        return /^tailscale/.test(n) ? "Tailscale" : /^nordlynx/.test(n) ? "NordVPN" : /^proton/.test(n) ? "Proton VPN" : /^mullvad/.test(n) ? "Mullvad" : /^zt/.test(n) ? "ZeroTier" : n;
    }
    function parseLinks(text) {
        const now = {};
        for (const l of text.split("\n")) {
            const f = l.split("\t");
            if (f.length < 4)
                continue;
            const [kind, iface, ssid, ap] = f;
            const name = kind === "wifi" ? (ssid || "Wi-Fi") : kind === "vpn" ? vpnName(iface) : "Ethernet " + iface;
            now[name] = { kind: kind, hotspot: ap === "1" };
        }
        // (the menu bar's Wi-Fi item shows the link without NetworkManager)
        if (Backends.wifiUnmanaged)
            Backends.recheck();
        return now;
    }
    function compare(now) {
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
