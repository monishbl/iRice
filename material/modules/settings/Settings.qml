import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import qs.services
import "../../common/SettingsMap.js" as SettingsMap

// iRice Settings: a regular app window. Translucent sidebar of sections on the
// left (the compositor blurs what's behind it), the selected pane on the right.
// In the Material look it's Android's two-pane Settings (SettingsMaterial.qml).
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
            // The look's layout on show (see glassUi and materialUi below)
            readonly property Item ui: Theme.material ? materialUi.item : glassUi.item

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
            // pages (read from their files, so nothing has to load)
            property string query: ""
            property var index: [] // { page, text, detail }
            readonly property var results: {
                const q = query.trim().toLowerCase();
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
                command: ["sh", "-c", "cd \"$1\" && grep -HnoE '(label|title|detail|pageTitle): \"[^\"]+\"' *.qml", "sh", Qt.resolvedUrl("pages").toString().replace("file://", "")]
                stdout: StdioCollector {
                    onStreamFinished: {
                        const out = [];
                        let last = null;
                        for (const line of text.split("\n")) {
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
                    target = "";
                    return;
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

            // The Glass look: a translucent sidebar and the pane beside it.
            // Material has its own layout (SettingsMaterial.qml); either one
            // hands the window its page loader, scroller and search field.
            Loader {
                id: glassUi
                anchors.fill: parent
                active: !Theme.material
                onLoaded: win.query = ""
                sourceComponent: Item {
                    readonly property Loader pages: pageLoader
                    readonly property Flickable scroll: scroller
                    readonly property Item searchField: search
                    function flashAt(x, y, w, h) {
                        flash.x = x;
                        flash.y = y;
                        flash.width = w;
                        flash.height = h;
                        flashAnim.restart();
                    }
                    Rectangle {
                        id: sidebar
                        width: 230
                        height: parent.height
                        color: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.78) : Qt.rgba(0.94, 0.94, 0.96, 0.8)

                        // Search, sticky at the top. Expanded at the top of the list or
                        // while in use; scrolled down and idle it folds into a magnifier
                        // (hover or click opens it again)
                        readonly property bool searchFolded: sideScroll.contentY > 4 && !search.editing && win.query === ""
                        Item {
                            id: searchBar
                            x: 10
                            y: 14
                            z: 2
                            width: parent.width - 20
                            height: 28
                            HoverHandler { id: barHover }

                            TextField {
                                id: search
                                width: sidebar.searchFolded && !barHover.hovered ? 28 : parent.width
                                height: parent.height
                                opacity: sidebar.searchFolded && !barHover.hovered ? 0 : 1
                                placeholder: "Search"
                                commitOnBlur: false
                                onTextChanged: win.query = text
                                Keys.onEscapePressed: {
                                    text = "";
                                    release();
                                }
                                // Down: into the results (or the sidebar)
                                Keys.onDownPressed: {
                                    let n = nextItemInFocusChain(true);
                                    for (let i = 0; i < 4 && n && (n === search || n.parent === search); i++)
                                        n = n.nextItemInFocusChain(true); // past the field's own text input
                                    n?.forceActiveFocus(Qt.TabFocusReason);
                                }
                                onCommitted: if (win.results.length > 0) win.go(win.results[0].page, win.results[0].section ? "" : win.results[0].text)
                                Behavior on width { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                            }
                            IconButton {
                                visible: search.opacity < 1
                                opacity: 1 - search.opacity
                                icon: "magnifying-glass"
                                tooltip: "Search"
                                onClicked: search.focusInput()
                            }
                        }

                        Flickable {
                            id: sideScroll
                            anchors { left: parent.left; right: parent.right; top: searchBar.bottom; topMargin: 10; bottom: parent.bottom }
                            contentHeight: list.implicitHeight + 16
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            Momentum {}

                            Column {
                                id: list
                                x: 10
                                width: sidebar.width - 20
                                spacing: 2

                                // Sections, or search results
                                Repeater {
                                    model: win.query.trim() === "" ? win.sections : []
                                    Column {
                                        id: entry
                                        required property var modelData
                                        required property int index
                                        width: list.width
                                        // A group heading above the first section of each group
                                        Text {
                                            visible: entry.index === 0 || win.sections[entry.index - 1].group !== entry.modelData.group
                                            width: parent.width
                                            leftPadding: 8
                                            topPadding: entry.index === 0 ? 0 : 12
                                            bottomPadding: 4
                                            text: entry.modelData.group
                                            color: Theme.color.tertiary
                                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                                        }
                                    Squircle {
                                        id: item
                                        readonly property var modelData: entry.modelData
                                        readonly property bool current: win.sectionId === modelData.id
                                        width: list.width
                                        height: 32
                                        radius: 8
                                        color: current ? Theme.accent : mouse.containsMouse ? Theme.color.fill : "transparent"
                                        // Keyboard: the sidebar is one Tab stop (the open
                                        // section); Up and Down open the next section
                                        activeFocusOnTab: current
                                        onCurrentChanged: if (current && win.sidebarKeys) forceActiveFocus(Qt.TabFocusReason)
                                        Keys.onPressed: event => {
                                            const i = win.sections.findIndex(x => x.id === modelData.id);
                                            const k = event.key;
                                            const j = k === Qt.Key_Up ? i - 1 : k === Qt.Key_Down ? i + 1 : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? win.sections.length - 1 : -2;
                                            if (j === -2 || j < 0 || j >= win.sections.length)
                                                return;
                                            win.sidebarKeys = true;
                                            win.go(win.sections[j].id);
                                            Qt.callLater(() => win.sidebarKeys = false);
                                            event.accepted = true;
                                        }
                                        Accessible.role: Accessible.PageTab
                                        Accessible.name: modelData.name
                                        Accessible.checked: current
                                        Accessible.onPressAction: win.go(modelData.id)
                                        FocusRing {
                                            radius: 12
                                        }

                                        Squircle {
                                            id: tile
                                            x: 6
                                            anchors.verticalCenter: parent.verticalCenter
                                            width: 22
                                            height: 22
                                            radius: 6
                                            color: item.modelData.color
                                            Icon { anchors.centerIn: parent; name: item.modelData.icon; fill: true; size: 13; color: "white" }
                                        }
                                        Text {
                                            anchors { left: tile.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                                            text: item.modelData.name
                                            color: item.current ? Theme.color.onAccent : Theme.color.label
                                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: item.current ? Font.DemiBold : Font.Normal }
                                        }
                                        MouseArea {
                                            id: mouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            onClicked: win.go(item.modelData.id)
                                        }
                                    }
                                    }
                                }

                                Repeater {
                                    model: win.results
                                    Squircle {
                                        id: result
                                        required property var modelData
                                        readonly property var section: win.sections.find(s => s.id === modelData.page.split("/")[0])
                                        width: list.width
                                        height: Math.max(36, texts.implicitHeight + 12)
                                        radius: 8
                                        color: resultMouse.containsMouse ? Theme.color.fill : "transparent"
                                        activeFocusOnTab: true
                                        Keys.onPressed: event => {
                                            const k = event.key;
                                            if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) {
                                                win.go(modelData.page, modelData.section ? "" : modelData.text);
                                            } else if (k === Qt.Key_Down || k === Qt.Key_Up) {
                                                nextItemInFocusChain(k === Qt.Key_Down)?.forceActiveFocus(k === Qt.Key_Down ? Qt.TabFocusReason : Qt.BacktabFocusReason);
                                            } else {
                                                return;
                                            }
                                            event.accepted = true;
                                        }
                                        Accessible.role: Accessible.ListItem
                                        Accessible.name: modelData.text
                                        Accessible.description: section?.name ?? ""
                                        Accessible.onPressAction: win.go(modelData.page, modelData.section ? "" : modelData.text)
                                        FocusRing {
                                            radius: 12
                                        }

                                        Squircle {
                                            id: resultTile
                                            x: 6
                                            y: 8
                                            width: 22
                                            height: 22
                                            radius: 6
                                            color: result.section?.color ?? Theme.color.fill
                                            Icon { anchors.centerIn: parent; name: result.section?.icon ?? "gear"; fill: true; size: 13; color: "white" }
                                        }
                                        Column {
                                            id: texts
                                            anchors { left: resultTile.right; leftMargin: 10; right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
                                            spacing: 1
                                            Text {
                                                width: parent.width
                                                text: result.modelData.text
                                                elide: Text.ElideRight
                                                color: Theme.color.label
                                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                                            }
                                            Text {
                                                width: parent.width
                                                visible: !result.modelData.section
                                                text: result.section?.name ?? ""
                                                elide: Text.ElideRight
                                                color: Theme.color.secondary
                                                font { family: Theme.font.family; pixelSize: Theme.font.caption }
                                            }
                                        }
                                        MouseArea {
                                            id: resultMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            onClicked: win.go(result.modelData.page, result.modelData.section ? "" : result.modelData.text)
                                        }
                                    }
                                }

                                // No results
                                Column {
                                    visible: win.query.trim() !== "" && win.results.length === 0
                                    width: list.width
                                    topPadding: 24
                                    spacing: 4
                                    Text {
                                        width: parent.width
                                        horizontalAlignment: Text.AlignHCenter
                                        text: "No results"
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                                    }
                                    Text {
                                        width: parent.width
                                        horizontalAlignment: Text.AlignHCenter
                                        wrapMode: Text.Wrap
                                        text: "Try a shorter word, like \"dock\" or \"sound\"."
                                        color: Theme.color.secondary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                                    }
                                }
                            }
                        }
                        ScrollIndicator {
                            target: sideScroll
                            anchors { right: sideScroll.right; top: sideScroll.top; bottom: sideScroll.bottom; rightMargin: 1 }
                        }
                    }

                    Rectangle {
                        anchors { left: sidebar.right; right: parent.right; top: parent.top; bottom: parent.bottom }
                        color: Theme.dark ? "#1E1E21" : "#F5F5F7"

                        // Toolbar: back and forward, and where you are (Section › Subpage;
                        // the section part goes back up to it)
                        Item {
                            id: toolbar
                            anchors { left: parent.left; right: parent.right; top: parent.top }
                            height: 52
                            QQ.Row {
                                id: arrows
                                x: 16
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 2
                                IconButton {
                                    icon: "caret-left"
                                    iconSize: 16
                                    tooltip: "Back"
                                    enabled: win.back.length > 0
                                    opacity: enabled ? 1 : 0.35
                                    onClicked: win.goBack()
                                }
                                IconButton {
                                    icon: "caret-right"
                                    iconSize: 16
                                    tooltip: "Forward"
                                    enabled: win.forward.length > 0
                                    opacity: enabled ? 1 : 0.35
                                    onClicked: win.goForward()
                                }
                            }
                            QQ.Row {
                                anchors { left: arrows.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                                spacing: 6
                                Text {
                                    id: crumb
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: win.section?.name ?? ""
                                    color: win.isSub ? (crumbMouse.containsMouse ? Theme.color.label : Theme.color.secondary) : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                                    MouseArea {
                                        id: crumbMouse
                                        anchors.fill: parent
                                        enabled: win.isSub
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.go(win.sectionId)
                                    }
                                }
                                Icon {
                                    visible: win.isSub
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: "caret-right"
                                    bold: true
                                    size: 12
                                    color: Theme.color.tertiary
                                }
                                Text {
                                    visible: win.isSub
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: pageLoader.item?.pageTitle ?? ""
                                    color: Theme.color.label
                                    font: crumb.font
                                }
                            }
                            Rectangle { // a hairline once the page scrolls under it
                                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                                height: 1
                                color: Theme.color.separator
                                opacity: scroller.contentY > 2 ? 1 : 0
                                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                            }
                        }

                        Flickable {
                            id: scroller
                            anchors { left: parent.left; right: parent.right; top: toolbar.bottom; bottom: parent.bottom }
                            contentHeight: page.implicitHeight + 64
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            Momentum {}

                            Column {
                                id: page
                                x: 28
                                y: 8
                                width: scroller.width - 56
                                spacing: 22

                                Loader {
                                    id: pageLoader
                                    width: parent.width
                                    source: Qt.resolvedUrl("pages/" + Panels.settingsPage.replace("/", "-") + ".qml")
                                    onLoaded: {
                                        scroller.contentY = 0;
                                        Qt.callLater(win.reveal);
                                    }
                                }
                            }

                            // Where a search result landed
                            Squircle {
                                id: flash
                                radius: 12
                                color: "transparent"
                                border.width: 2
                                border.color: Theme.accent
                                opacity: 0
                                SequentialAnimation {
                                    id: flashAnim
                                    NumberAnimation { target: flash; property: "opacity"; to: 1; duration: 150; easing.type: Easing.OutCubic }
                                    PauseAnimation { duration: 900 }
                                    NumberAnimation { target: flash; property: "opacity"; to: 0; duration: 400; easing.type: Easing.InCubic }
                                }
                            }
                        }
                        ScrollIndicator {
                            target: scroller
                            anchors { right: scroller.right; top: scroller.top; bottom: scroller.bottom; rightMargin: 2 }
                        }
                    }
                }
            }

            Loader {
                id: materialUi
                anchors.fill: parent
                active: Theme.material
                onLoaded: win.query = ""
                // (through this loader: inside SettingsMaterial `win` is its own property)
                readonly property var settingsWindow: win
                sourceComponent: SettingsMaterial {
                    host: materialUi.settingsWindow
                }
            }
        }
    }
}
