import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.services

// While tessera-ui draws sharing (rust.surfaces: share), what only this
// shell knows reaches it here, one line on its stdin (RustUi.tell("share",
// { feed })): what the sheet shares (Panels.sharePaths, shareNote), the
// nearby devices with each send's progress and how it ended, whether it can
// look, the incoming request (Sharing.request) and its transfer, the QR
// link card (LinkShare), the screen you're using. And what it asks of this
// shell (RustUi's "share" call) is done here: send to a device, the other
// ways, answering, hiding, cancelling and opening a request, closing the QR
// card. The transfers themselves stay this shell's (Sharing, LocalSend,
// QuickShare, Cast, LinkShare).
// (native/tessera-ui/src/surfaces/share/mod.rs reads what this sends)
Item {
    id: root

    readonly property bool here: RustUi.drawing("share")

    // ── The sheet ───────────────────────────────────────────────────────
    readonly property var paths: Panels.sharePaths ?? []
    readonly property string note: Panels.shareNote ?? ""
    readonly property bool castable: paths.length === 1 && note === "" && /\.(mp4|m4v|webm|mkv|mov|mp3|m4a|aac|ogg|oga|opus|flac|wav|jpe?g|png|gif|webp)$/i.test(paths[0])
    // Nearby devices, and Chromecasts for one video, song or photo (as the QML sheet)
    readonly property var devices: Sharing.devices.concat(castable ? Cast.devices.map(d => Object.assign({}, d, { cast: true, key: "cast:" + (d.id ?? ((d.host ?? "") + ":" + (d.port ?? ""))) })) : [])
    // Finding devices while the sheet is up (Sharing counts who's looking)
    readonly property bool sheetOpen: here && Panels.open === "share"
    property bool lookingHere: false
    onSheetOpenChanged: look(sheetOpen)
    function look(on) {
        if (on !== lookingHere) {
            lookingHere = on;
            Sharing.look(on);
        }
    }
    Component.onDestruction: look(false)

    function sheet() {
        return {
            paths: root.paths,
            note: root.note,
            icon: String(IconLookup.path(root.note !== "" ? "text-x-generic" : root.paths.length === 1 ? Search.iconForFile(root.paths[0] ?? "") : "folder-documents", "text-x-generic") ?? ""),
            devices: root.devices.map(d => {
                const s = Sharing.progressFor(d);
                return {
                    key: String(d.key ?? ""),
                    name: String(d.name ?? ""),
                    type: String(d.type ?? ""),
                    cast: !!d.cast,
                    castIcon: d.cast ? Cast.iconFor(d) : "",
                    prefer: String(d.prefer ?? ""),
                    sending: s ? { done: Number(s.done ?? 0), total: Number(s.total ?? 0), waiting: !!s.waiting } : null,
                    ended: s ? "" : Sharing.outcomeFor(d)
                };
            }),
            available: LocalSend.available,
            portsError: SharingPorts.error,
            errors: [LocalSend.error, QuickShare.error].filter(e => !!e).join(" ")
        };
    }

    // ── Someone nearby wants to share something ─────────────────────────
    readonly property var req: Sharing.request
    readonly property var item: Sharing.progressOf(req)
    function request() {
        const r = root.req;
        if (!r)
            return null;
        const it = root.item;
        return {
            id: String(r.id),
            from: String(r.from ?? ""),
            type: String(r.type ?? ""),
            what: String(r.what ?? "something"),
            files: (r.files ?? []).map(f => ({ name: String(f.name ?? ""), mime: String(f.mime ?? "") })),
            total: Number(r.total ?? 0),
            totalText: Number(r.total ?? 0) > 0 ? Transfers.size(r.total) : "",
            pin: String(r.pin ?? ""),
            state: String(r.state ?? ""),
            message: String(r.message ?? ""),
            paths: r.paths ?? [],
            folder: Config.options.sharing.folder ? Config.options.sharing.folder.split("/").pop() : "Downloads",
            item: it ? { done: Number(it.done ?? 0), total: Number(it.total ?? 0), waiting: !!it.waiting, transport: String(it.transport ?? ""), doneText: Transfers.size(it.done ?? 0), totalText: Transfers.size(it.total ?? 0) } : null
        };
    }

    // ── The QR code card ────────────────────────────────────────────────
    function link() {
        if (LinkShare.mode === "")
            return null;
        return {
            mode: LinkShare.mode,
            stage: LinkShare.stage,
            names: LinkShare.names,
            qr: LinkShare.qr,
            error: LinkShare.error,
            busy: LinkShare.busy,
            done: LinkShare.done,
            total: LinkShare.total,
            count: LinkShare.count,
            installing: Needs.installing,
            updates: Updates.running
        };
    }

    readonly property var feed: ({
        sheet: root.sheet(),
        request: root.request(),
        link: root.link(),
        screen: Hyprland.focusedMonitor?.name ?? "",
        motion: CoolDown.motionScale
    })
    // (sent at once: the paths must be there before Panels' open reaches it)
    function send() {
        if (root.here)
            RustUi.tell("share", { feed: root.feed });
    }
    onFeedChanged: send()
    Connections {
        target: RustUi
        // (sent again when tessera-ui starts, or starts drawing sharing)
        function onStarted() {
            root.send();
        }
        function onDrawnChanged() {
            root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "share")
                return;
            root.call(what, a);
        }
    }

    function call(what, a) {
        switch (what) {
        case "send": {
            const d = root.devices.find(x => x.key === String(a[0] ?? ""));
            if (!d)
                break;
            if (d.cast) {
                Cast.castFile(d, root.paths[0]);
                Panels.close();
            } else {
                Sharing.send(d, root.paths);
            }
            break;
        }
        case "copy":
            if (root.note !== "")
                Clipboard.copyText(root.note);
            else if (root.paths.length > 0)
                Quickshell.execDetached(["sh", "-c", "printf '%s\\n' \"$@\" | sed 's|^|file://|' | wl-copy -t text/uri-list", "sh"].concat(root.paths));
            break;
        case "reveal":
            Files.reveal(root.paths[0] ?? "");
            break;
        case "qr":
            if (root.paths.length > 0)
                LinkShare.send(root.paths);
            break;
        case "answer":
            Sharing.answer(String(a[0] ?? ""), a[1] === true);
            break;
        case "cancel":
            Sharing.cancelRequest(String(a[0] ?? ""));
            break;
        case "hide":
            Sharing.updateRequest(String(a[0] ?? ""), { hidden: true });
            break;
        case "drop":
            Sharing.dropRequest(String(a[0] ?? ""));
            break;
        case "open": {
            const r = Sharing.findRequest(String(a[0] ?? ""));
            if (r) {
                Files.open(r.paths[0]);
                Sharing.dropRequest(r.id);
            }
            break;
        }
        case "show": {
            const r = Sharing.findRequest(String(a[0] ?? ""));
            if (r) {
                Files.reveal(r.paths[0] ?? "");
                Sharing.dropRequest(r.id);
            }
            break;
        }
        case "linkClose":
            LinkShare.close();
            break;
        case "installQr":
            if (!Updates.running)
                Needs.install(["qrencode"]);
            break;
        }
    }
}
