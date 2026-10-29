pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import "SharingDevices.js" as Devices

// Sharing with nearby devices, both ways iRice speaks: LocalSend (computers
// and phones running the LocalSend app) and Quick Share (Android). The share
// sheet, the Control Center module and its page use this, not the two
// protocols directly:
//   - level: who can see this computer, for both at once (off | 10min |
//     everyone); each can still be set on its own (the page, Settings)
//   - devices: everything nearby as one list, a device seen both ways once
//   - look(): find devices while something is showing them (counted, so the
//     sheet and a Control Center module can both be open)
//   - shareText(): text (a selection, a link) goes as a small text file, the
//     one kind of item both protocols take
Singleton {
    id: root

    readonly property var cfg: Config.options.sharing
    readonly property string level: Devices.combine(cfg.receiving, cfg.quickShare)
    // The two differ (set separately on the page): the module says so
    readonly property bool mixed: cfg.receiving !== cfg.quickShare
    readonly property bool on: level !== "off"
    function setLevel(l) {
        cfg.receiving = l;
        cfg.quickShare = l;
    }
    readonly property string levelText: level === "off" ? "Off" : mixed ? "Custom" : level === "10min" ? "Everyone for 10 min" : "Everyone"

    // ── Nearby devices ──────────────────────────────────────────────────
    readonly property var devices: Devices.merge(LocalSend.peers, QuickShare.devices)
    property int lookers: 0
    readonly property bool looking: lookers > 0
    onLookingChanged: {
        LocalSend.sharing = looking;
        QuickShare.looking = looking;
        if (looking)
            LocalSend.scan();
    }
    function look(on) {
        lookers = Math.max(0, lookers + (on ? 1 : -1));
    }

    // Send to a device from `devices`: Quick Share for Android phones (no
    // app to open on the phone), LocalSend for the rest
    function send(device, paths) {
        if (!device || !paths || paths.length === 0)
            return;
        if (device.prefer === "quickshare")
            QuickShare.send(device.ids.quickshare, paths);
        else
            LocalSend.send(device.ids.localsend, paths);
    }
    // The send to this device in flight, for its progress ring
    function progressFor(device) {
        return Devices.progressFor(LocalSend.items.concat(QuickShare.items), device);
    }
    // Pick files, then send them to this device
    function pickFor(device) {
        FilePicker.pick({ multiple: true, title: "Send files to " + device.name }, paths => root.send(device, paths));
    }

    // ── Text ────────────────────────────────────────────────────────────
    // Written to a private folder as a .txt file; the sheet shows the text
    // itself. (Quick Share can also send text and links natively; that needs
    // a sendText in QuickShare.qml, which doesn't exist yet: TODO.)
    readonly property string textDir: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-share"
    function shareText(text, scr) {
        const t = String(text ?? "").trim();
        if (t === "") {
            note("Nothing selected", "Select some text, then try again.");
            return;
        }
        const file = textDir + "/" + (/^https?:\/\/\S+$/.test(t) ? "Link" : "Text") + " " + Qt.formatDateTime(new Date(), "yyyy-MM-dd hh.mm.ss") + ".txt";
        writer.pending = { file: file, text: t, screen: scr };
        writer.command = ["sh", "-c", "umask 077; mkdir -p \"$1\" && printf '%s\\n' \"$3\" > \"$2\"", "sh", textDir, file, t];
        writer.running = true;
    }
    Process {
        id: writer
        property var pending: null
        onExited: code => {
            const p = pending;
            pending = null;
            if (!p)
                return;
            if (code !== 0) {
                root.note("Couldn't share the text", "The temporary folder isn't writable. Check that " + root.textDir + " can be created.");
                return;
            }
            Panels.share([p.file], p.screen, p.text);
        }
    }
    // Share what's selected (the primary selection), else what's copied
    function shareSelection() {
        selection.running = true;
    }
    Process {
        id: selection
        command: ["sh", "-c", "t=$(wl-paste -p -n 2>/dev/null); [ -n \"$t\" ] || t=$(wl-paste -n -t text 2>/dev/null); printf '%s' \"$t\""]
        stdout: StdioCollector {
            onStreamFinished: root.shareText(text)
        }
    }

    function note(title, subtitle) {
        if (Config.options.island.enabled)
            Island.post({ kind: "custom", icon: "share-network", title: title, subtitle: subtitle, color: Theme.accent }, 4000);
        else
            Quickshell.execDetached(["notify-send", "-a", "iRice", title, subtitle]);
    }
}
