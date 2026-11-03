import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// App Library in the Material look: Android's app drawer. A large surface
// rising from the dock on a spring, a search pill on top, then every app in
// an A–Z grid (with a letter scrubber) or in category folders (Material
// containers with a 2×2 icon preview); the two views are a connected
// button group. The same data, keys and actions as the Glass App Library
// (AppLibrary.qml): typing searches; arrow keys move; Enter opens; Tab
// switches views; Escape steps back out of a folder, then closes.
Variants {
    model: Theme.material ? Quickshell.screens : []

    PanelLoader {
        id: slot
        required property var modelData
        panel: "launchpad"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "launchpad"

            readonly property bool shown: Panels.isOpen("launchpad", screen)

            // Opens on Material's default spatial spring, closes quickly
            property real progress: 0
            Component.onCompleted: {
                reset();
                if (shown)
                    openAnim.start();
            }
            onShownChanged: {
                if (!shown)
                    menu = null;
                if (shown) {
                    reset();
                    closeAnim.stop();
                    openAnim.start();
                } else {
                    openAnim.stop();
                    closeAnim.start();
                }
            }
            SpringAnimation {
                id: openAnim
                target: win
                property: "progress"
                to: 1
                spring: Theme.motion.springDefault.spring
                damping: Theme.motion.springDefault.damping
                epsilon: 0.004
            }
            NumberAnimation {
                id: closeAnim
                target: win
                property: "progress"
                to: 0
                duration: Theme.motion.panel * 0.6
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.motion.exit
            }

            visible: progress > 0 && closeGuard.open
            // While closing, input passes through (the layer stays for the fade)
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            CloseGuard { id: closeGuard; shown: win.shown }
            anchors { top: true; bottom: true; left: true; right: true }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            // The apps, the search and the selection: AppLibraryModel, as
            // the Glass App Library
            AppLibraryModel {
                id: lib
                searchField: field
            }
            // Plain A–Z (all apps, no search): the letter scrubber shows
            readonly property bool alphabetical: lib.query === "" && lib.view === 1 && lib.folder === null

            // What the grid shows changed: its cells come in one after another
            property bool entering: false
            readonly property string page: (lib.showingApps ? "apps" : "folders") + "/" + (lib.folder?.name ?? "") + "/" + (lib.query === "" ? lib.view : "search")
            onPageChanged: enter()
            function enter() {
                entering = true;
                enterTimer.restart();
            }
            Timer {
                id: enterTimer
                interval: 500
                onTriggered: win.entering = false
            }

            function reset() {
                lib.reset();
                enter();
            }
            // An app's menu, as Android 17's: its own shortcuts (the desktop
            // file's actions: New Window, New Private Window…), then the
            // system's actions
            property var menu: null
            function openMenu(entry, m, from) {
                const id = entry.id ?? "";
                const pinned = Config.options.dock.pinned.includes(id);
                const own = (entry.actions ?? []).map(a => ({ text: a.name, action: () => {
                    Panels.close();
                    a.execute();
                } }));
                const items = (own.length > 0 ? [{ header: "Shortcuts" }].concat(own) : []).concat([{ header: "Actions" }, { text: "Open", action: () => {
                    Panels.close();
                    Apps.launch(entry);
                } }, { text: pinned ? "Remove from Dock" : "Keep in Dock", action: () => pinned ? Apps.unpin(id) : Apps.pin(id) }]);
                const p = from.mapToItem(null, m.x, m.y);
                menu = { items: items, x: p.x, y: p.y };
            }
            Loader {
                active: win.menu !== null
                sourceComponent: ContextMenu {
                    screen: win.screen
                    items: win.menu?.items ?? []
                    pointX: (win.menu?.x ?? 0) + menuWidth / 2
                    pointY: win.menu?.y ?? 0
                    upward: false
                    onDismissed: win.menu = null
                }
            }
            function move(d) {
                lib.move(d);
                grid.reveal(lib.selected);
            }

            // Material's scrim; a click on it closes
            Rectangle {
                anchors.fill: parent
                color: Theme.m3.scrim
                opacity: 0.32 * Math.max(0, Math.min(1, win.progress))
                MouseArea {
                    anchors.fill: parent
                    onClicked: Panels.close()
                }
            }

            // The drawer
            Rectangle {
                id: sheet
                width: Math.min(1120, win.width - 160)
                height: Math.min(780, win.height - 120)
                x: Math.round((win.width - width) / 2)
                y: Math.round((win.height - height) / 2)
                radius: 28
                color: Theme.veil(Theme.m3.surface)
                opacity: Math.max(0, Math.min(1, win.progress * 1.6))
                // Rises from the dock and grows, on a spring, as the app
                // drawer swipes up
                transform: [
                    Scale {
                        origin.x: sheet.width / 2
                        origin.y: sheet.height
                        xScale: 0.94 + 0.06 * win.progress
                        yScale: 0.94 + 0.06 * win.progress
                    },
                    Translate {
                        y: (1 - win.progress) * 96
                    }
                ]

                // Swallow clicks on the drawer so they don't reach the scrim
                MouseArea { anchors.fill: parent }

                Item {
                    id: content
                    anchors { fill: parent; margins: 24 }

                    // Header: the search pill and the view switch
                    Item {
                        id: header
                        width: parent.width
                        height: 56

                        Rectangle {
                            id: search
                            width: parent.width - views.width - 16
                            height: 56
                            radius: 28
                            color: Theme.m3.surfaceContainerHigh

                            Icon {
                                visible: !backButton.visible
                                x: 18
                                anchors.verticalCenter: parent.verticalCenter
                                name: "magnifying-glass"
                                size: 22
                                color: Theme.m3.onSurfaceVariant
                            }
                            // (in a folder: back out of it)
                            IconButton {
                                id: backButton
                                visible: lib.folder !== null && lib.query === ""
                                x: 8
                                anchors.verticalCenter: parent.verticalCenter
                                width: 40
                                height: 40
                                icon: "caret-left"
                                iconSize: 22
                                iconColor: Theme.m3.onSurface
                                tooltip: "Back to categories"
                                onClicked: lib.back()
                            }
                            TextInput {
                                id: field
                                anchors { fill: parent; leftMargin: 56; rightMargin: clear.visible ? 56 : 24 }
                                verticalAlignment: TextInput.AlignVCenter
                                color: Theme.m3.onSurface
                                selectionColor: Theme.m3.primaryContainer
                                selectedTextColor: Theme.m3.onPrimaryContainer
                                clip: true
                                font { family: Theme.font.family; pixelSize: 16 }
                                Accessible.role: Accessible.EditableText
                                Accessible.name: "Search apps"
                                onTextChanged: lib.selected = 0
                                Keys.onEscapePressed: lib.back()
                                Keys.onReturnPressed: lib.activate(lib.selected)
                                Keys.onEnterPressed: lib.activate(lib.selected)
                                Keys.onTabPressed: lib.toggleView()
                                Keys.onLeftPressed: e => { if (text === "") win.move(-1); else e.accepted = false; }
                                Keys.onRightPressed: e => { if (text === "") win.move(1); else e.accepted = false; }
                                Keys.onUpPressed: win.move(-grid.columns)
                                Keys.onDownPressed: win.move(grid.columns)
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width
                                    visible: field.text === ""
                                    text: "Search " + lib.apps.length + " apps"
                                    elide: Text.ElideRight
                                    color: Theme.m3.onSurfaceVariant
                                    font: field.font
                                }
                            }
                            IconButton {
                                id: clear
                                visible: field.text !== ""
                                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                width: 40
                                height: 40
                                icon: "x"
                                iconSize: 22
                                iconColor: Theme.m3.onSurfaceVariant
                                tooltip: "Clear search"
                                onClicked: {
                                    field.text = "";
                                    field.forceActiveFocus();
                                }
                            }
                        }

                        // Categories / All apps: a connected button group
                        Segmented {
                            id: views
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            width: 260
                            height: 48
                            options: ["Categories", "All apps"]
                            accessibleName: "View"
                            current: lib.view
                            onSelected: i => {
                                lib.view = i;
                                lib.folder = null;
                                lib.selected = 0;
                                field.forceActiveFocus();
                            }
                        }
                    }

                    // An open folder's name
                    Row {
                        id: folderTitle
                        visible: lib.folder !== null && lib.query === ""
                        anchors { top: header.bottom; topMargin: 20; left: parent.left; leftMargin: 8 }
                        height: visible ? 32 : 0
                        spacing: 12
                        Text {
                            id: folderNameTitle
                            anchors.verticalCenter: parent.verticalCenter
                            text: lib.folder?.name ?? ""
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 24; weight: Font.Normal }
                        }
                        Text {
                            anchors.baseline: folderNameTitle.baseline
                            text: lib.folder ? lib.folder.apps.length + (lib.folder.apps.length === 1 ? " app" : " apps") : ""
                            color: Theme.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 14 }
                        }
                    }

                    GridView {
                        id: grid

                        Momentum {}
                        anchors {
                            top: folderTitle.visible ? folderTitle.bottom : header.bottom
                            topMargin: folderTitle.visible ? 12 : 20
                            left: parent.left
                            right: parent.right
                            rightMargin: scrubber.visible ? scrubber.width + 8 : 0
                            bottom: parent.bottom
                        }
                        readonly property int columns: Math.max(1, Math.floor(width / (lib.showingApps ? 116 : 212)))
                        cellWidth: Math.floor(width / columns)
                        cellHeight: lib.showingApps ? 120 : 244
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        model: lib.showingApps ? lib.visibleApps : lib.folders
                        // (the drawer scrolls itself to the selection, on a spring)
                        highlightFollowsCurrentItem: false
                        onMovementStarted: scrollAnim.stop()

                        // Scroll so cell i shows, or so its row leads
                        function scrollTo(y) {
                            const max = originY + Math.max(0, contentHeight - height);
                            scrollAnim.stop();
                            scrollAnim.to = Math.max(originY, Math.min(max, y));
                            scrollAnim.start();
                        }
                        function reveal(i) {
                            const rowTop = originY + Math.floor(i / columns) * cellHeight;
                            if (rowTop < contentY)
                                scrollTo(rowTop);
                            else if (rowTop + cellHeight > contentY + height)
                                scrollTo(rowTop + cellHeight - height);
                        }
                        function lead(i) {
                            scrollTo(originY + Math.floor(i / columns) * cellHeight);
                        }
                        // (critically damped: a long jump doesn't bounce past its row)
                        SpringAnimation {
                            id: scrollAnim
                            target: grid
                            property: "contentY"
                            spring: Theme.motion.springDefault.spring
                            damping: 0.62
                            epsilon: 0.5
                        }

                        delegate: Item {
                            id: cell
                            required property var modelData
                            required property int index
                            readonly property bool isSelected: index === lib.selected
                            width: grid.cellWidth
                            height: grid.cellHeight

                            // Comes in on a small spring, a little after the one before
                            property real intro: 0
                            property bool introArmed: false
                            Behavior on intro { enabled: cell.introArmed; SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.01 } }
                            Component.onCompleted: {
                                if (!win.entering || index > 40)
                                    return;
                                intro = 1;
                                introArmed = true;
                                introDelay.interval = 16 + Math.min(index, 24) * 14;
                                introDelay.start();
                            }
                            Timer {
                                id: introDelay
                                onTriggered: cell.intro = 0
                            }
                            opacity: Math.max(0, Math.min(1, 1 - cell.intro))
                            transform: Translate { y: cell.intro * 20 }

                            // App: the icon and its name
                            Rectangle {
                                id: appTile
                                visible: lib.showingApps
                                anchors.horizontalCenter: parent.horizontalCenter
                                y: 4
                                width: 108
                                height: 108
                                // (rounder when selected, on the fast spring)
                                property real corner: cell.isSelected ? 36 : 20
                                Behavior on corner { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
                                radius: Math.max(0, corner)
                                color: cell.isSelected ? Theme.m3.secondaryContainer : "transparent"
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                Accessible.role: Accessible.Button
                                Accessible.name: lib.showingApps ? cell.modelData.name : ""
                                Accessible.onPressAction: lib.activate(cell.index)

                                // State layer
                                Rectangle {
                                    anchors.fill: parent
                                    radius: parent.radius
                                    color: cell.isSelected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface
                                    opacity: appMouse.pressed ? 0.1 : appMouse.containsMouse ? 0.08 : 0
                                    Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                                }
                                AppIcon {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: 12
                                    size: 56
                                    source: lib.showingApps ? Apps.iconFor(cell.modelData) : ""
                                    transform: pressFx1.effects
                                    ElasticControl { id: pressFx1; pressed: appMouse.pressed }
                                    // (Material: grows on press: the ElasticControl above)
                                    Behavior on scale { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }
                                }
                                Text {
                                    // (Settings → Dock → App names: off, only icons)
                                    visible: Config.options.dock.appNames ?? true
                                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 12 }
                                    width: parent.width - 12
                                    horizontalAlignment: Text.AlignHCenter
                                    text: lib.showingApps ? cell.modelData.name : ""
                                    elide: Text.ElideRight
                                    color: cell.isSelected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: 13; weight: cell.isSelected ? Font.Medium : Font.Normal }
                                }
                                MouseArea {
                                    id: appMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onEntered: lib.selected = cell.index
                                    // Right-click or press and hold: its menu
                                    onClicked: m => m.button === Qt.RightButton ? win.openMenu(cell.modelData, m, appMouse) : lib.activate(cell.index)
                                    onPressAndHold: m => win.openMenu(cell.modelData, m, appMouse)
                                }
                            }

                            // Folder: a Material container with a 2×2 icon preview
                            Column {
                                visible: !lib.showingApps
                                anchors.horizontalCenter: parent.horizontalCenter
                                y: 4
                                spacing: 12
                                Rectangle {
                                    id: folderCard
                                    readonly property var list: lib.showingApps ? [] : cell.modelData.apps
                                    width: 188
                                    height: 188
                                    // (much rounder when selected, on the fast spring)
                                    property real corner: cell.isSelected ? 64 : 28
                                    Behavior on corner { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.1 } }
                                    radius: Math.max(0, corner)
                                    color: cell.isSelected ? Theme.m3.secondaryContainer : Theme.m3.surfaceContainerHigh
                                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                    transform: pressFx2.effects
                                    ElasticControl { id: pressFx2; pressed: folderMouse.pressed }
                                    // (Material: grows on press: the ElasticControl above)
                                    Behavior on scale { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }
                                    Accessible.role: Accessible.Button
                                    Accessible.name: lib.showingApps ? "" : cell.modelData.name + ", " + cell.modelData.apps.length + " apps"
                                    Accessible.onPressAction: lib.activate(cell.index)

                                    // State layer
                                    Rectangle {
                                        anchors.fill: parent
                                        radius: parent.radius
                                        color: cell.isSelected ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface
                                        opacity: folderMouse.pressed ? 0.1 : folderMouse.containsMouse ? 0.08 : 0
                                        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                                    }
                                    Grid {
                                        anchors.centerIn: parent
                                        columns: 2
                                        spacing: 16
                                        Repeater {
                                            model: folderCard.list.slice(0, 4)
                                            AppIcon {
                                                required property var modelData
                                                size: 60
                                                source: Apps.iconFor(modelData)
                                            }
                                        }
                                    }
                                    MouseArea {
                                        id: folderMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onEntered: lib.selected = cell.index
                                        onClicked: lib.activate(cell.index)
                                    }
                                }
                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 8
                                    Text {
                                        id: folderName
                                        text: lib.showingApps ? "" : cell.modelData.name
                                        color: Theme.m3.onSurface
                                        font { family: Theme.font.family; pixelSize: 16; weight: cell.isSelected ? Font.DemiBold : Font.Medium }
                                    }
                                    Text {
                                        anchors.baseline: folderName.baseline
                                        text: lib.showingApps ? "" : String(cell.modelData.apps.length)
                                        color: Theme.m3.onSurfaceVariant
                                        font { family: Theme.font.family; pixelSize: 14; features: { "tnum": 1 } }
                                    }
                                }
                            }
                        }
                    }

                    // A–Z: the letter scrubber, as in Android's app drawer. Click
                    // or drag along it to jump; the letter in view is marked.
                    Item {
                        id: scrubber
                        visible: win.alphabetical && letters.length > 1 && grid.contentHeight > grid.height
                        anchors { right: parent.right; top: grid.top; bottom: grid.bottom }
                        width: 28

                        // [{ letter, index }] for each letter's first app
                        readonly property var letters: {
                            const out = [];
                            for (let i = 0; i < lib.apps.length; i++) {
                                const c = (lib.apps[i].name ?? "").charAt(0).toUpperCase();
                                const l = /[A-Z]/.test(c) ? c : "#";
                                if (out.length === 0 || out[out.length - 1].letter !== l)
                                    out.push({ letter: l, index: i });
                            }
                            return out;
                        }
                        readonly property real step: letters.length > 0 ? Math.min(24, height / letters.length) : 24
                        // (the letter of the first row in view)
                        readonly property string current: {
                            if (!visible)
                                return "";
                            const first = Math.max(0, Math.floor((grid.contentY - grid.originY) / grid.cellHeight + 0.5)) * grid.columns;
                            let l = "";
                            for (const x of letters)
                                if (x.index <= first)
                                    l = x.letter;
                            return l;
                        }
                        function jump(y) {
                            const k = Math.max(0, Math.min(letters.length - 1, Math.floor((y - column.y) / step)));
                            const x = letters[k];
                            if (!x)
                                return;
                            lib.selected = x.index;
                            grid.lead(x.index);
                        }

                        Column {
                            id: column
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width
                            Repeater {
                                model: scrubber.letters
                                Item {
                                    required property var modelData
                                    readonly property bool marked: modelData.letter === scrubber.current
                                    width: scrubber.width
                                    height: scrubber.step
                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: Math.min(24, parent.height)
                                        height: width
                                        radius: width / 2
                                        color: Theme.m3.primary
                                        opacity: parent.marked ? 1 : 0
                                        scale: parent.marked ? 1 : 0.5
                                        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                                        Behavior on scale { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.005 } }
                                    }
                                    Text {
                                        anchors.centerIn: parent
                                        text: parent.modelData.letter
                                        color: parent.marked ? Theme.m3.onPrimary : Theme.m3.onSurfaceVariant
                                        font { family: Theme.font.family; pixelSize: 11; weight: Font.Medium }
                                    }
                                }
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onPressed: e => scrubber.jump(e.y)
                            onPositionChanged: e => {
                                if (pressed)
                                    scrubber.jump(e.y);
                            }
                        }
                    }

                    // No results
                    Column {
                        anchors.centerIn: grid
                        visible: lib.query !== "" && lib.visibleApps.length === 0
                        spacing: 8
                        MaterialShape {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 80
                            height: 80
                            shape: "cookie9"
                            color: Theme.m3.surfaceContainerHigh
                            Icon {
                                anchors.centerIn: parent
                                name: "magnifying-glass"
                                size: 32
                                color: Theme.m3.onSurfaceVariant
                            }
                        }
                        Item {
                            width: 1
                            height: 8
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "No apps match “" + field.text + "”"
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 22 }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "Check the spelling, or search the web from the search bar."
                            color: Theme.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 14 }
                        }
                    }
                }

                // More (Material 3 Expressive's FAB menu): get apps, change
                // the view, the drawer's settings
                FabMenu {
                    anchors { right: parent.right; bottom: parent.bottom; margins: 24 }
                    icon: "dots-three"
                    label: "More"
                    items: [
                        { text: "Get apps", icon: "storefront", action: () => Panels.openAppStore() },
                        { text: lib.view === 1 ? "Show folders" : "Show A to Z", icon: lib.view === 1 ? "squares-four" : "list", action: () => lib.toggleView() },
                        { text: "Dock and drawer settings", icon: "gear", action: () => Panels.openSettings("menubar", "Dock") }
                    ]
                }
            }
        }
    }
}
