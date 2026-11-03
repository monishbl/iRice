import QtQuick
import qs.common
import qs.services

// What both looks' app lists show and do (AppLibraryGlass and AppDrawer only
// draw it): every app, grouped into category folders or listed A–Z, the
// search over them, the selection, and the keys' actions.
QtObject {
    id: root

    // The view's search field
    required property TextInput searchField

    // [label, freedesktop categories]; apps go in the first match
    readonly property var groups: [
        ["Productivity", ["Office", "Calendar", "ContactManagement", "Finance", "WordProcessor", "Spreadsheet"]],
        ["Development", ["Development", "IDE", "TextEditor"]],
        ["Internet", ["Network", "WebBrowser", "Email", "Chat", "InstantMessaging"]],
        ["Creativity", ["Graphics", "Photography", "2DGraphics", "3DGraphics", "VectorGraphics", "RasterGraphics"]],
        ["Entertainment", ["AudioVideo", "Audio", "Video", "Player", "Music"]],
        ["Games", ["Game"]],
        ["Education", ["Education", "Science", "Math"]],
        ["Utilities", ["Utility", "Accessories", "FileManager", "TerminalEmulator", "Archiving"]],
        ["System", ["System", "Settings", "Monitor", "PackageManager", "HardwareSettings"]]
    ]

    readonly property var apps: Apps.entries.filter(a => !a.noDisplay).slice().sort((a, b) => a.name.localeCompare(b.name))
    readonly property var folders: {
        const map = {};
        for (const a of apps) {
            const cats = a.categories ?? [];
            const g = groups.find(g => g[1].some(c => cats.includes(c)));
            const key = g ? g[0] : "Other";
            (map[key] = map[key] ?? []).push(a);
        }
        return groups.map(g => g[0]).concat(["Other"]).filter(k => map[k]).map(k => ({ name: k, apps: map[k] }));
    }

    property int view: 0 // 0 folders, 1 all apps
    property var folder: null // open folder
    property int selected: 0
    readonly property string query: searchField.text.trim().toLowerCase()
    readonly property bool showingApps: query !== "" || view === 1 || folder !== null
    readonly property var visibleApps: query !== "" ? apps.map(a => ({ a: a, s: Math.max(Search.score(a.name, query), Search.score(a.genericName, query) * 0.8, Search.score((a.keywords ?? []).join(" "), query) * 0.6) })).filter(x => x.s >= 30).sort((x, y) => y.s - x.s).map(x => x.a) : folder ? folder.apps : apps
    readonly property int count: showingApps ? visibleApps.length : folders.length
    onCountChanged: selected = Math.min(selected, Math.max(0, count - 1))

    // Back to the start: no search, no folder, the first one chosen
    function reset() {
        searchField.text = "";
        folder = null;
        selected = 0;
        searchField.forceActiveFocus();
    }
    function toggleView() {
        view = 1 - view;
        folder = null;
        selected = 0;
    }
    function activate(i) {
        if (showingApps) {
            const a = visibleApps[i];
            if (!a)
                return;
            Panels.close();
            Apps.launch(a);
        } else if (folders[i]) {
            folder = folders[i];
            selected = 0;
        }
    }
    // Escape: clears the search, then steps out of a folder, then closes
    function back() {
        if (searchField.text !== "")
            searchField.text = "";
        else if (folder) {
            selected = Math.max(0, folders.indexOf(folder));
            folder = null;
        } else
            Panels.close();
    }
    function move(d) {
        selected = Math.max(0, Math.min(count - 1, selected + d));
    }
}
