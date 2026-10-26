pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common

// Screenshots, OCR, screen translation and recording. Region picking happens
// in RegionSelector; this service does the capturing and the follow-up.
Singleton {
    id: root

    readonly property string shotsDir: Quickshell.env("HOME") + "/Pictures/Screenshots"
    readonly property string videosDir: Quickshell.env("HOME") + "/Videos"
    readonly property string tmp: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-capture.png"

    // Region selection request: "" when idle, else screenshot | ocr | translate | record | ask
    property string selecting: ""
    property var selectingScreen: null

    property bool recording: false
    property string recordingFile: ""
    property real recordingStarted: 0

    // Last screen translation, shown by TranslationCard
    property var translation: null // { x, y, w, h, screen, source, text, busy, error }

    // The iRice screenshot editor (modules/screentools/editor) sets this while
    // it's in use (screenshot.editor isn't "swappy"); screenshots then go to
    // it through `shot` instead of Swappy and the notification.
    property bool hasEditor: false
    signal shot(string path)

    function stamp() {
        return Qt.formatDateTime(new Date(), "yyyy-MM-dd 'at' hh.mm.ss");
    }

    function focusedScreen() {
        return Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
    }

    // Global geometry string for grim/wf-recorder from screen-local rect
    function geometry(screen, r) {
        const mon = Hyprland.monitorFor(screen);
        return Math.round((mon?.x ?? 0) + r.x) + "," + Math.round((mon?.y ?? 0) + r.y) + " " + Math.round(r.width) + "x" + Math.round(r.height);
    }

    function select(mode) {
        if (mode === "record" && recording)
            return stopRecording();
        Panels.close();
        selectingScreen = focusedScreen();
        selecting = mode;
    }

    function cancelSelect() {
        selecting = "";
    }

    // Called by RegionSelector once the overlay is hidden.
    function captured(mode, screen, rect) {
        selecting = "";
        const geo = geometry(screen, rect);
        if (mode === "screenshot")
            shoot(["-g", geo], true);
        else if (mode === "ocr")
            run("ocr", "grim -g '" + geo + "' - | tesseract stdin stdout -l eng 2>/dev/null");
        else if (mode === "translate") {
            translation = { x: rect.x, y: rect.y, w: rect.width, h: rect.height, screen: screen, source: "", text: "", busy: true, error: "" };
            run("translate", "grim -g '" + geo + "' - | tesseract stdin stdout -l eng 2>/dev/null");
        } else if (mode === "record")
            startRecording(geo);
        else if (mode === "ask")
            askAbout(["-g", geo]);
    }

    // Ask AI about the screen: capture as JPEG, attach to the next chat message
    function askAbout(args) {
        const file = (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-ask-" + Date.now() + ".jpg";
        const q = a => "'" + a.replace(/'/g, "'\\''") + "'";
        run("ask", "grim -t jpeg -q 85 " + args.map(q).join(" ") + " " + q(file) + " && echo " + q(file) + " && base64 -w0 " + q(file));
    }
    function askScreen() {
        Panels.close();
        const mon = Hyprland.focusedMonitor;
        Qt.callLater(() => askAbout(mon ? ["-o", mon.name] : []));
    }

    function screenshotScreen() {
        Panels.close();
        const mon = Hyprland.focusedMonitor;
        Qt.callLater(() => shoot(mon ? ["-o", mon.name] : []));
    }

    // `area`: an area screenshot, which opens for markup (Swappy) when that's on
    function shoot(args, area) {
        const file = shotsDir + "/Screenshot " + stamp() + ".png";
        const q = a => "'" + a.replace(/'/g, "'\\''") + "'";
        run(area === true && !hasEditor && Config.options.screenshot.markup && Tools.has("swappy") ? "shotMarkup" : "shot", "mkdir -p " + q(shotsDir) + " && grim " + args.map(q).join(" ") + " " + q(file) + " && wl-copy < " + q(file) + " && echo " + q(file));
    }

    function startRecording(geo) {
        const file = videosDir + "/Screen Recording " + stamp() + ".mp4";
        recordingFile = file;
        recorder.command = ["sh", "-c", "mkdir -p \"$1\" && exec wf-recorder -g \"$2\" -f \"$3\"", "sh", videosDir, geo, file];
        recorder.running = true;
        recording = true;
        recordingStarted = Date.now();
    }

    function recordScreen() {
        Panels.close();
        const s = focusedScreen();
        startRecording(geometry(s, Qt.rect(0, 0, s.width, s.height)));
    }

    function stopRecording() {
        if (recorder.running)
            recorder.signal(2); // SIGINT: wf-recorder finalises the file
    }

    Process {
        id: recorder
        onExited: code => {
            root.recording = false;
            Launch.run(["sh", "-c", "a=$(notify-send -a 'Screen Recording' -i video-x-generic -A open=Open -A folder='Show in Folder' 'Recording saved' \"$(basename \"$1\")\"); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"", "sh", root.recordingFile, Files.manager]);
        }
    }

    // One shell job at a time per kind; output handled by kind.
    function run(kind, script) {
        const p = jobComponent.createObject(root, { kind: kind, script: script });
        p.running = true;
    }

    Component {
        id: jobComponent
        Process {
            id: job
            property string kind
            property string script
            command: ["sh", "-c", script]
            stdout: StdioCollector {
                onStreamFinished: root.finish(job.kind, text)
            }
            onExited: code => {
                if (code !== 0)
                    root.failed(job.kind);
                job.destroy();
            }
        }
    }

    function finish(kind, out) {
        if ((kind === "shot" || kind === "shotMarkup") && out.trim() !== "") {
            const file = out.trim();
            Sounds.play("screenshot");
            if (hasEditor) {
                shot(file); // floating thumbnail; click it to mark up
                return;
            }
            // Markup: Swappy edits the saved file (its Save writes it back;
            // Copy puts the edited image on the clipboard)
            if (kind === "shotMarkup") {
                Launch.run(["swappy", "-f", file, "-o", file]);
                return;
            }
            Launch.run(["sh", "-c", "a=$(notify-send -a Screenshot -i \"$1\" -A open=Open -A folder='Show in Folder' 'Screenshot saved' 'Copied to the clipboard'); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"", "sh", file, Files.manager]);
        } else if (kind === "ocr") {
            const text = out.trim();
            if (text === "") {
                Quickshell.execDetached(["notify-send", "-a", "Text Capture", "No text found", "Try selecting a larger area with clearer text."]);
                return;
            }
            Quickshell.execDetached(["wl-copy", text]);
            const words = text.split(/\s+/).length;
            Quickshell.execDetached(["notify-send", "-a", "Text Capture", "Copied " + words + (words === 1 ? " word" : " words"), text.slice(0, 200)]);
        } else if (kind === "translate") {
            const src = out.trim().replace(/\s*\n\s*/g, " ");
            if (src === "") {
                translation = Object.assign({}, translation, { busy: false, error: "No text found in that area." });
                return;
            }
            translation = Object.assign({}, translation, { source: src });
            const t = Config.options.sidebar.translateTarget;
            run("translated", "trans -brief -no-ansi -t '" + t + "' " + "'" + src.replace(/'/g, "'\\''") + "'");
        } else if (kind === "ask") {
            const nl = out.indexOf("\n");
            if (nl < 0)
                return;
            Ai.attach({ path: out.slice(0, nl), data: out.slice(nl + 1).trim(), mediaType: "image/jpeg" });
            Panels.sidebarTab = 0;
            Panels.show("sidebar", focusedScreen());
        } else if (kind === "translated") {
            translation = Object.assign({}, translation, { text: out.trim(), busy: false });
        }
    }

    function failed(kind) {
        if (kind === "shot" || kind === "shotMarkup" || kind === "ask")
            Quickshell.execDetached(["notify-send", "-a", "Screenshot", "Couldn't take the screenshot", "Make sure grim is installed."]);
        else if (kind === "ocr" || kind === "translate")
            Quickshell.execDetached(["notify-send", "-a", "Text Capture", "Couldn't read the screen", "Make sure tesseract and its English data are installed."]);
        else if (kind === "translated")
            translation = Object.assign({}, translation, { busy: false, error: "Couldn't translate. Check your connection and that translate-shell is installed." });
    }

    IpcHandler {
        target: "screen"
        function screenshot(): void { root.screenshotScreen(); }
        function region(mode: string): void { root.select(mode || "screenshot"); }
        function ask(): void { root.askScreen(); }
        function stop(): void { root.stopRecording(); }
    }
}
