pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// What the newer features stand on, checked once (and again after an
// install or a fix), so a page can say what's missing on this system and
// offer to install it or turn it on, instead of failing:
//   avahi (finding TVs, speakers and phones on the network), PipeWire,
//   NetworkManager, a keyring (the Secret Service: iCloud's password),
//   GNOME Online Accounts (Microsoft 365: its Microsoft provider, 3.50+),
//   iRice's goa-signin helper (built by Software Update)
Singleton {
    id: root

    property bool checked: false
    property bool avahiInstalled: true
    property bool avahi: true
    property bool pipewire: true
    property bool networkManager: true
    property bool secretTool: true
    property bool keyring: true
    property bool goa: true
    property bool goaMicrosoft: true
    property bool goaSignin: true

    function refresh() {
        probe.running = true;
    }
    Process {
        id: probe
        command: ["sh", "-c", [
            "command -v avahi-daemon >/dev/null && echo avahiInstalled",
            // (systemd's word, else the daemon's own socket and process:
            // any init system)
            "{ systemctl is-active --quiet avahi-daemon 2>/dev/null || { test -S /run/avahi-daemon/socket && pgrep -x avahi-daemon >/dev/null; }; } && echo avahi",
            "timeout 3 pw-cli info 0 >/dev/null 2>&1 && echo pipewire",
            "{ systemctl is-active --quiet NetworkManager 2>/dev/null || pgrep -x NetworkManager >/dev/null; } && echo networkManager",
            "command -v secret-tool >/dev/null && echo secretTool",
            "{ busctl --user list 2>/dev/null; busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ListActivatableNames 2>/dev/null; } | grep -q org.freedesktop.secrets && echo keyring",
            "{ test -x /usr/lib/goa-daemon || test -x /usr/libexec/goa-daemon; } && echo goa",
            "v=$(pacman -Q gnome-online-accounts 2>/dev/null | awk '{print $2}'); [ -n \"$v\" ] && [ \"$(vercmp \"$v\" 3.50)\" -ge 0 ] && echo goaMicrosoft",
            "test -x \"$1/native/build/goa-signin\" || command -v irice-goa-signin >/dev/null && echo goaSignin",
            ":"].join("; "), "sh", Quickshell.shellDir]
        stdout: StdioCollector {
            onStreamFinished: {
                const has = k => text.split("\n").includes(k);
                for (const k of ["avahiInstalled", "avahi", "pipewire", "networkManager", "secretTool", "keyring", "goa", "goaMicrosoft", "goaSignin"])
                    root[k] = has(k);
                root.checked = true;
            }
        }
    }
    Component.onCompleted: refresh()

    // iRice's native helpers (goa-signin among them): built here, as you
    property bool building: false
    property string buildError: ""
    function buildNative() {
        if (builder.running)
            return;
        building = true;
        buildError = "";
        builder.running = true;
    }
    Process {
        id: builder
        command: [Quickshell.shellDir + "/native/build.sh"]
        stderr: StdioCollector { id: buildErr }
        // (no build.sh: nothing ends, so it is said here)
        onRunningChanged: if (!running && root.building) {
            root.building = false;
            root.refresh();
        }
        onExited: code => {
            root.building = false;
            if (code !== 0)
                root.buildError = "The build didn't work: " + (buildErr.text.trim().split("\n").pop() || "cmake, a C compiler and the Qt 6 development files are needed (base-devel, cmake, qt6-declarative).");
            root.refresh();
        }
    }

    // Installing what's missing (Software Update's job), then a fresh look
    property bool installAsked: false
    property var installPackages: []
    readonly property bool installing: Updates.running && installAsked
    function install(packages) {
        if (Updates.running || packages.length === 0)
            return;
        installAsked = true;
        installPackages = packages;
        Updates.update(["--install", packages.join(",")]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (root.installAsked) {
                root.installAsked = false;
                Tools.recheck();
                // (Online Accounts just came: its sign-in helper can be built now)
                if (j.state === "done" && !root.goaSignin && root.installPackages.includes("gnome-online-accounts"))
                    root.buildNative();
                else
                    root.refresh();
            }
        }
    }
    // Turning a service on (iRice's root helper: the fixes in System health)
    function turnOnAvahi() {
        if (!avahiInstalled)
            install(["avahi"]);
        else
            Health.fix("avahi");
    }
    Connections {
        target: Health
        function onFixDone(id) {
            root.refresh();
        }
    }
}
