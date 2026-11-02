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
Scope {
    LazyLoader {
        active: Panels.appStoreOpen
        FloatingWindow {
            id: win
            title: "App Store"
            implicitWidth: 980
            implicitHeight: 660
            minimumSize: Qt.size(760, 500)
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
                search.text = "";
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
                width: 230
                height: parent.height
                color: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.78) : Qt.rgba(0.94, 0.94, 0.96, 0.8)

                TextField {
                    id: search
                    x: 10
                    y: 14
                    width: parent.width - 20
                    height: 28
                    placeholder: "Search apps"
                    commitOnBlur: false
                    onTextChanged: {
                        win.query = text;
                        if (/^aur:/i.test(text))
                            win.filter = "aur";
                        searchDebounce.restart();
                    }
                    onCommitted: {
                        searchDebounce.stop();
                        if (win.query.trim().length >= 2)
                            win.go("search");
                    }
                    Keys.onEscapePressed: text = ""
                    Component.onCompleted: if (!Panels.appStoreApp) focusInput()
                }

                Column {
                    anchors { left: parent.left; right: parent.right; top: search.bottom; topMargin: 14; leftMargin: 10; rightMargin: 10 }
                    spacing: 2
                    PlaceRow {
                        place: win.places[0]
                        selected: win.isOn(place)
                        onActivated: win.pick(place)
                    }
                    Text {
                        leftPadding: 8
                        topPadding: 14
                        bottomPadding: 4
                        text: "Categories"
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
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
                    Item { width: 1; height: 12 }
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
                height: 32
                radius: 8
                color: selected ? Theme.color.fillPressed : rowMouse.containsMouse ? Theme.color.fillHover : "transparent"
                activeFocusOnTab: true
                Accessible.role: Accessible.ListItem
                Accessible.name: place.name
                Keys.onPressed: e => {
                    if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) {
                        row.activate();
                        e.accepted = true;
                    }
                }
                FocusRing { radius: 10 }
                Icon {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    name: row.place.icon
                    size: 16
                    color: row.selected ? Theme.color.label : Theme.color.secondary
                }
                Text {
                    x: 36
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.place.name
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: row.selected ? Font.Medium : Font.Normal }
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
                color: Theme.dark ? "#1E1E21" : "#F5F5F7"
                readonly property int columns: Math.max(1, Math.floor((width - 48 + 16) / (300 + 16)))
                readonly property real cardW: (width - 48 - (columns - 1) * 16) / columns

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
                            spacing: 4
                            Text {
                                text: win.heading
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.DemiBold }
                            }
                            Text {
                                text: win.view === "installed" ? "Apps on this computer from the Arch repos, the AUR and Flathub"
                                    : win.view === "search" && win.filter === "aur" ? "Community packages from the AUR"
                                    : "From the Arch repos and Flathub"
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                        }

                        // Where search results come from (the AUR only when chosen)
                        QQ.Row {
                            visible: win.view === "search"
                            spacing: 8
                            Repeater {
                                model: [{ id: "all", name: "All" }, { id: "repo", name: "Arch repos" }, { id: "flathub", name: "Flathub" }, { id: "aur", name: "AUR" }]
                                Squircle {
                                    id: chipButton
                                    required property var modelData
                                    readonly property bool on: win.filter === modelData.id
                                    width: chipLabel.implicitWidth + 24
                                    height: 28
                                    radius: 14
                                    color: on ? Theme.accentTint : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                    border.width: on ? 1 : 0
                                    border.color: Theme.accent
                                    activeFocusOnTab: true
                                    Accessible.role: Accessible.RadioButton
                                    Accessible.name: modelData.name
                                    Accessible.checked: on
                                    function choose() {
                                        win.filter = modelData.id;
                                        if (modelData.id !== "aur" && /^aur:/i.test(search.text))
                                            search.text = search.text.replace(/^aur:\s*/i, "");
                                        win.load();
                                    }
                                    Keys.onPressed: e => {
                                        if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Space) {
                                            chipButton.choose();
                                            e.accepted = true;
                                        }
                                    }
                                    FocusRing { radius: 16 }
                                    Text {
                                        id: chipLabel
                                        anchors.centerIn: parent
                                        text: chipButton.modelData.name
                                        color: chipButton.on ? Theme.accentInk : Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
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
                            height: 64
                            radius: Theme.radius.module - 6
                            color: Theme.color.fill
                            Icon {
                                x: 16
                                anchors.verticalCenter: parent.verticalCenter
                                name: "info"
                                size: 20
                                color: Theme.color.secondary
                            }
                            Column {
                                anchors { left: parent.left; leftMargin: 48; right: catalogueButton.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                spacing: 2
                                Text {
                                    text: "Get the app catalogue"
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                                }
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: "Logos, descriptions and categories for the Arch repos' apps (about 20 MB)"
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                                }
                            }
                            SettingsButton {
                                id: catalogueButton
                                anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                text: Updates.running ? "Installing…" : "Install Catalogue"
                                busy: Updates.running
                                onClicked: AppStore.installCatalogue()
                            }
                        }

                        // Loading, a problem, or nothing found
                        Text {
                            visible: win.loading
                            text: win.view !== "search" ? "Loading…" : win.filter === "aur" ? "Looking in the AUR…" : "Looking in the Arch repos and on Flathub…"
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
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
                                text: win.view === "search" ? (win.filter === "aur" ? "Check the spelling, or try a shorter name." : "Check the spelling, or try a shorter name. Community packages are under AUR.") : win.view === "installed" ? "Apps you get from the App Store show here." : "The store couldn't reach Flathub. Check your connection."
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
                                    text: sectionCol.modelData.title
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                                }
                                Flow {
                                    width: parent.width
                                    spacing: 16
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
                            spacing: 16
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
                            icon: "caret-left"
                            iconSize: 16
                            tooltip: "Back"
                            onClicked: win.detail = null
                        }

                        QQ.Row {
                            width: parent.width
                            spacing: 20
                            AppTile {
                                app: page.app
                                size: 96
                            }
                            Column {
                                width: parent.width - 116
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 6
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: page.app?.name ?? ""
                                    color: Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.display; weight: Font.DemiBold }
                                }
                                Text {
                                    width: parent.width
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    text: page.app?.summary ?? ""
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.font.title }
                                }
                                QQ.Row {
                                    spacing: 8
                                    SourceChip { app: page.app }
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        visible: text !== ""
                                        text: page.app?.developer ? (page.app.source === "aur" ? "packaged by " : "by ") + page.app.developer : ""
                                        color: Theme.color.tertiary
                                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                                    }
                                }
                            }
                        }

                        // Get / Open / Remove
                        Column {
                            spacing: 8
                            QQ.Row {
                                spacing: 8
                                SettingsButton {
                                    primary: true
                                    text: page.busyWith === "installing" ? "Installing…" : page.app?.installed ? "Open" : "Get"
                                    busy: page.busyWith !== ""
                                    onClicked: page.app?.installed ? AppStore.open(page.app) : AppStore.install(page.app)
                                }
                                SettingsButton {
                                    visible: page.app?.installed ?? false
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
                            height: aurText.implicitHeight + 28
                            radius: Theme.radius.module - 6
                            color: Qt.alpha(Theme.color.warning, Theme.dark ? 0.16 : 0.12)
                            border.width: 1
                            border.color: Qt.alpha(Theme.color.warning, 0.4)
                            readonly property bool littleUsed: (page.app?.popularity ?? 0) < 0.1 && (page.app?.votes ?? 0) < 10
                            Icon {
                                x: 14
                                y: 14
                                name: "warning"
                                size: 18
                                color: Theme.color.warning
                            }
                            Text {
                                id: aurText
                                x: 44
                                y: 14
                                width: parent.width - 58
                                wrapMode: Text.Wrap
                                textFormat: Text.StyledText
                                text: "<b>A community package from the AUR.</b> It's built on this computer from a script anyone can upload, not checked by Arch. "
                                    + (parent.littleUsed ? "Hardly anyone uses this one: install it only if you know and trust it. " : "")
                                    + (page.app?.votes ?? 0) + " votes · popularity " + (page.app?.popularity ?? 0)
                                    + (page.app?.outOfDate ? " · flagged out of date" : "")
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body }
                            }
                        }

                        Text {
                            visible: text !== ""
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: page.app?.description && page.app.description !== page.app.summary ? page.app.description : ""
                            color: Theme.color.label
                            lineHeight: 1.25
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }

                        // Details
                        Column {
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
                                    onClicked: Launch.open(page.app.homepage)
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
                    color: Theme.color.fill
                    Text {
                        anchors.centerIn: parent
                        text: (tile.app?.name ?? "?").slice(0, 1).toUpperCase()
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: tile.size * 0.42; weight: Font.DemiBold }
                    }
                }
            }
            // Where it comes from
            component SourceChip: Squircle {
                id: chip
                property var app
                width: chipText.implicitWidth + 16
                height: 22
                radius: 11
                color: Theme.color.fill
                Text {
                    id: chipText
                    anchors.centerIn: parent
                    text: AppStore.sourceName(chip.app) + (chip.app?.verified ? " · verified" : "")
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                }
            }
            // An app in a list: logo, name, summary, where from; Get or Open
            component AppCard: Squircle {
                id: card
                required property var modelData
                signal opened(var app)
                readonly property string busyWith: AppStore.busyWith(modelData)
                height: 76
                radius: Theme.radius.module - 6
                color: cardMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
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
                FocusRing { radius: Theme.radius.module - 4 }
                MouseArea {
                    id: cardMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: card.opened(card.modelData)
                }
                AppTile {
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    app: card.modelData
                    size: 44
                }
                Column {
                    anchors { left: parent.left; leftMargin: 70; right: getButton.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: card.modelData.name
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: card.modelData.summary || " "
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: AppStore.sourceName(card.modelData)
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                }
                SettingsButton {
                    id: getButton
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    text: card.busyWith !== "" ? "…" : card.modelData.installed ? "Open" : "Get"
                    busy: card.busyWith !== ""
                    // (an AUR package's Get goes to its page first: the warning)
                    onClicked: card.modelData.installed ? AppStore.open(card.modelData) : card.modelData.source === "aur" ? card.opened(card.modelData) : AppStore.install(card.modelData)
                }
            }
        }
    }
}
