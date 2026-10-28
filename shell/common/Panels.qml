pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Which popover is open, and on which screen. Only one at a time, like macOS.
Singleton {
    id: root

    property string open: ""
    property var screen: null
    property real anchorX: 0 // left edge of the menu bar item that opened a menu
    property string ccPage: "" // Control Center detail page: "" | wifi | bluetooth | sound | focus | caffeine
    property string sessionAction: "" // Spotlight power action: preselect and require keyboard confirmation
    function showSession(action, scr) {
        show("session", scr, undefined, action);
    }
    // Share sheet: send these files to a nearby device
    property var sharePaths: []
    function pickShare() {
        FilePicker.pick({ multiple: true, title: "Send files" }, paths => share(paths));
    }
    function share(paths, scr) {
        sharePaths = paths;
        show("share", scr);
    }
    // Quick Look: a preview of this file over whatever is open ("" = none)
    property string quickLookPath: ""
    function quickLook(path) {
        quickLookPath = quickLookPath === path ? "" : (path ?? "");
    }
    property string exposeApp: "" // App Exposé: Mission Control shows only this app's windows (entry id or class)
    property bool ccEditing: false // Control Center "Edit Controls"
    property bool widgetEditing: false // desktop widgets: Edit Widgets (gallery open)
    // A control is being dragged (slider, reordering): panels hold still
    // instead of rubber-banding under the pointer
    property int holdPull: 0
    signal addWidget(string kind, string size) // from the gallery to the desktop of Panels.screen
    property bool settingsOpen: false
    property string settingsPage: "appearance"
    property bool osdVisible: false // banners move down while the HUD shows
    property int sidebarTab: 0
    property int ncTab: 0 // Notification Center widgets: 0 calendar, 1 to-do, 2 focus // 0 AI, 1 Translate, 2 Anime
    property var trayMenu: null // QsMenuHandle of the tray item whose menu is open
    property string lastClosed: ""
    property real lastClosedAt: 0

    // Menu bar items, so an open popover can pass clicks and hovers on the bar
    // strip to them (switch menus by clicking or sliding across, like macOS)
    property var barItems: []
    function registerBarItem(item) {
        barItems = barItems.concat([item]);
    }
    function unregisterBarItem(item) {
        barItems = barItems.filter(i => i !== item);
    }
    function barItemAt(scr, x) {
        return barItems.find(i => {
            if (!i.visible || i.QsWindow.window?.screen !== scr)
                return false;
            const p = i.mapToItem(null, 0, 0);
            return x >= p.x && x < p.x + i.width;
        }) ?? null;
    }

    function isOpen(name, scr) {
        return open === name && (!scr || screen === scr);
    }

    // App Exposé: one app's windows, from every space
    function appExpose(app, scr) {
        if (!app)
            return;
        exposeApp = app;
        show("missionControl", scr);
    }

    function show(name, scr, x, action) {
        if (name !== "missionControl")
            exposeApp = "";
        sessionAction = name === "session" ? (action ?? "") : "";
        open = name;
        // No screen given (Spotlight actions, shortcuts): the focused one
        screen = scr ?? Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
        if (x !== undefined)
            anchorX = x;
    }

    // A click on a menu bar item first dismisses the open popover (focus grab
    // cleared), then reaches the item; don't let that reopen the same popover.
    function toggle(name, scr, x) {
        if (isOpen(name, scr) || (lastClosed === name && Date.now() - lastClosedAt < 250))
            close();
        else
            show(name, scr, x);
    }

    function openSettings(page) {
        close();
        if (page)
            settingsPage = page;
        settingsOpen = true;
    }

    function close() {
        exposeApp = ""; // Mission Control keeps its own copy while it animates out
        if (open !== "") {
            lastClosed = open;
            lastClosedAt = Date.now();
        }
        open = "";
        ccPage = "";
    }
}
