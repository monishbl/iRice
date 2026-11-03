pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// The boot animation (Settings → Lock & Login Screen): Plymouth's theme, read
// from its config (readable by all), and set through iRice's root helper
// (irice-root bootsplash, which always asks for the password: it rebuilds
// the boot image). The choices: the system's own theme (the one set before
// iRice changed it, which the helper remembers), one of iRice's designs
// (tools/make-bootsplash.py; their names in assets/plymouth/designs.json),
// or none (Plymouth's text theme).
//   BootSplash.chosen   "system" | a design's id | "none"
//   BootSplash.set(id)
Singleton {
    id: root

    property bool checked: false
    property bool installed: false // Plymouth is
    property bool helper: false // the installed helper knows bootsplash (and has the themes)
    property bool splash: true // "splash" is on the kernel's command line (else Plymouth stays hidden)
    property bool hooked: true // plymouth is in mkinitcpio's HOOKS
    property string builder: "" // what builds the boot image: mkinitcpio, dracut, booster
    property string theme: "" // Plymouth's theme now
    property var record: null // what the helper remembers: { original, applied }

    readonly property var designs: {
        try {
            return JSON.parse(designsFile.text());
        } catch (e) {
            return [];
        }
    }
    FileView {
        id: designsFile
        path: Quickshell.shellDir + "/assets/plymouth/designs.json"
    }
    // The system's own theme: as it was before iRice, unless set since by hand
    readonly property string original: record && record.applied === theme ? record.original : theme
    readonly property string chosen: {
        const id = theme.startsWith("irice-") ? theme.slice(6) : "";
        if (designs.some(d => d.id === id))
            return id;
        return theme === "text" && original !== "text" ? "none" : "system";
    }

    function refresh() {
        if (!probe.running)
            probe.running = true;
    }
    Process {
        id: probe
        command: ["sh", "-c", [
            "command -v plymouth-set-default-theme >/dev/null && test -d /usr/share/plymouth/themes && echo installed",
            "t=$(sed -n 's/^[[:space:]]*Theme[[:space:]]*=[[:space:]]*//p' /etc/plymouth/plymouthd.conf 2>/dev/null | tail -n 1)",
            "[ -n \"$t\" ] || t=$(sed -n 's/^[[:space:]]*Theme[[:space:]]*=[[:space:]]*//p' /usr/share/plymouth/plymouthd.defaults 2>/dev/null | tail -n 1)",
            "echo \"theme:$t\"",
            "echo \"record:$(head -c 512 /etc/irice/bootsplash.json 2>/dev/null)\"",
            "grep -qw splash /proc/cmdline && echo splash",
            "for b in mkinitcpio dracut booster; do command -v $b >/dev/null && { echo \"builder:$b\"; break; }; done",
            "grep -hE '^[[:space:]]*HOOKS[+]?=' /etc/mkinitcpio.conf /etc/mkinitcpio.conf.d/*.conf 2>/dev/null | grep -qw plymouth && echo hooked",
            "grep -q 'org.irice.root.bootsplash' /usr/share/polkit-1/actions/org.irice.root.policy 2>/dev/null && test -x /usr/lib/irice/irice-root && test -d /usr/lib/irice/plymouth/irice-breathe && echo helper",
            ":"].join("; ")]
        stdout: StdioCollector {
            onStreamFinished: {
                const lines = text.split("\n");
                const value = k => (lines.find(l => l.startsWith(k + ":")) ?? "").slice(k.length + 1).trim();
                for (const k of ["installed", "splash", "hooked", "helper"])
                    root[k] = lines.includes(k);
                root.theme = value("theme");
                root.builder = value("builder");
                try {
                    const r = JSON.parse(value("record") || "null");
                    root.record = r && typeof r.original === "string" && typeof r.applied === "string" ? r : null;
                } catch (e) {
                    root.record = null;
                }
                root.checked = true;
            }
        }
    }

    // Why a choice can't be made here, and what to do (else "")
    readonly property string blocked: !checked ? ""
        : !installed ? "Your computer starts without a boot animation: Plymouth isn't installed. Install the plymouth package, add plymouth to HOOKS in /etc/mkinitcpio.conf, then come back here."
        : builder === "dracut" ? "This computer builds its boot image with dracut, which iRice doesn't rebuild. To use one of these, run in a terminal: sudo plymouth-set-default-theme -R irice-breathe (or another design's name)."
        : builder === "booster" ? "This computer builds its boot image with booster, which doesn't show boot animations."
        : builder === "" ? "There's no mkinitcpio to rebuild the boot image with."
        : !hooked ? "Plymouth isn't in the boot image. Add plymouth to HOOKS in /etc/mkinitcpio.conf (after kms), then come back here."
        : !helper ? "Update iRice first (Settings → Software Update): the update installs the boot animations. Then pick one here."
        : ""

    // Setting one (the password, then about a minute while the boot image is rebuilt)
    property string pending: "" // the choice being set
    property string error: ""
    property bool done: false
    readonly property bool busy: pending !== ""
    function set(id) {
        if (busy || id === chosen || blocked !== "")
            return;
        error = "";
        done = false;
        if (System.sandboxed) {
            error = "Not from the test session: it would change this computer's boot animation.";
            return;
        }
        if (Tools.ready && !Tools.has("pkexec")) {
            error = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.";
            return;
        }
        pending = id;
        setProc.command = ["pkexec", "/usr/lib/irice/irice-root", "bootsplash", id];
        setProc.running = true;
    }
    Process {
        id: setProc
        stderr: StdioCollector { id: setErr }
        onExited: code => {
            root.pending = "";
            root.done = code === 0;
            // (126: the password prompt was dismissed; nothing changed)
            if (code !== 0 && code !== 126)
                root.error = code === 127 ? "Not allowed: an administrator's password is needed."
                    : (setErr.text.replace(/^irice-root: /, "").trim().replace(/^./, c => c.toUpperCase()) || "That didn't change. Update iRice, then try again.");
            root.refresh();
        }
    }
}
