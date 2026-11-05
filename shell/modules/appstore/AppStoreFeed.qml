import QtQuick
import Quickshell
import qs.common
import qs.services

// While tessera-ui draws the App Store (rust.surfaces: appstore), the store
// stays this shell's (services/AppStore.qml: appstore.py, Software Update,
// flatpak) and the window is drawn there. This tells it, a line on its
// stdin (RustUi.tell):
//   state   { open, catalogue, updates, jobs, app }  whether it's open
//           (Panels.appStoreOpen), the catalogue notice, Software Update
//           running, AppStore.jobs, the app Spotlight asked for
//   answer  { token, apps, sections, error }  a place's apps (each with its
//           logo's file and where it's from)
//   changed { key, installed }  installed or removed
//   show    app  Spotlight's "Get" while it's open
// and does what it asks (RustUi's "appstore" call): load a place, close,
// install, open, remove, the catalogue, the website, copy the name.
// (native/tessera-ui/src/surfaces/appstore reads what this sends)
Item {
    id: root

    readonly property bool here: RustUi.drawing("appstore")
    readonly property bool open: Panels.appStoreOpen && here

    // A logo's file (AppStore.iconFor gives a file URL, or a theme's), or
    // Flathub's https URL (tessera-ui fetches and keeps it, as Qt's Image would)
    function file(u) {
        const s = String(u ?? "");
        return s.startsWith("file://") ? decodeURIComponent(s.slice(7)) : s.startsWith("/") || s.startsWith("https://") ? s : "";
    }
    function enrich(a) {
        return a ? Object.assign({}, a, { _icon: file(AppStore.iconFor(a)), _source: AppStore.sourceName(a) }) : null;
    }
    readonly property var feed: ({
        open: Panels.appStoreOpen,
        catalogue: AppStore.status.catalogue === false && Tools.has("pacman"),
        updates: Updates.running,
        jobs: AppStore.jobs,
        app: enrich(Panels.appStoreApp)
    })
    function send() {
        RustUi.tell("appstore", { state: root.feed });
    }
    onFeedChanged: send()

    // The store runs while its window is open (as the QML window counts itself)
    property bool counted: false
    onOpenChanged: {
        if (open && !counted) {
            counted = true;
            AppStore.users++;
            AppStore.refreshStatus();
        } else if (!open && counted) {
            counted = false;
            AppStore.users--;
        }
    }
    Component.onDestruction: if (counted) AppStore.users--

    Connections {
        target: RustUi
        // (said again when tessera-ui starts, or starts drawing it)
        function onStarted() {
            root.send();
        }
        function onDrawnChanged() {
            root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "appstore")
                return;
            root.call(what, a);
        }
    }
    Connections {
        target: Panels
        function onAppStoreAppChanged() {
            if (Panels.appStoreApp)
                RustUi.tell("appstore", { show: root.enrich(Panels.appStoreApp) });
        }
    }
    Connections {
        target: AppStore
        function onChanged(key, installed) {
            RustUi.tell("appstore", { changed: { key: key, installed: installed } });
        }
    }

    function call(what, a) {
        switch (what) {
        case "load": {
            // (the QML window's load(): the same questions)
            const token = a[0], view = String(a[1] ?? ""), category = String(a[2] ?? ""), query = String(a[3] ?? ""), filter = String(a[4] ?? "all");
            const done = r => RustUi.tell("appstore", { answer: {
                token: token,
                error: r.error ?? "",
                apps: (r.apps ?? []).map(root.enrich),
                sections: (r.sections ?? []).map(s => ({ title: s.title ?? "", apps: (s.apps ?? []).map(root.enrich) }))
            } });
            if (view === "discover")
                AppStore.ask("discover", {}, done);
            else if (view === "category")
                AppStore.ask("category", { category: category }, done);
            else if (view === "installed")
                AppStore.ask("installed", {}, done);
            else
                AppStore.ask("search", { q: query.replace(/^aur:\s*/i, ""), limit: 40, sources: filter === "all" ? ["repo", "flathub"] : [filter] }, done);
            break;
        }
        case "close":
            Panels.appStoreOpen = false;
            break;
        case "install":
            if (a[0]?.key)
                AppStore.install(a[0]);
            break;
        case "open":
            if (a[0]?.key)
                AppStore.open(a[0]);
            break;
        case "remove":
            if (a[0]?.key)
                AppStore.remove(a[0]);
            break;
        case "catalogue":
            AppStore.installCatalogue();
            break;
        case "website":
            Launch.openLink(String(a[0] ?? ""));
            break;
        case "copy":
            Quickshell.clipboardText = String(a[0] ?? "");
            break;
        }
    }
}
