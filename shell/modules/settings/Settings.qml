import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import qs.services
import "../../common/SettingsMap.js" as SettingsMap
import "SettingsIndex.js" as SettingsIndex

// iRice Settings: a regular app window. This is the window's logic, the same
// in both looks: history, search, paging and finding a row. Each look draws
// it its own way: Glass a translucent sidebar beside the pane
// (SettingsGlass.qml), Material Android's two panes (SettingsMaterial.qml).
Scope {
    LazyLoader {
        active: Panels.settingsOpen

        FloatingWindow {
            id: win
            title: "iRice Settings"
            // (Material: two panes, Android's 300 px list beside the page)
            implicitWidth: Theme.material ? 1040 : 860
            implicitHeight: Theme.material ? 700 : 620
            minimumSize: Theme.material ? Qt.size(840, 520) : Qt.size(720, 480)
            color: "transparent"
            onClosed: Panels.settingsOpen = false

            // Sidebar sections, under group headings, the same in both looks
            // (SettingsMap: the pages, the ones shown as a part of another,
            // where older page ids go). A page can open subpages
            // (SubpageLink): "page/sub" is the file pages/page-sub.qml, whose
            // pageTitle names it in the breadcrumb.
            readonly property var sections: SettingsMap.sections
            // The page on show: its section, and its subpage if any
            readonly property string sectionId: Panels.settingsPage.split("/")[0]
            readonly property var section: sections.find(x => x.id === sectionId) ?? null
            readonly property bool isSub: Panels.settingsPage.includes("/")
            // The look's layout on show (the view loader, below)
            readonly property Item ui: view.item

            // Back and forward, like a browser (the arrows, Alt+← / Alt+→,
            // the mouse's side buttons)
            property bool sidebarKeys: false // the open section changed from the sidebar's arrow keys: it takes focus
            property var back: []
            property var forward: []
            property string shown // (not a binding: it must still hold the old page when the page changes)
            Component.onCompleted: shown = Panels.settingsPage
            property bool stepping: false
            Connections {
                target: Panels
                function onSettingsPageChanged() {
                    if (SettingsMap.aliases[Panels.settingsPage])
                        return; // (an older id: Panels sets where it is now)
                    if (!win.stepping && win.shown !== Panels.settingsPage) {
                        win.back = win.back.concat([win.shown]).slice(-50);
                        win.forward = [];
                    }
                    win.shown = Panels.settingsPage;
                }
            }
            // Asked to show a row or part on the page already open
            Connections {
                target: Panels
                function onSettingsTargetChanged() {
                    if (Panels.settingsTarget !== "")
                        Qt.callLater(win.reveal);
                }
            }
            function goBack() {
                if (back.length === 0)
                    return;
                stepping = true;
                forward = [Panels.settingsPage].concat(forward);
                const to = back[back.length - 1];
                back = back.slice(0, -1);
                Panels.settingsPage = to;
                stepping = false;
            }
            function goForward() {
                if (forward.length === 0)
                    return;
                stepping = true;
                back = back.concat([Panels.settingsPage]);
                const to = forward[0];
                forward = forward.slice(1);
                Panels.settingsPage = to;
                stepping = false;
            }
            // Out of the way while you sign in to Google: the sign-in window
            // and the browser would open behind Settings (it floats; they
            // don't). It waits in the dock and comes back by itself when the
            // sign-in is done, failed or cancelled.
            property bool aside: false
            readonly property var toplevel: Hyprland.toplevels.values.find(t => t.title === "iRice Settings") ?? null
            Connections {
                target: Google
                function onSigningInChanged() {
                    if (Google.signingIn && !win.aside && win.toplevel) {
                        win.aside = true;
                        Apps.minimize(win.toplevel);
                    } else if (!Google.signingIn && win.aside) {
                        win.aside = false;
                        Apps.restore(win.toplevel);
                    }
                }
            }
            Shortcut { sequence: "Alt+Left"; onActivated: win.goBack() }
            Shortcut { sequence: "Alt+Right"; onActivated: win.goForward() }

            // Search: section names, plus every row and group caption in the
            // pages (read from their files, so nothing has to load; only
            // this look's, SettingsIndex)
            property string query: ""
            property var index: [] // { page, text, detail }
            readonly property var results: {
                // (an old name, e.g. Spotlight, finds what it's called now)
                const q = SettingsMap.searchQuery(query.trim().toLowerCase());
                if (q === "")
                    return [];
                const words = q.split(/\s+/);
                const hit = s => words.every(w => s.toLowerCase().includes(w));
                const out = [];
                for (const s of sections)
                    if (hit(s.name))
                        out.push({ page: s.id, text: s.name, detail: "", section: true });
                for (const e of index)
                    if (hit(e.text + " " + e.detail + " " + (sections.find(s => s.id === e.page.split("/")[0])?.name ?? "")))
                        out.push(e);
                return out.slice(0, 60);
            }
            Process {
                running: true
                command: ["sh", "-c", "cd \"$1\" && grep -HnE '[{}]|(label|title|detail|pageTitle): |(visible|active): *!?Theme[.]material' *.qml", "sh", Qt.resolvedUrl("pages").toString().replace("file://", "")]
                stdout: StdioCollector {
                    onStreamFinished: {
                        const out = [];
                        let last = null;
                        for (const line of SettingsIndex.captions(text, Theme.material)) {
                            const m = line.match(/^([a-z]+(?:-[a-z]+)?)\.qml:(\d+):(label|title|detail|pageTitle): "(.*)"$/);
                            if (!m || m[4].includes("+") || m[4].length < 3)
                                continue;
                            if (m[3] === "detail") {
                                if (last && last.page === m[1] && +m[2] - last.line <= 3)
                                    last.detail = m[4];
                                continue;
                            }
                            // (a page shown inside another: found there)
                            const at = SettingsMap.pageOfFile(m[1]);
                            last = { page: at.page, line: +m[2], text: m[4], detail: "" };
                            if (!out.some(e => e.page === last.page && e.text === last.text))
                                out.push(last);
                        }
                        win.index = out;
                    }
                }
            }

            // Go to a page and, for a search result, scroll to its row and flash it
            property string target: ""
            function go(page, text) {
                const old = SettingsMap.aliases[page];
                if (old) {
                    page = old.page;
                    text = text || old.part || "";
                }
                target = text ?? "";
                if (Panels.settingsPage === page)
                    Qt.callLater(reveal);
                else
                    Panels.settingsPage = page;
            }
            property int revealTries: 0
            property bool revealSettling: false
            property string revealAgain: ""
            Timer {
                id: revealSettle
                interval: 300
                onTriggered: {
                    win.target = win.revealAgain;
                    win.reveal();
                }
            }
            Timer {
                id: revealRetry
                interval: 60
                onTriggered: win.reveal()
            }
            function reveal() {
                const ui = win.ui;
                if (target === "" && Panels.settingsTarget !== "") {
                    target = Panels.settingsTarget;
                    Panels.settingsTarget = "";
                }
                if (target === "" || !ui || !ui.pages.item)
                    return;
                // (a part's heading, or a row in a closed More options: that
                // opens, and this runs again once it has)
                const find = (it, hidden) => {
                    for (const c of it.children) {
                        if ((c.label === target || c.title === target) && (c.visible || hidden))
                            return c;
                        const f = find(c, hidden || (c.moreOptions === true && !c.expanded));
                        if (f)
                            return f;
                    }
                    return null;
                };
                const row = find(ui.pages.item);
                if (!row) {
                    // (a merged page's parts load a moment after the page:
                    // look again a few times before giving up)
                    if (revealTries < 12) {
                        revealTries++;
                        revealRetry.restart();
                        return;
                    }
                    revealTries = 0;
                    target = "";
                    return;
                }
                revealTries = 0;
                // (and once more when the parts above it have settled: a part
                // loading late moves everything under it)
                if (!revealSettling) {
                    revealSettling = true;
                    revealAgain = target;
                    revealSettle.restart();
                } else {
                    revealSettling = false;
                }
                for (let up = row.parent; up; up = up.parent)
                    if (up.moreOptions === true && !up.expanded) {
                        up.expanded = true;
                        Qt.callLater(reveal);
                        return;
                    }
                target = "";
                const scroller = ui.scroll;
                const p = row.mapToItem(scroller.contentItem, 0, 0);
                scroller.contentY = Math.max(0, Math.min(scroller.contentHeight - scroller.height, p.y - 80));
                ui.flashAt(p.x - 4, p.y, row.width + 8, Math.min(row.height, Theme.material ? 80 : 64));
            }

            // The mouse's side buttons: back and forward
            TapHandler {
                acceptedButtons: Qt.BackButton | Qt.ForwardButton
                onTapped: (point, button) => button === Qt.BackButton ? win.goBack() : win.goForward()
            }
            // A click anywhere but the search field ends the search (it stays
            // put; the click still goes through to what's under it)
            MouseArea {
                anchors.fill: parent
                z: 1000
                enabled: win.ui?.searchField.editing ?? false
                acceptedButtons: Qt.AllButtons
                onPressed: mouse => {
                    const search = win.ui.searchField;
                    const p = mapToItem(search, mouse.x, mouse.y);
                    if (p.x < 0 || p.y < 0 || p.x > search.width || p.y > search.height)
                        search.release();
                    mouse.accepted = false;
                }
            }

            // The look's layout: Glass's translucent sidebar beside the pane
            // (SettingsGlass), or Material's two panes (SettingsMaterial).
            // Either hands the window its page loader, scroller and search
            // field (ui above).
            Loader {
                id: view
                anchors.fill: parent
                // (through the loader: inside the views `win` is their own property)
                readonly property var settingsWindow: win
                onLoaded: win.query = ""
                sourceComponent: Theme.material ? materialView : glassView
                Component {
                    id: glassView
                    SettingsGlass {
                        host: view.settingsWindow
                    }
                }
                Component {
                    id: materialView
                    SettingsMaterial {
                        host: view.settingsWindow
                    }
                }
            }
        }
    }
}

