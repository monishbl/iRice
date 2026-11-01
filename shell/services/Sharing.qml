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

    // ── Incoming requests (modules/share/IncomingCard: AirDrop's alert) ──
    // One shown at a time, both protocols alike: asking (Accept / Decline),
    // then receiving (a ring around the sender), then done (Open, Show in
    // Files) or failed. Hidden: the transfer goes on in the island, and what
    // arrives is said there as before.
    // [{ id ("qs:in-1"), via, session, from, type, what, files, total, pin,
    //    state: asking | receiving | done | failed, paths, message, hidden }]
    property var requests: []
    readonly property var request: requests.find(r => !r.hidden) ?? null
    function addRequest(r) {
        requests = requests.filter(x => x.id !== r.id).concat([Object.assign({ state: "asking", hidden: false, paths: [], message: "" }, r)]);
    }
    function findRequest(id) {
        return requests.find(r => r.id === id) ?? null;
    }
    function updateRequest(id, patch) {
        requests = requests.map(r => r.id === id ? Object.assign({}, r, patch) : r);
    }
    function dropRequest(id) {
        requests = requests.filter(r => r.id !== id);
    }
    // A protocol's helper stopped: its open requests failed with it (the
    // card stayed on "receiving" for good)
    function helperEnded(via) {
        if (requests.some(r => r.via === via && (r.state === "asking" || r.state === "receiving")))
            requests = requests.map(r => r.via === via && (r.state === "asking" || r.state === "receiving") ? Object.assign({}, r, { state: "failed", message: "The transfer stopped before everything arrived." }) : r);
    }
    function answer(id, accept) {
        const r = findRequest(id);
        if (!r)
            return;
        if (r.via === "demo") {
            accept ? updateRequest(id, { state: "receiving" }) : dropRequest(id);
            return;
        }
        const svc = r.via === "quickshare" ? QuickShare : LocalSend;
        svc.tell({ cmd: "answer", session: r.session, accept: accept });
        if (accept) {
            updateRequest(id, { state: "receiving" });
        } else {
            svc.finish(r.session);
            dropRequest(id);
        }
    }
    // The card shows this transfer's progress (the service's own item)
    function progressOf(r) {
        if (r?.via === "demo")
            return r.progress ?? null;
        const svc = r?.via === "quickshare" ? QuickShare : LocalSend;
        return r ? (svc.active[r.session] ?? null) : null;
    }
    // A request that ended while hidden is said in the island, as before
    function shownRequest(id) {
        const r = findRequest(id);
        return !!r && !r.hidden;
    }
    function cancelRequest(id) {
        const r = findRequest(id);
        if (!r)
            return;
        if (r.via === "demo") {
            dropRequest(id);
            return;
        }
        (r.via === "quickshare" ? QuickShare : LocalSend).cancel(r.session);
        dropRequest(id);
    }

    // ── How sends ended, for the sheet ("Sent" under a device, a while) ──
    property var outcomes: ({}) // nameKey -> { state: sent | declined | failed, at }
    function outcome(name, state) {
        const o = Object.assign({}, outcomes);
        o[Devices.nameKey(name)] = { state: state, at: Date.now() };
        outcomes = o;
    }
    function outcomeFor(device) {
        const o = outcomes[Devices.nameKey(device?.name)];
        return o && Date.now() - o.at < 60000 ? o.state : "";
    }

    // The dev sandbox only: the incoming card without a phone
    //   qs ipc call sharingdemo ask | progress <0-100> | done | fail
    IpcHandler {
        target: "sharingdemo"
        enabled: Quickshell.env("IRICE_SANDBOX") === "1"
        function ask(): void {
            root.addRequest({ id: "demo:1", via: "demo", session: "1", from: "Galaxy S25 Ultra", type: "phone", what: "2 photos",
                files: [{ name: "IMG_2041.jpg", mime: "image/jpeg" }, { name: "IMG_2042.jpg", mime: "image/jpeg" }], total: 6200000, pin: "4821" });
        }
        function progress(pct: int): void {
            root.updateRequest("demo:1", { state: "receiving", progress: { done: 6200000 * pct / 100, total: 6200000, waiting: false, transport: "Wi-Fi Direct" } });
        }
        function done(): void {
            root.updateRequest("demo:1", { state: "done", paths: ["/tmp/IMG_2041.jpg", "/tmp/IMG_2042.jpg"] });
        }
        function fail(): void {
            root.updateRequest("demo:1", { state: "failed", message: "Galaxy S25 Ultra stopped sending." });
        }
    }
}
