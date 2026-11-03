pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import "../common/ScreenGeometry.js" as ScreenGeometry
import "OcrLangs.js" as OcrLangs

// Screenshots, OCR, screen translation, recording and circle to search.
// Region picking happens in RegionSelector; this service does the capturing
// and the follow-up.
Singleton {
    id: root

    readonly property string shotsDir: Quickshell.env("HOME") + "/Pictures/Screenshots"
    readonly property string videosDir: Quickshell.env("HOME") + "/Videos"
    readonly property string tmp: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-capture.png"

    // Region selection request: "" when idle, else screenshot | ocr | translate | record | ask | circle
    // (circle: circle to search, drawn freehand or as a box)
    property string selecting: ""
    property var selectingScreen: null

    // Recording lives in Recorder; these forward to it
    readonly property bool recording: Recorder.recording
    readonly property string recordingFile: Recorder.file
    readonly property real recordingStarted: Recorder.started

    // Last screen translation, shown by TranslationCard
    property var translation: null // { x, y, w, h, screen, source, text, busy, error }

    // The iRice screenshot editor (modules/screentools/editor) sets this while
    // it's in use (screenshot.editor isn't "swappy"); screenshots then go to
    // it through `shot` instead of Swappy and the notification.
    property bool hasEditor: false
    signal shot(string path)
    signal editRequested(string path) // open a picture in the screenshot editor (a bug report's screenshot)

    function stamp() {
        return Qt.formatDateTime(new Date(), "yyyy-MM-dd 'at' hh.mm.ss.zzz") + "-" + Math.random().toString(36).slice(2, 10);
    }

    function focusedScreen() {
        return Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
    }

    // Global geometry string for grim/wf-recorder from screen-local rect
    function geometry(screen, r) {
        const mon = Hyprland.monitorFor(screen);
        r = ScreenGeometry.integerClip(r, screen?.width ?? 0, screen?.height ?? 0);
        return Math.round((mon?.x ?? 0) + r.x) + "," + Math.round((mon?.y ?? 0) + r.y) + " " + Math.round(r.width) + "x" + Math.round(r.height);
    }

    // Text Capture's and Live Text's languages: the system's and the keyboard
    // layouts' (whose tesseract-data-* is installed) plus English, e.g.
    // "deu+eng" (OcrLangs.js)
    property string ocrLangs: "eng"
    readonly property string ocrLangArg: ocrLangs.replace(/[^a-z_+]/g, "") || "eng" // (for -l)
    property var ocrMissing: [] // wanted languages without tesseract data
    // Settings' list: [{ code, name, installed }], English first
    property var ocrLanguages: []
    readonly property bool hasTesseract: Tools.has("tesseract")
    onHasTesseractChanged: if (hasTesseract)
        langList.running = true
    function listLanguages() {
        if (hasTesseract)
            langList.running = true;
    }
    Connections {
        target: KeyboardLayout
        function onVariantsChanged() { root.listLanguages(); }
    }
    // Tesseract or a language through Software Update's job (never pacman
    // here): Settings' Install buttons, Live Text's
    property var ocrInstalling: [] // the packages asked for, until the job ends
    property string ocrInstallError: ""
    function installOcr(code) {
        if (Updates.running)
            return;
        ocrInstalling = /^[a-z_]+$/.test(code ?? "") ? ["tesseract-data-" + code] : ["tesseract", "tesseract-data-eng"];
        ocrInstallError = "";
        Needs.install(ocrInstalling);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (root.ocrInstalling.length > 0)
                root.ocrInstallError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            root.ocrInstalling = [];
            root.listLanguages(); // (what's there now)
        }
    }
    Process {
        id: langList
        command: ["tesseract", "--list-langs"]
        stdout: StdioCollector {
            onStreamFinished: {
                const env = { LC_ALL: Quickshell.env("LC_ALL") ?? "", LC_MESSAGES: Quickshell.env("LC_MESSAGES") ?? "", LANG: Quickshell.env("LANG") ?? "", LANGUAGE: Quickshell.env("LANGUAGE") ?? "" };
                const layouts = KeyboardLayout.layouts.map((l, i) => ({ layout: l, variant: KeyboardLayout.variants[i] ?? "" }));
                const p = OcrLangs.pick(env, text, layouts);
                const have = OcrLangs.installedSet(text);
                root.ocrLangs = p.langs.join("+");
                root.ocrMissing = p.missing;
                const w = OcrLangs.wanted(env, layouts);
                root.ocrLanguages = ["eng"].concat(w.filter(c => c !== "eng")).map(c => ({ code: c, name: OcrLangs.nameOf(c), installed: !!have[c] }));
            }
        }
    }
    function ocrCommand(geo) {
        return "grim -g '" + geo + "' - | tesseract stdin stdout -l '" + ocrLangArg + "' 2>/dev/null";
    }

    function select(mode) {
        if ((mode === "record" || mode === "record-window") && recording)
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
        rect = ScreenGeometry.clip(rect, screen?.width ?? 0, screen?.height ?? 0);
        if (rect.width < 1 || rect.height < 1)
            return;
        const geo = geometry(screen, rect);
        if (mode === "screenshot")
            shoot(["-g", geo], true);
        else if (mode === "ocr")
            run("ocr", ocrCommand(geo));
        else if (mode === "translate") {
            translation = { x: rect.x, y: rect.y, w: rect.width, h: rect.height, screen: screen, source: "", text: "", busy: true, error: "" };
            run("translate", ocrCommand(geo));
        } else if (mode === "record" || mode === "record-window")
            startRecording(geo);
        else if (mode === "ask" || mode === "circle")
            askAbout(["-g", geo], mode);
    }

    // Ask AI about the screen: capture as JPEG, attach to the next chat
    // message (`kind` circle: and ask what it is). The pictures stay
    // owner-only in the runtime folder; ones over a day old go.
    function askAbout(args, kind) {
        const dir = (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/irice-ai";
        const file = dir + "/shot-" + Date.now() + ".jpg";
        const q = a => "'" + a.replace(/'/g, "'\\''") + "'";
        run(kind ?? "ask", "umask 077; mkdir -p " + q(dir) + " && find " + q(dir) + " -name 'shot-*.jpg' -mmin +1440 -delete; grim -t jpeg -q 85 " + args.map(q).join(" ") + " " + q(file) + " && echo " + q(file) + " && base64 -w0 " + q(file));
    }

    // Circle to search's "Search the web": only now does the picture leave
    // the computer, to Google Lens (lens-upload.py); its results open in the
    // browser. lensBusy: the picture on its way; lensDone: how the last one
    // went, { path, ok, text }, for the chat row that asked.
    property string lensBusy: ""
    property var lensDone: null
    function searchWeb(path) {
        if (lensBusy !== "" || path === "")
            return;
        lensBusy = path;
        lensDone = null;
        const q = a => "'" + a.replace(/'/g, "'\\''") + "'";
        run("lens", "exec python3 -I " + q(Quickshell.shellDir + "/assets/scripts/lens-upload.py") + " " + q(path));
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
        Recorder.start({ region: geo });
    }

    function recordScreen() {
        Panels.close();
        Recorder.start({ output: Hyprland.focusedMonitor?.name ?? "" });
    }

    function stopRecording() {
        Recorder.stop();
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
            WifiShare.scan(file); // (a Wi-Fi code in it: the island offers to join)
            LiveText.indexSoon(); // (its words, for Search: in the background, later)
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
            Launch.run(["sh", "-c", "a=$(timeout 600 notify-send -a Screenshot -i \"$1\" -A open=Open -A folder='Show in Folder' 'Screenshot saved' 'Copied to the clipboard'); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"", "sh", file, Files.manager]);
        } else if (kind === "ocr") {
            const text = out.trim();
            if (text === "") {
                Quickshell.execDetached(["notify-send", "-a", "Text Capture", "No text found", "Try selecting a larger area with clearer text." + missingHint()]);
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
            // (both quoted; "--": text read off the screen that starts with
            // "-" is never an option, like -i=FILE, which would send a file)
            const q = a => "'" + String(a).replace(/'/g, "'\\''") + "'";
            run("translated", "trans -brief -no-ansi -t " + q(t) + " -- " + q(src));
        } else if (kind === "ask" || kind === "circle") {
            const nl = out.indexOf("\n");
            if (nl < 0)
                return;
            const picture = { path: out.slice(0, nl), data: out.slice(nl + 1).trim(), mediaType: "image/jpeg" };
            Panels.sidebarTab = 0;
            // (circle: in the sidebar of the screen it was drawn on)
            Panels.show("sidebar", kind === "circle" ? selectingScreen : focusedScreen());
            if (kind === "circle")
                Ai.searchPicture(picture);
            else
                Ai.attach(picture);
        } else if (kind === "lens") {
            const path = lensBusy;
            lensBusy = "";
            let r = null;
            try {
                r = JSON.parse(out);
            } catch (e) {}
            if (r?.ok && Launch.openLink(r.url)) {
                lensDone = { path: path, ok: true, text: "Opened in your browser" };
            } else if (r?.fallback) {
                // (Lens changed how it takes pictures: paste it there instead)
                Quickshell.execDetached(["sh", "-c", "wl-copy --type image/jpeg < \"$1\"", "sh", path]);
                Launch.open("https://lens.google.com/");
                lensDone = { path: path, ok: true, text: "Paste the picture into Lens with Ctrl+V: it's on the clipboard." };
            } else {
                lensDone = { path: path, ok: false, text: r?.error || "Couldn't search the picture. Try again." };
            }
        } else if (kind === "translated") {
            translation = Object.assign({}, translation, { text: out.trim(), busy: false });
        }
    }

    // "For text in your language, install tesseract-data-deu."
    function missingHint() {
        const m = ocrMissing.filter(c => /^[a-z_]+$/.test(c));
        return m.length === 0 ? "" : " For text in your language, install " + m.map(c => "tesseract-data-" + c).join(" and ") + ".";
    }

    function failed(kind) {
        if (kind === "translate")
            translation = Object.assign({}, translation, { busy: false, error: "Could not read text from the screen." });
        if (kind === "lens" && lensBusy !== "") { // (unless its output already said)
            lensDone = { path: lensBusy, ok: false, text: "Couldn't start the search. Make sure python3 is installed, then try again." };
            lensBusy = "";
        }
        if (kind === "shot" || kind === "shotMarkup" || kind === "ask" || kind === "circle")
            Quickshell.execDetached(["notify-send", "-a", "Screenshot", "Couldn't take the screenshot", "Make sure grim is installed."]);
        else if (kind === "ocr" || kind === "translate")
            Quickshell.execDetached(["notify-send", "-a", "Text Capture", "Couldn't read the screen", "Install tesseract and tesseract-data-eng, then try again." + missingHint()]);
        else if (kind === "translated")
            translation = Object.assign({}, translation, { busy: false, error: "Couldn't translate. Check your connection and that translate-shell is installed." });
    }

    IpcHandler {
        target: "screen"
        function screenshot(): void { root.screenshotScreen(); }
        function region(mode: string): void { root.select(mode || "screenshot"); }
        function ask(): void { root.askScreen(); }
        // Circle to search: circle (or box) anything on the focused screen
        function circle(): void { root.select("circle"); }
        function stop(): void { root.stopRecording(); }
        // qs ipc call screen ocrLanguages  (e.g. "deu+eng; missing: fra")
        function ocrLanguages(): string { return root.ocrLangs + "; missing: " + (root.ocrMissing.join(",") || "none"); }
    }
}
