pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Snapshots at startup (Settings → Software Update → Going back): whether
// this boot started from a snapshot (GRUB's snapshot menu, grub-btrfs), and
// whether that menu is kept up to date (grub-btrfs's daemon). Started from
// one, the island says so once, with the way to keep it.
Singleton {
    id: root

    property bool checked: false
    property string booted: "" // the snapshot this boot started from ("412", or a timeshift date)
    property bool btrfs: false
    property bool grub: false
    property bool grubBtrfs: false // grub-btrfs installed
    property bool menuOn: false // its daemon keeps the menu up to date
    property bool snapper: false
    property bool assistant: false // Btrfs Assistant (restores a snapshot for good)
    // Worth offering: GRUB on btrfs with snapper
    readonly property bool canMenu: btrfs && grub && snapper

    function refresh() {
        if (!probe.running)
            probe.running = true;
    }
    Process {
        id: probe
        command: ["sh", "-c", [
            "echo \"cmdline:$(cat /proc/cmdline 2>/dev/null)\"",
            "echo \"root:$(findmnt -no FSTYPE,OPTIONS / 2>/dev/null | tr -s ' ' ' ')\"",
            "{ findmnt -no FSTYPE / | grep -q btrfs || grep -q 'subvol=' /proc/cmdline; } 2>/dev/null && echo btrfs",
            "{ test -d /boot/grub || test -d /boot/grub2; } && command -v grub-mkconfig >/dev/null && echo grub",
            "command -v grub-btrfsd >/dev/null && echo grubBtrfs",
            "systemctl is-enabled --quiet grub-btrfsd 2>/dev/null && echo menuOn",
            "test -f /etc/snapper/configs/root && echo snapper",
            "command -v btrfs-assistant >/dev/null && echo assistant",
            ":"].join("; ")]
        stdout: StdioCollector {
            onStreamFinished: {
                const lines = text.split("\n");
                const has = k => lines.includes(k);
                for (const k of ["btrfs", "grub", "grubBtrfs", "menuOn", "snapper", "assistant"])
                    root[k] = has(k);
                const where = lines.filter(l => l.startsWith("cmdline:") || l.startsWith("root:")).join(" ");
                const snap = /snapshots\/(\d+)\/snapshot/.exec(where) ?? /timeshift-btrfs\/snapshots\/([^\/\s,]+)\//.exec(where);
                root.booted = snap ? snap[1] : "";
                root.checked = true;
            }
        }
    }
    Component.onCompleted: refresh()
    Connections {
        target: Updates
        function onFinished(j) {
            root.refresh(); // (grub-btrfs or Btrfs Assistant may have come)
        }
    }

    // Said once a boot, a little after the desktop is up
    Timer {
        interval: 15000
        running: root.checked && root.booted !== ""
        onTriggered: Island.post({
            kind: "custom", icon: "clock-counter-clockwise", title: "Started from a snapshot",
            subtitle: "Your system is as it was at snapshot " + root.booted + ". Keep it, or restart to go back to now.",
            color: Theme.accent, action: "Open Settings", onAction: () => Panels.openSettings("update")
        }, 15000)
    }

    // The boot menu's snapshots, on or off (iRice's root helper: one password)
    property bool busy: false
    property string error: ""
    function setMenu(on) {
        if (menuProc.running)
            return;
        error = "";
        if (Tools.ready && !Tools.has("pkexec")) {
            error = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.";
            return;
        }
        busy = true;
        menuProc.command = ["pkexec", "/usr/lib/irice/irice-root", "bootmenu", on ? "on" : "off"];
        menuProc.running = true;
    }
    Process {
        id: menuProc
        stderr: StdioCollector { id: menuErr }
        onExited: code => {
            root.busy = false;
            if (code !== 0 && code !== 126)
                root.error = code === 127 ? "Not allowed (an administrator's password is needed)."
                    : (menuErr.text.replace(/^irice-root: /, "").trim() || "That didn't change. Update iRice, then try again.");
            root.refresh();
        }
    }

    // Keeping the snapshot you started from: Btrfs Assistant restores it
    // (it knows each distribution's subvolume layout)
    function openAssistant() {
        Quickshell.execDetached(["btrfs-assistant"]);
    }
}
