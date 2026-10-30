pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Bluetooth
import Quickshell.Networking
import qs.common

// What this computer actually runs underneath iRice, so a feature whose
// backend isn't there says so (and how to fix it, where there's a fix)
// instead of quietly not working (assets/scripts/system-stack.sh):
//   - Wi-Fi: Quickshell.Networking only speaks NetworkManager; with iwd,
//     ConnMan, wpa_supplicant or systemd-networkd in charge the Wi-Fi menus
//     say so and offer that manager's own app
//   - energy modes: power-profiles-daemon, or TLP / auto-cpufreq / tuned /
//     system76-power managing power instead (no modes then)
//   - Bluetooth: no adapter, BlueZ not installed, its service stopped, or
//     switched off with rfkill
// Looked at once at startup, and again with recheck() (System health
// refreshing, after a fix, the Wi-Fi link changing).
Singleton {
    id: root

    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/system-stack.sh").toString().replace("file://", ""))
    property bool ready: false
    property var found: ({})

    function recheck() {
        if (proc.running)
            again = true;
        else
            proc.running = true;
    }
    property bool again: false
    Process {
        id: proc
        running: true
        command: ["bash", root.script]
        stdout: StdioCollector {
            onStreamFinished: {
                const f = {};
                for (const l of text.split("\n")) {
                    const i = l.indexOf("=");
                    if (i > 0)
                        f[l.slice(0, i)] = l.slice(i + 1);
                }
                root.found = f;
                root.ready = true;
            }
        }
        onExited: if (root.again) {
            root.again = false;
            running = true;
        }
    }
    // System health refreshing, or one of its fixes done (Start Bluetooth, …)
    Connections {
        target: Health
        function onCheckingChanged() {
            if (Health.checking)
                root.recheck();
        }
        function onFixDone(id) {
            if (id === "bluetooth") {
                root.btFixing = false;
                if (Health.error !== "")
                    root.btFixError = Health.error;
            }
            root.recheck();
        }
    }

    // ── Wi-Fi ───────────────────────────────────────────────────────────
    // NetworkManager answers (Quickshell found it at startup)
    readonly property bool nmActive: Networking.backend === NetworkBackendType.NetworkManager
    // Who manages the network: networkmanager | iwd | connman | wpa_supplicant | systemd-networkd | none
    readonly property string netManager: found.net ?? ""
    readonly property var netNames: ({ "networkmanager": "NetworkManager", "iwd": "iwd", "connman": "ConnMan", "wpa_supplicant": "wpa_supplicant", "systemd-networkd": "systemd-networkd" })
    readonly property string netManagerName: netNames[netManager] ?? ""
    readonly property bool hasWifiHardware: (found.wifi_iface ?? "") !== ""
    readonly property bool wifiLinkUp: found.wifi_up === "1"
    // iRice's Wi-Fi menus can't work: the reason, else ""
    readonly property bool wifiUnmanaged: ready && !nmActive
    readonly property string wifiTitle: !wifiUnmanaged ? "" : netManager === "networkmanager" ? "Restart iRice for Wi-Fi" : netManagerName !== "" ? "Wi-Fi is managed by " + netManagerName : "No network service is running"
    readonly property string wifiReason: !wifiUnmanaged ? ""
        : netManager === "networkmanager" ? "NetworkManager started after iRice did. Restart iRice (Settings → About) to use its Wi-Fi menus."
        : netManagerName !== "" ? "iRice's Wi-Fi menus need NetworkManager. Use " + (wifiApp !== "" ? wifiAppName : netManagerName + "'s own tools") + " to choose a network."
        : nmInstalled ? "iRice's Wi-Fi menus need NetworkManager, which is installed but not running." : "iRice's Wi-Fi menus need NetworkManager. Install networkmanager, or set up another network service."
    readonly property string wifiShort: !wifiUnmanaged ? "" : netManagerName !== "" && netManager !== "networkmanager" ? "Managed by " + netManagerName : "Needs NetworkManager"
    readonly property bool nmInstalled: found.nm_installed === "1"
    // NetworkManager is installed, nothing else runs: it can just be started
    readonly property bool canStartNm: wifiUnmanaged && netManager === "none" && nmInstalled
    readonly property string wifiApp: found.wifi_app ?? ""
    readonly property bool wifiAppTerminal: found.wifi_app_terminal === "1"
    readonly property string wifiAppName: ({ "iwgtk": "iwgtk", "iwgui": "iwgui", "impala": "impala", "iwctl": "iwctl", "cmst": "CMST", "connman-gtk": "connman-gtk", "connmanctl": "connmanctl", "wpa_gui": "wpa_gui", "wpa_cli": "wpa_cli" })[wifiApp] ?? wifiApp
    function openWifiApp() {
        if (wifiApp === "")
            return;
        Panels.close();
        if (wifiAppTerminal)
            Launch.terminal([wifiApp], false);
        else
            Launch.run([wifiApp]);
    }
    function startNm() {
        Health.fix("networkmanager");
    }

    // ── Energy modes ────────────────────────────────────────────────────
    // power-profiles-daemon (or tuned-ppd) is there to switch modes
    readonly property bool ppd: !ready || found.ppd === "1"
    readonly property bool ppdInstalled: found.ppd_installed === "1"
    readonly property string powerDaemon: found.powerd ?? "" // tlp | auto-cpufreq | tuned | system76-power
    readonly property string powerDaemonName: ({ "tlp": "TLP", "auto-cpufreq": "auto-cpufreq", "tuned": "TuneD", "system76-power": "system76-power" })[powerDaemon] ?? powerDaemon
    // Short, for a tile or a disabled row: "Managed by TLP" | "Needs power-profiles-daemon"
    readonly property string powerShort: ppd ? "" : powerDaemon !== "" ? "Managed by " + powerDaemonName : "Needs power-profiles-daemon"
    readonly property string powerReason: ppd ? ""
        : powerDaemon !== "" ? powerDaemonName + " manages power on this computer, so iRice doesn't switch energy modes. Change them in " + powerDaemonName + "'s own settings."
        : ppdInstalled ? "Energy modes need power-profiles-daemon, which is turned off for good (masked). sudo systemctl unmask power-profiles-daemon turns it back on."
        : "Energy modes need power-profiles-daemon. Install power-profiles-daemon to use Low Power and Performance."

    // ── Bluetooth ───────────────────────────────────────────────────────
    // ok | no-adapter | not-installed | stopped | blocked | hard-blocked
    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property string btState: {
        if (adapter && adapter.state !== BluetoothAdapterState.Blocked)
            return "ok";
        if (!ready)
            return adapter ? "ok" : "";
        if (found.bt_hard === "1")
            return "hard-blocked";
        if (found.bt_soft === "1" || adapter?.state === BluetoothAdapterState.Blocked)
            return "blocked";
        if (found.bt_hardware !== "1")
            return "no-adapter";
        if (found.bt_installed !== "1")
            return "not-installed";
        if (found.bt_running !== "1")
            return "stopped";
        return "no-adapter";
    }
    readonly property string btTitle: ({
            "no-adapter": "No Bluetooth adapter",
            "not-installed": "Bluetooth isn't installed",
            "stopped": "Bluetooth is turned off",
            "blocked": "Bluetooth is blocked",
            "hard-blocked": "Bluetooth is switched off"
        })[btState] ?? ""
    readonly property string btReason: ({
            "no-adapter": "No Bluetooth adapter found. If this computer has one, check that it's turned on in its firmware settings.",
            "not-installed": "Install bluez and bluez-utils, then start Bluetooth (sudo systemctl enable --now bluetooth).",
            "stopped": "The Bluetooth service isn't running. Start it to use Bluetooth devices.",
            "blocked": "Bluetooth is blocked by rfkill, as in flight mode. Unblock it to use Bluetooth devices.",
            "hard-blocked": "Bluetooth is switched off with a key or switch on this computer, or in its firmware settings. Turn it on there."
        })[btState] ?? ""
    // For a tile: "No adapter" | "Not installed" | "Service off" | "Blocked" | "Switched off"
    readonly property string btShort: ({ "no-adapter": "No adapter", "not-installed": "Not installed", "stopped": "Service off", "blocked": "Blocked", "hard-blocked": "Switched off" })[btState] ?? ""
    // The one button, if there is one: start | unblock | ""
    readonly property string btFix: btState === "stopped" ? "start" : btState === "blocked" ? "unblock" : ""
    readonly property string btFixLabel: btFix === "start" ? "Start Bluetooth" : btFix === "unblock" ? "Unblock Bluetooth" : ""
    property bool btFixing: false
    property string btFixError: ""
    function fixBluetooth() {
        btFixError = "";
        if (btFix === "start") {
            if (Health.fixing !== "") {
                btFixError = "Another fix is running. Try again in a moment.";
                return;
            }
            btFixing = true;
            Health.fix("bluetooth"); // systemctl enable --now bluetooth, through iRice's root helper
        } else if (btFix === "unblock") {
            btFixing = true;
            unblock.running = true;
        }
    }
    Process {
        id: unblock
        command: ["rfkill", "unblock", "bluetooth"]
        onExited: code => {
            root.btFixing = false;
            if (code !== 0)
                root.btFixError = "Couldn't unblock it. Run rfkill unblock bluetooth in a terminal.";
            else
                powerOn.restart();
            root.recheck();
        }
    }
    // Unblocked: turned on too (that's what the button was for), once BlueZ
    // has seen the change
    Timer {
        id: powerOn
        interval: 800
        onTriggered: if (root.adapter && root.adapter.state !== BluetoothAdapterState.Blocked) root.adapter.enabled = true
    }
}
