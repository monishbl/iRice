pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// The App Store (modules/appstore, and Spotlight's "Get" rows): apps from
// the Arch repos, the AUR and Flathub, found by assets/scripts/appstore.py
// (kept running while the store or Spotlight wants it, two minutes after).
// Installing and removing: packages (repos, AUR) through Software Update,
// with the password and progress in the island; Flathub apps with flatpak,
// for this user only (no password), after installing flatpak itself if
// needed. An AUR package is a community one: the store says so first.
//   app: see appstore.py (key, source, pkg, name, summary, icon, iconName,
//        installed, desktop, votes, popularity, installs, verified, …)
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/appstore.py"
    readonly property bool available: Tools.has("pacman") || Tools.has("flatpak")

    // ── Asking the backend ──────────────────────────────────────────────
    property bool wanted: false
    property int users: 0 // windows that show the store (it runs while > 0)
    function want() {
        wanted = true;
        idle.restart();
    }
    Timer {
        id: idle
        interval: 120000
        onTriggered: if (root.users === 0) root.wanted = false
    }
    onUsersChanged: users > 0 ? want() : idle.restart()

    property int lastId: 0
    property var waiting: ({}) // id -> callback
    property var queued: [] // asked before it had started
    function ask(op, args, cb) {
        want();
        const id = ++lastId;
        waiting[id] = cb;
        const line = JSON.stringify(Object.assign({ id: id, op: op }, args ?? {})) + "\n";
        if (backend.running)
            backend.write(line);
        else
            queued = queued.concat([line]);
        return id;
    }
    KeptProcess {
        id: backend
        wanted: root.wanted
        command: ["python3", "-I", root.script, "--serve"]
        stdinEnabled: true
        onStarted: {
            for (const l of root.queued)
                write(l);
            root.queued = [];
        }
        stdout: SplitParser {
            onRead: line => {
                let r = null;
                try {
                    r = JSON.parse(line);
                } catch (e) {
                    return;
                }
                const cb = root.waiting[r.id];
                if (!r.partial) // (a search's local matches come first, then all)
                    delete root.waiting[r.id];
                if (cb)
                    cb(r);
            }
        }
    }

    // What the backend can reach: { catalogue, flatpak, helper }
    property var status: ({})
    function refreshStatus() {
        ask("status", {}, r => root.status = r.status ?? {});
    }

    // ── Spotlight: a few installable matches for what's typed ───────────
    // Asked only when nothing installed matches (Search.qml decides), once
    // typing pauses; the repos' matches come at once (they're on this
    // computer), Flathub's after; a search done in the last ten minutes
    // comes from memory, and searches typed past are dropped unanswered.
    property string quickQuery: ""
    property var quick: [] // apps, for quickQuery
    property bool quickLoading: false // (Spotlight shows "Looking for apps…" meanwhile)
    property int quickRevision: 0
    function lookUp(q) {
        if (q === quickQuery)
            return;
        quickQuery = q;
        quick = [];
        quickLoading = true;
        quickRevision++;
        quickDebounce.restart();
    }
    // Nothing to look up (an installed app matches): stop showing it
    function forget() {
        if (quickQuery === "" && !quickLoading)
            return;
        quickDebounce.stop();
        quickQuery = "";
        quick = [];
        quickLoading = false;
    }
    Timer {
        id: quickDebounce
        interval: 350
        onTriggered: {
            const q = root.quickQuery;
            root.ask("search", { q: q, limit: 4, stream: true }, r => {
                if (q !== root.quickQuery || r.stale)
                    return;
                root.quick = (r.apps ?? []).filter(a => !a.installed).slice(0, 3);
                if (!r.partial)
                    root.quickLoading = false;
                root.quickRevision++;
            });
        }
    }

    // ── An app's logo: the icon theme's (matching the other apps), else its
    // own (the catalogue's, or Flathub's), else "" (a tile with its initial)
    function iconFor(app) {
        if (!app)
            return "";
        // (not by an AUR package's name: themes fall back on dashes, so every
        // "spotify-something" would get Spotify's)
        const themed = IconLookup.exact((app.source === "aur" ? [app.iconName] : [app.iconName, app.pkg]).filter(x => !!x));
        if (themed !== "")
            return themed;
        const own = app.icon ?? "";
        return own.startsWith("/") ? "file://" + own : own;
    }
    function sourceName(app) {
        return app?.source === "repo" ? "Arch repos" : app?.source === "aur" ? "AUR · community" : app?.source === "flathub" ? "Flathub" : "";
    }

    // ── Installing, opening, removing ───────────────────────────────────
    property var jobs: ({}) // key -> { state: installing | removing, error }
    property string pendingKey: "" // the package job Software Update is running for us
    function busyWith(app) {
        return jobs[app?.key]?.state ?? "";
    }
    function errorFor(app) {
        return jobs[app?.key]?.error ?? "";
    }
    function setJob(key, job) {
        const j = Object.assign({}, jobs);
        if (job)
            j[key] = job;
        else
            delete j[key];
        jobs = j;
    }
    signal changed(string key, bool installed) // installed or removed

    function install(app) {
        if (!app || busyWith(app) !== "")
            return;
        if (app.source === "flathub")
            return flatpakJob(app, "install");
        if (Updates.running) {
            setJob(app.key, { state: "", error: "Software Update is busy. Try again when it's done." });
            return;
        }
        setJob(app.key, { state: "installing" });
        pendingKey = app.key;
        pendingApp = app;
        Updates.update(app.source === "aur" ? ["--aur-install", app.pkg] : ["--install", app.pkg]);
    }
    function remove(app) {
        if (!app || busyWith(app) !== "")
            return;
        if (app.source === "flathub")
            return flatpakJob(app, "remove");
        if (Updates.running) {
            setJob(app.key, { state: "", error: "Software Update is busy. Try again when it's done." });
            return;
        }
        setJob(app.key, { state: "removing" });
        pendingKey = app.key;
        pendingApp = app;
        Updates.update(["--remove", app.pkg]);
    }
    property var pendingApp: null
    Connections {
        target: Updates
        function onFinished(j) {
            const key = root.pendingKey;
            if (key === "")
                return;
            const app = root.pendingApp;
            const was = root.jobs[key]?.state ?? "";
            root.pendingKey = "";
            root.pendingApp = null;
            if (j.state === "done") {
                root.setJob(key, null);
                root.done(app, was);
            } else {
                root.setJob(key, { state: "", error: j.error || j.text || (was === "removing" ? "It couldn't be removed." : "It couldn't be installed.") });
            }
        }
    }
    // The app catalogue (logos, categories for the repos' apps)
    function installCatalogue() {
        if (!Updates.running)
            Updates.update(["--install", "archlinux-appstream-data"]);
    }

    function done(app, was) {
        ask("refresh", {}, () => {});
        root.changed(app?.key ?? "", was === "installing");
        if (!app)
            return;
        if (was === "installing")
            Island.post({ kind: "custom", icon: "check-circle", title: app.name + " is installed", subtitle: sourceName(app), color: Theme.accents.green, action: "Open", onAction: () => root.open(app) }, 8000);
        else
            Island.post({ kind: "custom", icon: "trash", title: app.name + " is removed", subtitle: sourceName(app), color: Theme.accent }, 5000);
    }

    // Flathub: per user, from Flathub (added once), flatpak itself first
    property var flatpakQueue: null // { app, op } waiting for flatpak to be installed
    function flatpakJob(app, op) {
        if (!Tools.has("flatpak")) {
            if (Updates.running) {
                setJob(app.key, { state: "", error: "Software Update is busy. Try again when it's done." });
                return;
            }
            setJob(app.key, { state: "installing" });
            flatpakQueue = { app: app, op: op };
            Updates.update(["--install", "flatpak"]);
            return;
        }
        setJob(app.key, { state: op === "install" ? "installing" : "removing" });
        flatpakProc.app = app;
        flatpakProc.op = op;
        flatpakProc.command = op === "install"
            ? ["sh", "-c", "flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo && flatpak install --user -y --noninteractive flathub \"$1\"", "sh", app.pkg]
            : ["flatpak", "uninstall", "--user", "-y", "--noninteractive", app.pkg];
        flatpakProc.running = true;
    }
    Connections {
        target: Updates
        function onFinished(j) {
            const q = root.flatpakQueue;
            if (!q)
                return;
            root.flatpakQueue = null;
            root.setJob(q.app.key, null);
            Tools.recheck();
            if (j.state === "done")
                Qt.callLater(() => root.flatpakJob(q.app, q.op));
            else
                root.setJob(q.app.key, { state: "", error: "Flatpak couldn't be installed. " + (j.error || "") });
        }
    }
    Process {
        id: flatpakProc
        property var app: null
        property string op: ""
        stderr: StdioCollector { id: flatpakErr }
        onExited: code => {
            const app = flatpakProc.app;
            const was = flatpakProc.op === "install" ? "installing" : "removing";
            if (code === 0) {
                root.setJob(app.key, null);
                root.done(app, was);
            } else {
                const said = flatpakErr.text.trim().split("\n").filter(l => l.trim() !== "").pop() ?? "";
                root.setJob(app.key, { state: "", error: (was === "installing" ? "It couldn't be installed. " : "It couldn't be removed. ") + said.replace(/^error:\s*/i, "") });
            }
        }
    }

    function open(app) {
        if (!app)
            return;
        const id = (app.desktop ?? "").replace(/\.desktop$/, "");
        const entry = (id !== "" ? DesktopEntries.byId(id) : null) ?? DesktopEntries.heuristicLookup(app.pkg) ?? DesktopEntries.heuristicLookup(app.name);
        if (entry)
            Launch.app(entry);
        else if (app.source === "flathub")
            Launch.run(["flatpak", "run", app.pkg], app.pkg);
        else if (id !== "")
            Launch.run(["gtk-launch", id], id);
    }
}
