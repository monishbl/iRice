import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.services
import qs.modules.settings

// The App Store: a regular window. Search and the store's places on the
// left (Discover, categories, Installed), apps as cards on the right, and an
// app's page over them: its logo, where it comes from, Get / Open / Remove.
// Apps come from the Arch repos and Flathub (services/AppStore); the AUR's
// community packages only when asked for (the AUR filter on search results,
// or "aur: name"), each saying what it is, with its votes and popularity,
// before it's installed. Spotlight's "Get" rows open it on that app.
// Material: like the Play Store. A pill search bar over a navigation drawer
// (the open place on a secondaryContainer pill), Material filter chips, app
// cards on surfaceContainer, and an app's page with a big logo, a filled
// Install pill and its details as an Expressive segmented list.
Scope {
    LazyLoader {
        active: Panels.appStoreOpen
        FloatingWindow {
            id: win
            title: "Software"
            implicitWidth: Theme.material ? 1040 : 980
            implicitHeight: Theme.material ? 720 : 660
            minimumSize: Theme.material ? Qt.size(820, 640) : Qt.size(760, 500)
            color: "transparent"
            onClosed: Panels.appStoreOpen = false

            // What's on show: a place (discover | category | installed |
            // search), and an app's page over it when `detail` is set
            property string view: "discover"
            property string category: ""
            property string query: ""
            property string filter: "all" // search results from: all (repos, Flathub) | repo | flathub | aur
            property var apps: []
            property var sections: [] // Discover: [{ title, apps }]
            property bool loading: false
            property string failed: ""
            property var detail: null
            property int token: 0 // (an answer for a place no longer shown is dropped)

            readonly property var places: [
                { id: "discover", name: "Discover", icon: "compass" },
                { id: "Productivity", name: "Productivity", icon: "briefcase", category: true },
                { id: "Development", name: "Development", icon: "code", category: true },
                { id: "Graphics & Design", name: "Graphics & Design", icon: "paint-brush", category: true },
                { id: "Music & Video", name: "Music & Video", icon: "film-strip", category: true },
                { id: "Games", name: "Games", icon: "game-controller", category: true },
                { id: "Internet", name: "Internet", icon: "globe", category: true },
                { id: "Education & Science", name: "Education & Science", icon: "graduation-cap", category: true },
                { id: "Utilities", name: "Utilities", icon: "wrench", category: true },
                { id: "System", name: "System", icon: "gear", category: true },
                { id: "installed", name: "Installed", icon: "download-simple" }
            ]
            readonly property string placeId: view === "category" ? category : view
            readonly property string heading: view === "search" ? "Results for “" + query + "”" : (places.find(p => p.id === placeId)?.name ?? "")

            function isOn(place) {
                return view !== "search" && placeId === place.id;
            }
            function pick(place) {
                searchBox.text = "";
                go(place.category ? "category" : place.id, place.category ? place.id : "");
            }
            function go(v, cat) {
                detail = null;
                view = v;
                category = cat ?? "";
                load();
            }
            function load() {
                const t = ++token;
                loading = true;
                failed = "";
                apps = [];
                sections = [];
                const done = r => {
                    if (t !== token)
                        return;
                    loading = false;
                    failed = r.error ?? "";
                    apps = r.apps ?? [];
                    sections = r.sections ?? [];
                };
                if (view === "discover")
                    AppStore.ask("discover", {}, done);
                else if (view === "category")
                    AppStore.ask("category", { category: category }, done);
                else if (view === "installed")
                    AppStore.ask("installed", {}, done);
                else
                    AppStore.ask("search", { q: query.replace(/^aur:\s*/i, ""), limit: 40, sources: filter === "all" ? ["repo", "flathub"] : [filter] }, done);
            }
            Component.onCompleted: {
                AppStore.users++;
                AppStore.refreshStatus();
                load();
                if (Panels.appStoreApp)
                    detail = Panels.appStoreApp;
            }
            Component.onDestruction: AppStore.users--

            // The search field on show (Material has its own pill), and what
            // typing in either does
            readonly property Item searchBox: Theme.material && m3Search.item ? m3Search.item : search
            function typed(text) {
                win.query = text;
                if (/^aur:/i.test(text))
                    win.filter = "aur";
                searchDebounce.restart();
            }
            function entered() {
                searchDebounce.stop();
                if (win.query.trim().length >= 2)
                    win.go("search");
            }
            Connections {
                target: Panels
                function onAppStoreAppChanged() {
                    if (Panels.appStoreApp)
                        win.detail = Panels.appStoreApp;
                }
            }
            // Installed or removed: the page and the list say so
            Connections {
                target: AppStore
                function onChanged(key, installed) {
                    if (win.detail?.key === key)
                        win.detail = Object.assign({}, win.detail, { installed: installed });
                    win.apps = win.apps.map(a => a.key === key ? Object.assign({}, a, { installed: installed }) : a);
                    win.sections = win.sections.map(s => Object.assign({}, s, { apps: s.apps.map(a => a.key === key ? Object.assign({}, a, { installed: installed }) : a) }));
                    if (win.view === "installed")
                        win.load();
                }
            }
            Timer {
                id: searchDebounce
                interval: 300
                onTriggered: {
                    if (win.query.trim().length >= 2)
                        win.go("search");
                    else if (win.view === "search")
                        win.go("discover");
                }
            }

            // ── Sidebar ─────────────────────────────────────────────────
            Rectangle {
                id: sidebar
                width: Theme.material ? 272 : 230
                height: parent.height
                color: Theme.material ? Theme.m3.surfaceContainer : Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.78) : Qt.rgba(0.94, 0.94, 0.96, 0.8)

                TextField {
                    id: search
                    visible: !Theme.material
                    x: 10
                    y: 14
                    width: parent.width - 20
                    height: 28
                    placeholder: "Search apps"
                    commitOnBlur: false
                    onTextChanged: win.typed(text)
                    onCommitted: win.entered()
                    Keys.onEscapePressed: text = ""
                    Component.onCompleted: if (!Panels.appStoreApp && !Theme.material) focusInput()
                }
                // Material: the Play Store's pill search bar
                Loader {
                    id: m3Search
                    active: Theme.material
                    x: 12
                    y: 16
                    width: parent.width - 24
                    height: 56
                    sourceComponent: Rectangle {
                        id: pill
                        property alias text: pillInput.text
                        function focusInput() {
                            pillInput.cursorPosition = pillInput.length;
                            pillInput.forceActiveFocus();
                        }
                        radius: height / 2
                        color: pillInput.activeFocus ? Theme.m3.surfaceContainerHighest : pillHover.hovered ? Qt.tint(Theme.m3.surfaceContainerHigh, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHigh
                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                        Component.onCompleted: if (!Panels.appStoreApp) focusInput()
                        HoverHandler {
                            id: pillHover
                            cursorShape: Qt.IBeamCursor
                        }
                        TapHandler {
                            onTapped: if (!pillInput.activeFocus) pill.focusInput()
                        }
                        Icon {
                            id: pillGlass
                            x: 16
                            anchors.verticalCenter: parent.verticalCenter
                            name: "magnifying-glass"
                            size: 22
                            color: Theme.m3.onSurfaceVariant
                        }
                        TextInput {
                            id: pillInput
                            anchors { left: pillGlass.right; leftMargin: 12; right: pillClear.visible ? pillClear.left : parent.right; rightMargin: pillClear.visible ? 4 : 20; verticalCenter: parent.verticalCenter }
                            color: Theme.m3.onSurface
                            selectionColor: Qt.alpha(Theme.m3.primary, 0.3)
                            selectedTextColor: Theme.m3.onSurface
                            selectByMouse: true
                            activeFocusOnTab: true
                            clip: true
                            font { family: Theme.font.family; pixelSize: 16 }
                            Accessible.role: Accessible.EditableText
                            Accessible.name: "Search apps"
                            onTextChanged: win.typed(text)
                            onAccepted: win.entered()
                            Keys.onEscapePressed: text = ""
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: pillInput.text === ""
                                text: "Search apps"
                                color: Theme.m3.onSurfaceVariant
                                font: pillInput.font
                            }
                        }
                        IconButton {
                            id: pillClear
                            visible: pillInput.text !== ""
                            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            width: 40
                            height: 40
                            icon: "x"
                            iconSize: 20
                            iconColor: Theme.m3.onSurfaceVariant
                            tooltip: "Clear search"
                            onClicked: {
                                pillInput.text = "";
                                pill.focusInput();
                            }
                        }
                    }
                }

                Column {
                    anchors { left: parent.left; right: parent.right; top: Theme.material ? m3Search.bottom : search.bottom; topMargin: Theme.material ? 12 : 14; leftMargin: Theme.material ? 12 : 10; rightMargin: Theme.material ? 12 : 10 }
                    spacing: Theme.material ? 0 : 2
                    PlaceRow {
                        place: win.places[0]
                        selected: win.isOn(place)
                        onActivated: win.pick(place)
                    }
                    Text {
                        leftPadding: Theme.material ? 16 : 8
                        topPadding: Theme.material ? 12 : 14
                        bottomPadding: Theme.material ? 8 : 4
                        text: "Categories"
                        color: Theme.material ? Theme.m3.onSurfaceVariant : Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.material ? 12 : Theme.font.caption; weight: Theme.material ? Font.Medium : Font.DemiBold; letterSpacing: Theme.material ? 0.5 : 0 }
                    }
                    Repeater {
                        model: win.places.filter(p => p.category)
                        PlaceRow {
                            required property var modelData
                            place: modelData
                            selected: win.isOn(place)
                            onActivated: win.pick(place)
                        }
                    }
                    Item { width: 1; height: Theme.material ? 8 : 12 }
                    PlaceRow {
                        place: win.places[win.places.length - 1]
                        selected: win.isOn(place)
                        onActivated: win.pick(place)
                    }
                }
            }

            // A place in the sidebar
            // (inline components don't see this file's ids: they're given what they need)
            component PlaceRow: Squircle {
                id: row
                property var place
                property bool selected: false
                signal activated
                function activate() {
                    activated();
                }
                width: parent?.width ?? 0
                // (Material: a navigation drawer item, the open one on a
                // secondaryContainer pill)
                height: Theme.material ? 44 : 32
                radius: Theme.material ? height / 2 : 8
                color: Theme.material ? (selected ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, rowMouse.pressed ? 0.1 : rowMouse.containsMouse ? 0.08 : 0)) : Qt.alpha(Theme.m3.onSurface, rowMouse.pressed ? 0.1 : rowMouse.containsMouse ? 0.08 : 0))
                    : selected ? Theme.color.fillPressed : rowMouse.containsMouse ? Theme.color.fillHover : "transparent"
                Behavior on color { enabled: Theme.material; ColorAnimation { duration: Theme.motion.hover } }
                activeFocusOnTab: true
                Accessible.role: Accessible.ListItem
                Accessible.name: place.name
                Keys.onPressed: e => {
                    if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) {
                        row.activate();
                        e.accepted = true;
                    }
                }
                FocusRing { radius: row.radius + 2 }
                Icon {
                    x: Theme.material ? 16 : 10
                    anchors.verticalCenter: parent.verticalCenter
                    name: row.place.icon
                    size: Theme.material ? 22 : 16
                    fill: Theme.material && row.selected
                    color: Theme.material ? (row.selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant) : row.selected ? Theme.color.label : Theme.color.secondary
                }
                Text {
                    x: Theme.material ? 52 : 36
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.place.name
                    color: Theme.material ? (row.selected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant) : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: Theme.material ? (row.selected ? Font.DemiBold : Font.Medium) : row.selected ? Font.Medium : Font.Normal; letterSpacing: Theme.material ? 0.1 : 0 }
                }
                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: row.activate()
                }
            }

            // ── Content ─────────────────────────────────────────────────
            Rectangle {
                id: content
                anchors { left: sidebar.right; right: parent.right; top: parent.top; bottom: parent.bottom }
                color: Theme.material ? Theme.m3.surface : Theme.dark ? "#1E1E21" : "#F5F5F7"
                readonly property int gap: Theme.material ? 12 : 16
                readonly property int columns: Math.max(1, Math.floor((width - 48 + gap) / (300 + gap)))
                readonly property real cardW: (width - 48 - (columns - 1) * gap) / columns

                Flickable {
                    id: listScroll
                    visible: win.detail === null
                    anchors.fill: parent
                    contentHeight: listCol.implicitHeight + 48
                    boundsBehavior: Flickable.StopAtBounds
                    clip: true

                    Column {
                        id: listCol
                        x: 24
                        y: 24
                        width: content.width - 48
                        spacing: 24

                        Column {
                            spacing: Theme.material ? 8 : 4
                            Text {
                                text: win.heading
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.material ? 32 : Theme.font.large; weight: Theme.material ? Font.Medium : Font.DemiBold }
                            }
                            Text {
                                text: win.view === "installed" ? "Apps on this computer from the Arch repos, the AUR and Flathub"
                                    : win.view === "search" && win.filter === "aur" ? "Community packages from the AUR"
                                    : "From the Arch repos and Flathub"
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body }
                            }
                        }

                        // Where search results come from (the AUR only when chosen;
                        // Material: filter chips, the chosen one ticked)
                        QQ.Row {
                            visible: win.view === "search"
                            spacing: 8
                            Repeater {
                                model: [{ id: "all", name: "All" }, { id: "repo", name: "Arch repos" }, { id: "flathub", name: "Flathub" }, { id: "aur", name: "AUR" }]
                                Squircle {
                                    id: chipButton
                                    required property var modelData
                                    readonly property bool on: win.filter === modelData.id
                                    width: chipLabel.implicitWidth + (Theme.material ? (on ? 16 + 18 + 8 : 32) : 24)
                                    height: Theme.material ? 32 : 28
                                    radius: Theme.material ? 8 : 14
                                    color: Theme.material ? (on ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, chipMouse.containsMouse ? 0.08 : 0)) : Qt.alpha(Theme.m3.onSurfaceVariant, chipMouse.containsMouse ? 0.08 : 0))
                                        : on ? Theme.accentTint : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                    border.width: Theme.material ? (on ? 0 : 1) : on ? 1 : 0
                                    border.color: Theme.material ? Theme.m3.outlineVariant : Theme.accent
                                    Behavior on width { enabled: Theme.material; NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                                    activeFocusOnTab: true
                                    Accessible.role: Accessible.RadioButton
                                    Accessible.name: modelData.name
                                    Accessible.checked: on
                                    function choose() {
                                        win.filter = modelData.id;
                                        if (modelData.id !== "aur" && /^aur:/i.test(win.searchBox.text))
                                            win.searchBox.text = win.searchBox.text.replace(/^aur:\s*/i, "");
                                        win.load();
                                    }
                                    Keys.onPressed: e => {
                                        if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) {
                                            chipButton.choose();
                                            e.accepted = true;
                                        }
                                    }
                                    FocusRing { radius: chipButton.radius + 2 }
                                    Icon {
                                        visible: Theme.material && chipButton.on
                                        x: 8
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 18
                                        color: Theme.m3.onSecondaryContainer ?? "transparent"
                                    }
                                    Text {
                                        id: chipLabel
                                        anchors.horizontalCenter: Theme.material ? undefined : parent.horizontalCenter
                                        anchors.right: Theme.material ? parent.right : undefined
                                        anchors.rightMargin: 16
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: chipButton.modelData.name
                                        color: Theme.material ? (chipButton.on ? Theme.m3.onSecondaryContainer : Theme.m3.onSurfaceVariant) : chipButton.on ? Theme.accentInk : Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium; letterSpacing: Theme.material ? 0.1 : 0 }
                                    }
                                    MouseArea {
                                        id: chipMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        onClicked: chipButton.choose()
                                    }
                                }
                            }
                        }

                        // No catalogue yet: the repos' apps have no logos or categories
                        Squircle {
                            visible: AppStore.status.catalogue === false && Tools.has("pacman")
                            width: parent.width
                            height: Theme.material ? 80 : 64
                            radius: Theme.material ? 24 : Theme.radius.module - 6
                            color: Theme.material ? Theme.m3.secondaryContainer : Theme.color.fill
                            Icon {
                                x: 16
                                anchors.verticalCenter: parent.verticalCenter
                                name: "info"
                                size: Theme.material ? 24 : 20
                                color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.secondary
                            }
                            Column {
                                anchors { left: parent.left; leftMargin: Theme.material ? 56 : 48; right: catalogueButton.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                spacing: 2
                                Text {
                                    text: "Get the app catalogue"
                                    color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 16 : Theme.font.body; weight: Font.Medium }
                                }
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: "Logos, descriptions and categories for the Arch repos' apps (about 20 MB)"
                                    color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small }
                                }
                            }
                            SettingsButton {
                                id: catalogueButton
                                primary: Theme.material
                                anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                text: Updates.running ? "Installing…" : "Install Catalogue"
                                busy: Updates.running
                                onClicked: AppStore.installCatalogue()
                            }
                        }

                        // Loading, a problem, or nothing found
                        // (Material: the loading indicator beside the words)
                        QQ.Row {
                            visible: win.loading
                            spacing: 12
                            Loader {
                                active: Theme.material
                                visible: active
                                anchors.verticalCenter: parent.verticalCenter
                                sourceComponent: Spinner { size: 32 }
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: win.view !== "search" ? "Loading…" : win.filter === "aur" ? "Looking in the AUR…" : "Looking in the Arch repos and on Flathub…"
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body }
                            }
                        }
                        Text {
                            visible: !win.loading && win.failed !== ""
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Couldn't load the apps (" + win.failed + "). Check your connection, then choose the place again."
                            color: Theme.color.danger
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                        Column {
                            visible: !win.loading && win.failed === "" && win.apps.length === 0 && win.sections.length === 0
                            width: parent.width
                            spacing: 6
                            Text {
                                text: win.view === "search" ? "No apps found for “" + win.query + "”" : win.view === "installed" ? "No apps installed from here yet" : "Nothing here right now"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.Medium }
                            }
                            Text {
                                width: parent.width
                                wrapMode: Text.Wrap
                                text: win.view === "search" ? (win.filter === "aur" ? "Check the spelling, or try a shorter name." : "Check the spelling, or try a shorter name. Community packages are under AUR.") : win.view === "installed" ? "Apps you install from Software show here." : "The store couldn't reach Flathub. Check your connection."
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                        }

                        // Discover: sections of cards; elsewhere one grid
                        Repeater {
                            model: win.view === "discover" ? win.sections : []
                            Column {
                                id: sectionCol
                                required property var modelData
                                width: listCol.width
                                spacing: 12
                                Text {
                                    textFormat: Text.PlainText
                                    text: sectionCol.modelData.title
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 22 : Theme.font.title; weight: Theme.material ? Font.Medium : Font.DemiBold }
                                }
                                Flow {
                                    width: parent.width
                                    spacing: content.gap
                                    Repeater {
                                        model: sectionCol.modelData.apps
                                        AppCard {
                                            width: content.cardW
                                            onOpened: app => win.detail = app
                                        }
                                    }
                                }
                            }
                        }
                        Flow {
                            visible: win.view !== "discover"
                            width: parent.width
                            spacing: content.gap
                            Repeater {
                                model: win.view !== "discover" ? win.apps : []
                                AppCard {
                                    width: content.cardW
                                    onOpened: app => win.detail = app
                                }
                            }
                        }
                    }
                }

                // ── An app's page ───────────────────────────────────────
                Flickable {
                    id: page
                    visible: win.detail !== null
                    anchors.fill: parent
                    contentHeight: pageCol.implicitHeight + 48
                    boundsBehavior: Flickable.StopAtBounds
                    clip: true
                    readonly property var app: win.detail
                    readonly property string busyWith: AppStore.busyWith(app)

                    Column {
                        id: pageCol
                        x: 32
                        y: 16
                        width: Math.min(content.width - 64, 720)
                        spacing: 24

                        IconButton {
                            width: Theme.material ? 40 : implicitWidth
                            height: Theme.material ? 40 : implicitHeight
                            icon: "caret-left"
                            iconSize: Theme.material ? 24 : 16
                            tooltip: "Back"
                            onClicked: win.detail = null
                        }

                        QQ.Row {
                            width: parent.width
                            spacing: Theme.material ? 24 : 20
                            AppTile {
                                id: bigTile
                                app: page.app
                                size: Theme.material ? 112 : 96
                            }
                            Column {
                                width: parent.width - bigTile.width - parent.spacing
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.material ? 8 : 6
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: page.app?.name ?? ""
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 32 : Theme.font.display; weight: Theme.material ? Font.Medium : Font.DemiBold }
                                }
                                Text {
                                    width: parent.width
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    textFormat: Text.PlainText
                                    text: page.app?.summary ?? ""
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.material ? 16 : Theme.font.title }
                                }
                                QQ.Row {
                                    spacing: 8
                                    SourceChip { app: page.app }
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        visible: text !== ""
                                        text: page.app?.developer ? (page.app.source === "aur" ? "packaged by " : "by ") + page.app.developer : ""
                                        color: Theme.material ? Theme.m3.primary : Theme.color.tertiary
                                        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.Normal }
                                    }
                                }
                            }
                        }

                        // Get / Open / Remove (Material: the Play Store's wide
                        // Install pill; Remove beside Open, in the error container)
                        Column {
                            spacing: 8
                            QQ.Row {
                                spacing: 8
                                // (Material 3 Expressive: one split button,
                                // Install or Open, the rest in its menu)
                                SplitButton {
                                    height: 48
                                    text: page.busyWith === "installing" ? "Installing…" : page.busyWith === "removing" ? "Removing…" : page.app?.installed ? "Open" : "Install"
                                    icon: page.app?.installed ? "arrow-square-out" : "download-simple"
                                    busy: page.busyWith !== ""
                                    onClicked: page.app?.installed ? AppStore.open(page.app) : AppStore.install(page.app)
                                    menu: (page.app?.installed ? [{ text: "Remove", icon: "trash", danger: true, enabled: page.busyWith === "", action: () => AppStore.remove(page.app) }] : [])
                                        .concat(page.app?.homepage ? [{ text: "Open the website", icon: "globe", action: () => Launch.openLink(page.app.homepage) }] : [])
                                        .concat([{ text: "Copy the name", icon: "copy", action: () => Quickshell.clipboardText = page.app?.name ?? "" }])
                                }
                                SettingsButton {
                                    visible: !Theme.material && (page.app?.installed ?? false)
                                    danger: true
                                    text: page.busyWith === "removing" ? "Removing…" : "Remove"
                                    busy: page.busyWith !== ""
                                    onClicked: AppStore.remove(page.app)
                                }
                            }
                            Text {
                                visible: text !== ""
                                width: pageCol.width
                                wrapMode: Text.Wrap
                                text: AppStore.errorFor(page.app)
                                color: Theme.color.danger
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            Text {
                                visible: page.busyWith !== "" && page.app?.source !== "flathub"
                                text: "Software Update asks for your password, then shows its progress in the island."
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                        }

                        // A community package: what that means, and how used it is
                        Squircle {
                            visible: page.app?.source === "aur"
                            width: parent.width
                            height: aurText.implicitHeight + (Theme.material ? 40 : 28)
                            // (Material: a bold tertiary-container card)
                            radius: Theme.material ? 24 : Theme.radius.module - 6
                            color: Theme.material ? Theme.m3.tertiaryContainer : Qt.alpha(Theme.color.warning, Theme.dark ? 0.16 : 0.12)
                            border.width: Theme.material ? 0 : 1
                            border.color: Qt.alpha(Theme.color.warning, 0.4)
                            readonly property bool littleUsed: (page.app?.popularity ?? 0) < 0.1 && (page.app?.votes ?? 0) < 10
                            Icon {
                                x: Theme.material ? 20 : 14
                                y: Theme.material ? 20 : 14
                                name: "warning"
                                size: Theme.material ? 24 : 18
                                color: Theme.material ? Theme.m3.onTertiaryContainer : Theme.color.warning
                            }
                            Text {
                                id: aurText
                                x: Theme.material ? 56 : 44
                                y: Theme.material ? 20 : 14
                                width: parent.width - (Theme.material ? 76 : 58)
                                wrapMode: Text.Wrap
                                textFormat: Text.StyledText
                                text: "<b>A community package from the AUR.</b> It's built on this computer from a script anyone can upload, not checked by Arch. "
                                    + (parent.littleUsed ? "Hardly anyone uses this one: install it only if you know and trust it. " : "")
                                    + (page.app?.votes ?? 0) + " votes · popularity " + (page.app?.popularity ?? 0)
                                    + (page.app?.outOfDate ? " · flagged out of date" : "")
                                color: Theme.material ? Theme.m3.onTertiaryContainer : Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body }
                            }
                        }

                        Text {
                            visible: text !== ""
                            width: parent.width
                            wrapMode: Text.Wrap
                            textFormat: Text.PlainText
                            text: page.app?.description && page.app.description !== page.app.summary ? page.app.description : ""
                            color: Theme.color.label
                            lineHeight: Theme.material ? 1.3 : 1.25
                            font { family: Theme.font.family; pixelSize: Theme.material ? 15 : Theme.font.body }
                        }

                        // Details (Material: an Expressive segmented list)
                        Group {
                            visible: Theme.material
                            width: parent.width
                            title: "About this app"
                            Row {
                                icon: "package"
                                label: "Package"
                                detail: page.app?.pkg ?? ""
                            }
                            Row {
                                visible: !!page.app?.version
                                icon: "arrow-circle-up"
                                label: "Version"
                                detail: page.app?.version ?? ""
                            }
                            Row {
                                icon: "storefront"
                                label: "From"
                                detail: AppStore.sourceName(page.app) + (page.app?.repo ? " · " + page.app.repo : "")
                            }
                            Row {
                                visible: !!page.app?.homepage
                                icon: "globe"
                                label: "Website"
                                detail: page.app?.homepage ?? ""
                                SettingsButton {
                                    minor: true
                                    text: "Open"
                                    onClicked: Launch.openLink(page.app.homepage)
                                }
                            }
                        }
                        Column {
                            visible: !Theme.material
                            spacing: 6
                            Text {
                                text: "Package " + (page.app?.pkg ?? "") + (page.app?.version ? " · " + page.app.version : "") + (page.app?.repo ? " · " + page.app.repo : "")
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            Text {
                                visible: !!page.app?.homepage
                                text: "Website"
                                color: homeMouse.containsMouse ? Theme.accentInk : Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; underline: homeMouse.containsMouse }
                                MouseArea {
                                    id: homeMouse
                                    anchors { fill: parent; margins: -4 }
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Launch.openLink(page.app.homepage)
                                }
                            }
                        }
                    }
                }
            }

            // ── Pieces ──────────────────────────────────────────────────
            // An app's logo, or a quiet tile with its initial
            component AppTile: Item {
                id: tile
                property var app
                property real size: 48
                readonly property string source: AppStore.iconFor(app)
                width: size
                height: size
                AppIcon {
                    visible: tile.source !== ""
                    anchors.fill: parent
                    size: tile.size
                    source: tile.source
                }
                Squircle {
                    visible: tile.source === ""
                    anchors.fill: parent
                    radius: tile.size * 0.24
                    color: Theme.material ? Theme.m3.secondaryContainer : Theme.color.fill
                    Text {
                        anchors.centerIn: parent
                        text: (tile.app?.name ?? "?").slice(0, 1).toUpperCase()
                        color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: tile.size * 0.42; weight: Font.DemiBold }
                    }
                }
            }
            // Where it comes from
            component SourceChip: Squircle {
                id: chip
                property var app
                // (Material: an assist chip, outlined, 8 px corners)
                width: chipText.implicitWidth + (Theme.material ? 24 : 16)
                height: Theme.material ? 32 : 22
                radius: Theme.material ? 8 : 11
                color: Theme.material ? "transparent" : Theme.color.fill
                border.width: Theme.material ? 1 : 0
                border.color: Theme.color.separator
                Text {
                    id: chipText
                    anchors.centerIn: parent
                    text: AppStore.sourceName(chip.app) + (chip.app?.verified ? " · verified" : "")
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.caption; weight: Font.Medium }
                }
            }
            // An app in a list: logo, name, summary, where from; Get or Open
            component AppCard: Squircle {
                id: card
                required property var modelData
                signal opened(var app)
                readonly property string busyWith: AppStore.busyWith(modelData)
                // (Material: a card on surfaceContainer with a hover state layer)
                height: Theme.material ? 88 : 76
                radius: Theme.material ? 20 : Theme.radius.module - 6
                color: Theme.material ? Qt.tint(Theme.m3.surfaceContainer, Qt.alpha(Theme.m3.onSurface, cardMouse.pressed ? 0.1 : cardMouse.containsMouse ? 0.08 : 0))
                    : cardMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: modelData.name
                Keys.onPressed: e => {
                    if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) {
                        card.opened(card.modelData);
                        e.accepted = true;
                    }
                }
                FocusRing { radius: card.radius + 2 }
                MouseArea {
                    id: cardMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: card.opened(card.modelData)
                }
                AppTile {
                    x: Theme.material ? 16 : 14
                    anchors.verticalCenter: parent.verticalCenter
                    app: card.modelData
                    size: Theme.material ? 56 : 44
                }
                Column {
                    anchors { left: parent.left; leftMargin: Theme.material ? 88 : 70; right: getButton.left; rightMargin: Theme.material ? 12 : 10; verticalCenter: parent.verticalCenter }
                    spacing: Theme.material ? 0 : 2
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: card.modelData.name
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.material ? 16 : Theme.font.body; weight: Theme.material ? Font.Medium : Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        text: card.modelData.summary || " "
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small }
                    }
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: AppStore.sourceName(card.modelData)
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.material ? 12 : Theme.font.caption }
                    }
                }
                SettingsButton {
                    id: getButton
                    anchors { right: parent.right; rightMargin: Theme.material ? 16 : 14; verticalCenter: parent.verticalCenter }
                    text: card.busyWith !== "" ? "…" : card.modelData.installed ? "Open" : Theme.material ? "Install" : "Get"
                    busy: card.busyWith !== ""
                    // (an AUR package's Get goes to its page first: the warning)
                    onClicked: card.modelData.installed ? AppStore.open(card.modelData) : card.modelData.source === "aur" ? card.opened(card.modelData) : AppStore.install(card.modelData)
                }
            }
        }
    }
}
