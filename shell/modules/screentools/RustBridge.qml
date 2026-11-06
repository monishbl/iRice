import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.services

// The screen tools and Quick Look drawn by tessera-ui (rust.surfaces:
// screentools, quicklook; native/tessera-ui/src/surfaces/screentools): this
// shell keeps capturing, recording and reading text (ScreenTools, Recorder,
// LiveText, WifiShare) and tells tessera-ui their state; it answers what
// tessera-ui asks (RustUi's screenCall: a region chosen, the toolbar's tools,
// the recording settings, Live Text's reads and actions, sharing).
Scope {
    id: root

    readonly property bool tools: RustUi.drawing("screentools")
    readonly property bool look: RustUi.drawing("quicklook")

    function state() {
        const t = ScreenTools.translation;
        return {
            select: ScreenTools.selecting,
            selectScreen: ScreenTools.selectingScreen?.name ?? "",
            countdown: Recorder.countdown,
            countdownScreen: Recorder.countdown > 0 ? (ScreenTools.focusedScreen()?.name ?? "") : "",
            translation: t ? { x: t.x, y: t.y, w: t.w, h: t.h, screen: t.screen?.name ?? "", source: t.source ?? "", text: t.text ?? "", busy: !!t.busy, error: t.error ?? "" } : null,
            recording: Recorder.recording,
            available: Recorder.available,
            gsr: Recorder.gsr,
            folder: Recorder.folder,
            home: Quickshell.env("HOME") ?? "",
            toolbarRecord: Recorder.toolbarRecord,
            mics: Recorder.mics.map(m => ({ name: m.name, description: m.description })),
            ocrAvailable: LiveText.available,
            ocrInstalling: Updates.running && ScreenTools.ocrInstalling.length > 0,
            updatesRunning: Updates.running,
            ocrInstallError: ScreenTools.ocrInstallError,
            telHandler: LiveText.telHandler,
            shareOpen: Panels.open === "share",
            focused: Hyprland.focusedMonitor?.name ?? ""
        };
    }
    readonly property string snapshot: tools || look ? JSON.stringify(state()) : ""
    onSnapshotChanged: send()
    function send() {
        if (snapshot === "")
            return;
        const s = JSON.parse(snapshot);
        RustUi.tell("screentools", { state: s });
        RustUi.tell("quicklook", { state: s });
    }
    Connections {
        target: RustUi
        function onStarted() { root.send(); }
    }

    // The toolbar opens with Options showing (Spotlight's "Screen Recording
    // Options") or for recording only (Control Center's Record): said before
    // RustUi forwards the opening
    Connections {
        target: Panels
        function onOpenChanged() {
            if (!root.tools || Panels.open !== "screenTools")
                return;
            RustUi.send({ cmd: "options", surface: "screentools", open: Recorder.toolbarOptions, record: Recorder.toolbarRecord });
            Recorder.toolbarOptions = false;
            Recorder.refreshMics();
        }
    }

    // Live Text's answers, for whichever asked (the editor, Quick Look)
    Connections {
        target: LiveText
        function onDone(k, r) {
            const o = { livetext: k, result: { status: r.status, width: r.width, height: r.height, words: r.words } };
            RustUi.tell("screentools", o);
            RustUi.tell("quicklook", o);
        }
    }

    Connections {
        target: RustUi
        function onScreenCall(name, a) {
            switch (name) {
            case "screen.captured": {
                const scr = Quickshell.screens.find(s => s.name === a[1]) ?? ScreenTools.selectingScreen ?? ScreenTools.focusedScreen();
                ScreenTools.captured(String(a[0] ?? ""), scr, Qt.rect(Number(a[2]) || 0, Number(a[3]) || 0, Number(a[4]) || 0, Number(a[5]) || 0));
                break;
            }
            case "screen.cancel":
                ScreenTools.cancelSelect();
                break;
            case "screen.do":
                if (a[0] === "shot")
                    ScreenTools.screenshotScreen();
                else if (a[0] === "select" && ["screenshot", "ocr", "translate", "ask", "circle", "record", "record-window"].includes(a[1]))
                    ScreenTools.select(a[1]);
                else if (a[0] === "record" && ["screen", "window", "region"].includes(a[1]))
                    Recorder.recording ? Recorder.stop() : Recorder.recordArea(a[1]);
                else if (a[0] === "translationClose")
                    ScreenTools.translation = null;
                break;
            case "recorder.set": {
                const k = String(a[0] ?? ""), v = a[1];
                const ok = { area: ["screen", "window", "region"].includes(v), mic: typeof v === "string", systemAudio: typeof v === "boolean", cursor: typeof v === "boolean", countdown: [0, 3, 5].includes(v), fps: [30, 60].includes(v), codec: ["h264", "hevc", "av1"].includes(v), quality: ["medium", "high", "very_high", "ultra"].includes(v), folder: v === "" };
                if (ok[k])
                    Config.options.recorder[k] = v;
                break;
            }
            case "recorder.folder":
                Panels.close();
                if (a[0] === "pick")
                    FilePicker.pick({ folder: true, title: "Save recordings to", start: Recorder.folder }, p => Config.options.recorder.folder = p);
                else if (a[0] === "open")
                    Quickshell.execDetached(["sh", "-c", "mkdir -p \"$1\" && exec ${2:-xdg-open} \"$1\"", "sh", Recorder.folder, Files.manager]);
                break;
            case "livetext.read": {
                const path = String(a[0] ?? ""), rev = String(a[1] ?? "");
                if (!path.startsWith("/"))
                    break;
                LiveText.read(path, rev);
                const k = LiveText.keyOf(path, rev);
                WifiShare.find(path, found => {
                    const o = { wifi: k, found: (found ?? []).map(w => Object.assign({}, w, { joinable: WifiShare.joinable(w) })) };
                    RustUi.tell("screentools", o);
                    RustUi.tell("quicklook", o);
                });
                break;
            }
            case "livetext.act":
                if (a[0] && ["link", "email", "phone", "wifi"].includes(a[0].kind))
                    LiveText.act(a[0]);
                break;
            case "livetext.install":
                ScreenTools.installOcr("");
                break;
            case "panels.share": {
                const paths = (a[0] ?? []).filter(p => typeof p === "string" && p.startsWith("/"));
                if (paths.length > 0)
                    Panels.share(paths, Quickshell.screens.find(s => s.name === a[1]) ?? undefined);
                break;
            }
            }
        }
    }
}
