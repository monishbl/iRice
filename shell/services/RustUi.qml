pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import qs.common

// tessera-ui (native/tessera-ui, built by tools/build-native.sh --ui): Tessera's
// interface in Rust, drawing the surfaces Settings lists in `rust.surfaces`
// in place of this shell's (docs/rust-ui-plan.md). This shell hides its own
// copy of a surface only while tessera-ui says it draws it (drawing(name));
// when tessera-ui isn't built, ends, or never says so, this shell draws it.
// It ends when this shell does (its stdin closes).
//
// Panels it draws (the emoji picker, the clipboard…) still open through
// Panels, from their shortcut, IPC, Spotlight or Control Center: their
// PanelLoader steps aside (`elsewhere: RustUi.drawing(name)`) and this
// forwards each open and close to tessera-ui, a JSON line on its stdin
// ({"cmd":"open","surface":"emoji","screen":"DP-1"}, {"cmd":"close",…}). It
// answers on stdout when a panel closed itself ("closed": Panels follows)
// and asks for what only this shell does ("call": the short list below).
// The protocol: native/tessera-ui/src/shell.rs.
Singleton {
    id: root

    readonly property list<string> wanted: Config.ready ? Config.options.rust.surfaces : []
    readonly property string binary: Quickshell.env("TESSERA_UI") || Paths.data + "/bin/tessera-ui"
    property var drawn: [] // what it said it draws
    function drawing(name) {
        return proc.running && drawn.includes(name);
    }
    // A line for it (its stdin): what this shell still owns that its surfaces
    // show (modules/menubar/RustBridge.qml), {"src":"shell","kind":…};
    // send() is below, with the panels'
    // It has (re)started: whoever feeds it says everything again
    signal started
    // Panels whose name isn't the surface's (surface names are lowercase)
    readonly property var panelNames: ({
            controlcenter: "controlCenter",
            osk: "keyboard",
            notes: "quickNote", // (the quick note)
            network: "wifiMenu" // (the menu bar's Wi-Fi menu, with the speed test)
        })
    function panelOf(surface) {
        return panelNames[surface] ?? surface;
    }
    function surfaceOf(panel) {
        return Object.keys(panelNames).find(k => panelNames[k] === panel) ?? panel;
    }
    // Control Center's calls, for its RustBridge (cc.act, cc.block…)
    signal ccCall(string name, var args)
    // What its surfaces say ({"src":"ui","kind":"osd","shown":…}): the OSD's
    // capsule shows (the banners move down under it)
    property bool osdShown: false
    // What the desktop drawn in Rust asks of this shell (its "desktop"
    // call): done by modules/desktop/DesktopFeed.qml
    signal desktopCall(string what, var args)
    // What the smaller surfaces drawn in Rust ask of their QML twins (the
    // overlay's widgets, the welcome screen's choices, a hot corner's
    // action): answered by Overlay.qml, Welcome.qml and HotCorners.qml
    signal surfaceCall(string name, var args)
    // What the share sheet and cards, the App Store, the shelf, the Wi-Fi
    // menu, live captions and the quick note drawn there ask of this shell
    // (their "share", "appstore", "shelf", "network", "captions" and "notes"
    // calls): done by each one's feed (modules/<name>/*Feed.qml)
    signal feedCall(string surface, string what, var args)
    // Its surfaces' other lines ({"src":"ui","kind":"notifications",…}: the
    // notification list, services/Notifications.qml) and whether it runs
    signal message(var e)
    readonly property bool running: proc.running
    // Tells a surface it draws something only this shell knows (a line on
    // its stdin, {"to":"osd",…}): e.g. Audio.hush, the OSD's preview
    function tell(surface, what) {
        if (drawing(surface))
            send(Object.assign({ to: surface }, what));
    }

    // (armed a moment after the list changes, once `command` has the new
    // list: started in the same instant, it could run with the old one)
    property bool armed: false
    KeptProcess {
        id: proc
        wanted: root.armed
        command: [root.binary, "--surfaces", root.wanted.join(",")]
        stdinEnabled: true // (it ends when this closes: the shell quit)
        onRunningChanged: if (!running) {
            root.drawn = [];
            root.osdShown = false;
        }
        stdout: SplitParser {
            onRead: line => root.handle(line)
        }
    }
    // A new list: start it again with that one (what's drawn waits for it to
    // say). An empty list just stops it (a restart then would leave it
    // waiting out a back-off the next time it's wanted).
    onWantedChanged: Qt.callLater(() => {
        const on = root.wanted.length > 0;
        if (on && root.armed && proc.running) {
            root.drawn = [];
            proc.restart();
        }
        root.armed = on;
    })

    function handle(line) {
        let e;
        try {
            e = JSON.parse(line);
        } catch (x) {
            return;
        }
        if (e.src !== "ui")
            return;
        if (e.kind === "ready") {
            drawn = e.surfaces ?? [];
            forwarded = "";
            Qt.callLater(follow); // (a panel already open moves over)
            started();
        } else if (e.kind === "unknown") {
            console.info("tessera-ui: no " + e.surface + " in Rust yet: this shell draws it");
        } else if (e.kind === "closed") {
            // it closed itself (Esc, a click outside, a pick)
            if (forwarded === e.surface)
                forwarded = "";
            if (Panels.open === panelOf(e.surface))
                Panels.close();
        } else if (e.kind === "call") {
            call(e.call, e.args ?? []);
        } else if (e.kind === "osd") {
            osdShown = e.shown === true;
        } else {
            message(e);
        }
    }

    // ── Panels drawn in Rust: their opening and closing ─────────────────
    property string forwarded: "" // the panel tessera-ui was told to open
    property string forwardedScreen: ""
    function send(o) {
        if (proc.running)
            proc.write(JSON.stringify(o) + "\n");
    }
    function follow() {
        const want = Panels.open !== "" && drawing(surfaceOf(Panels.open)) ? surfaceOf(Panels.open) : "";
        const scr = want !== "" ? (Panels.screen?.name ?? "") : "";
        if (forwarded !== "" && forwarded !== want)
            send({ cmd: "close", surface: forwarded });
        if (want !== "" && (want !== forwarded || scr !== forwardedScreen))
            send({ cmd: "open", surface: want, screen: scr });
        forwarded = want;
        forwardedScreen = scr;
    }
    Connections {
        target: Panels
        // (later: Panels.show sets the panel, then its screen)
        function onOpenChanged() {
            Qt.callLater(root.follow);
        }
        function onScreenChanged() {
            Qt.callLater(root.follow);
        }
    }
    onDrawnChanged: if (drawn.length === 0) forwarded = ""

    // The sidebar drawn in Rust: the tab Panels shows (sent before it opens,
    // and again once it's drawing), Screen tools' and Spotlight's pictures and
    // exchanges go to its chat (ScreenTools.qml, Ai.qml), and once it's back
    // here the chat it had goes on in this shell's sidebar
    readonly property bool sidebarHere: drawing("sidebar")
    function sidebarTab() {
        if (sidebarHere)
            send({ cmd: "tab", surface: "sidebar", tab: Panels.sidebarTab });
    }
    onSidebarHereChanged: sidebarHere ? sidebarTab() : Ai.reloadHistory(true)
    Connections {
        target: Panels
        function onSidebarTabChanged() {
            root.sidebarTab();
        }
    }

    // What a panel drawn in Rust asks of this shell (its files, its windows)
    function call(name, a) {
        switch (name) {
        case "clipboard.refresh":
            Clipboard.refresh(); // (it stamps new entries with when they came)
            break;
        case "clipboard.pin":
            if (a[0]?.id !== undefined)
                Clipboard.pin({ id: String(a[0].id), text: String(a[0].text ?? ""), image: String(a[0].image ?? "") });
            break;
        case "clipboard.unpin":
            if (a[0])
                Clipboard.unpin(a[0]);
            break;
        case "panels.openSettings":
            Panels.openSettings(a[0], a[1]);
            break;
        case "launch.open":
            if (/^https:\/\//.test(a[0] ?? ""))
                Launch.open(a[0]);
            break;
        case "wallpaper.set":
            if (typeof a[0] === "string" && a[0].startsWith("/"))
                Wallpaper.set(a[0]);
            break;
        case "wallpaper.openFolder":
            Files.openFolder(Wallpaper.directory);
            break;
        case "sidebar.tab": // (its tab row: Panels keeps it for next time)
            Panels.sidebarTab = Number(a[0] ?? 0);
            break;
        case "ai.reloadHistory": // (it wrote the chat history files)
            Ai.reloadHistory(false);
            break;
        case "screen.ask": // (its screenshot menu: the picture comes back as "attach")
            if (a[0] === "screen")
                ScreenTools.askScreen();
            else if (a[0] === "ask" || a[0] === "circle")
                ScreenTools.select(a[0]);
            break;
        case "launch.openLink": // (web and mail links only: Launch says)
            Launch.openLink(a[0] ?? "");
            break;

        case "today.close":
            Today.close();
            break;
        case "today.pin":
            Today.pin();
            break;
        case "today.set": // (Today's own options, TodayOptions.qml's)
            if (a[0] === "mode" && ["ask", "widget", "both", "off"].includes(a[1]))
                Config.options.today.mode = a[1];
            else if (a[0] === "entrance" && ["bloom", "shrink", "drop"].includes(a[1]))
                Config.options.today.entrance = a[1];
            else if (a[0] === "name" && typeof a[1] === "string")
                Config.options.today.name = a[1].trim().slice(0, 100);
            else if (a[0] === "chime" && typeof a[1] === "boolean")
                Config.options.sounds.today = a[1];
            break;
        case "todos.toggle": {
            const t = Todos.find(String(a[0] ?? ""));
            if (t)
                Todos.update(t, { done: !t.done });
            break;
        }
        case "panels.show": // (the panels a Rust surface opens: Control Center's power menu, Search's "Just ask")
            if (["session", "sidebar"].includes(a[0]))
                Panels.show(a[0], Quickshell.screens.find(s => s.name === a[1]) ?? undefined);
            break;
        case "audio.hush":
            Audio.hush();
            break;
        case "brightness.hush":
            Brightness.hush();
            break;
        case "cc.act":
        case "cc.page":
        case "cc.editing":
        case "cc.layout":
        case "cc.media":
        case "cc.block":
        case "cc.pick":
            ccCall(name, a);
            break;
        case "desktop":
            root.desktopCall(String(a[0] ?? ""), a.slice(1));
            break;
        case "search.query": // (search: spotlight)
            Search.rustQuery(a[0], a[1]);
            break;
        case "search.run":
            Search.rustRun(String(a[0] ?? ""));
            break;
        case "search.action":
            Search.rustAction(String(a[0] ?? ""));
            break;
        case "ai.quickAsk":
            Ai.quickAsk(String(a[0] ?? ""));
            break;
        case "ai.stopQuick":
            Ai.stopQuick();
            break;
        case "ai.continueQuick":
            Ai.continueQuick();
            break;
        case "panels.quickLook":
            Panels.quickLook(String(a[0] ?? ""));
            break;
        case "files.open":
            if (String(a[0] ?? "").startsWith("/"))
                Files.open(String(a[0]));
            break;
        case "search.command":
            Search.rustCommand(String(a[0] ?? ""));
            break;
        case "overlay.widget": // (overlay: Overlay.qml)
        case "overlay.crosshair":
        case "overlay.record":
        case "overlay.pasteImage":
        case "overlay.fps":
        case "welcome.mode": // (welcome: Welcome.qml)
        case "welcome.wall":
        case "welcome.pick":
        case "welcome.later":
        case "welcome.finish":
        case "hotcorners.run": // (hotcorners: HotCorners.qml)
            root.surfaceCall(name, a);
            break;
        case "tiling.snap": { // (decorations' tile grid: Tiling keeps the size for Restore)
            const t = Hyprland.toplevels.values.find(w => "0x" + String(w.address).replace(/^0x/, "") === String(a[1] ?? ""));
            if (t && Tiling.layouts[a[0]])
                Tiling.snap(String(a[0]), t);
            break;
        }
        case "share":
        case "appstore":
        case "shelf":
        case "network":
        case "captions":
        case "notes":
            root.feedCall(name, String(a[0] ?? ""), a.slice(1));
            break;
        default:
            console.warn("tessera-ui: no call " + name);
        }
    }
}
