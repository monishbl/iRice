import QtQuick
import QtQuick as QQ
import qs.common
import qs.services

// Settings in the Material look: Android's two-pane Settings, as on a Pixel
// tablet. On the left, on surfaceContainer, a pill search bar and the pages,
// each icon in a tonal circle, the open one on a secondaryContainer pill that
// slides there on a spring. On the right, on the surface, the page under a
// large headline, its groups as Expressive segmented lists (Group.qml).
// The window (Settings.qml) keeps the history, search and paging; this is
// only the look, and it hands back its page loader, scroller and search field.
Rectangle {
    id: root

    required property var host
    readonly property var win: host
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

    // Each sidebar group's icons in one tonal colour, turn about: primary,
    // secondary, tertiary (as Android's Settings colours its rows)
    readonly property var groups: {
        const g = [];
        for (const s of win.sections)
            if (!g.includes(s.group))
                g.push(s.group);
        return g;
    }
    readonly property var tones: ["Primary", "Secondary", "Tertiary"]
    function tone(sectionId) {
        const s = win.sections.find(x => x.id === sectionId);
        return tones[Math.max(0, groups.indexOf(s?.group ?? "")) % 3];
    }

    color: Theme.m3.surface

    // ── The list pane ──
    Rectangle {
        id: pane
        width: 300
        height: parent.height
        color: Theme.m3.surfaceContainer

        // Search: Android's pill search bar. Typing lists matching pages and
        // settings in place of the pages; Esc clears it.
        Rectangle {
            id: search
            readonly property bool editing: input.activeFocus
            function release() {
                input.focus = false;
                root.Window.window?.contentItem?.forceActiveFocus();
            }
            function focusInput() {
                input.cursorPosition = input.length;
                input.forceActiveFocus();
            }

            x: 16
            y: 16
            z: 2
            width: parent.width - 32
            height: 52
            radius: height / 2
            color: search.editing ? Theme.m3.surfaceContainerHighest : searchHover.hovered ? Qt.tint(Theme.m3.surfaceContainerHigh, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHigh
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            HoverHandler {
                id: searchHover
                cursorShape: Qt.IBeamCursor
            }
            TapHandler {
                onTapped: if (!input.activeFocus) search.focusInput()
            }

            Icon {
                id: glass
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                name: "magnifying-glass"
                size: 22
                color: Theme.m3.onSurfaceVariant
            }
            TextInput {
                id: input
                anchors { left: glass.right; leftMargin: 12; right: clear.visible ? clear.left : parent.right; rightMargin: clear.visible ? 4 : 20; verticalCenter: parent.verticalCenter }
                color: Theme.m3.onSurface
                selectionColor: Qt.alpha(Theme.m3.primary, 0.3)
                selectedTextColor: Theme.m3.onSurface
                selectByMouse: true
                activeFocusOnTab: true
                clip: true
                font { family: Theme.font.family; pixelSize: 16 }
                Accessible.role: Accessible.EditableText
                Accessible.name: "Search settings"
                onTextChanged: root.win.query = text
                onAccepted: {
                    const r = root.win.results;
                    if (r.length > 0)
                        root.win.go(r[0].page, r[0].section ? "" : r[0].text);
                }
                Keys.onEscapePressed: {
                    text = "";
                    search.release();
                }
                // Down: into the results (or the pages)
                Keys.onDownPressed: {
                    let n = nextItemInFocusChain(true);
                    if (n === clear)
                        n = n.nextItemInFocusChain(true);
                    n?.forceActiveFocus(Qt.TabFocusReason);
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: input.text === ""
                    text: "Search settings"
                    color: Theme.m3.onSurfaceVariant
                    font: input.font
                }
            }
            IconButton {
                id: clear
                visible: input.text !== ""
                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                width: 40
                height: 40
                icon: "x"
                iconSize: 20
                iconColor: Theme.m3.onSurfaceVariant
                tooltip: "Clear search"
                onClicked: {
                    input.text = "";
                    search.focusInput();
                }
            }
        }

        Flickable {
            id: sideScroll
            anchors { left: parent.left; right: parent.right; top: search.bottom; topMargin: 12; bottom: parent.bottom }
            contentHeight: list.implicitHeight + 16
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Momentum {}

            Item {
                id: list
                x: 12
                width: pane.width - 24
                implicitHeight: entries.implicitHeight

                // The open page's pill: it slides from page to page on a spring
                // (its item says where it is)
                Rectangle {
                    id: indicator
                    property Item at: null
                    property bool placed: false
                    visible: at !== null && root.win.query.trim() === ""
                    width: list.width
                    height: 56
                    radius: 28
                    y: at ? at.parent.y + at.y : 0
                    color: Theme.m3.secondaryContainer
                    Behavior on y {
                        enabled: indicator.placed
                        SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping }
                    }
                }

                Column {
                    id: entries
                    width: parent.width
                    spacing: 2

                    // Pages, under their group headings
                    Repeater {
                        model: root.win.query.trim() === "" ? root.win.sections : []
                        Column {
                            id: entry
                            required property var modelData
                            required property int index
                            width: entries.width

                            Text {
                                visible: entry.index === 0 || root.win.sections[entry.index - 1].group !== entry.modelData.group
                                width: parent.width
                                leftPadding: 16
                                topPadding: entry.index === 0 ? 4 : 18
                                bottomPadding: 8
                                text: entry.modelData.group
                                color: Theme.m3.onSurfaceVariant
                                font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium; letterSpacing: 0.5 }
                            }
                            Item {
                                id: item
                                readonly property var modelData: entry.modelData
                                readonly property bool current: root.win.sectionId === modelData.id
                                readonly property string tone: root.tone(modelData.id)
                                width: entries.width
                                height: 56
                                // (the pill moves on a spring when you change page,
                                // and is simply put there when the list is built)
                                onCurrentChanged: {
                                    if (!current)
                                        return;
                                    indicator.placed = true;
                                    indicator.at = item;
                                    if (root.win.sidebarKeys)
                                        forceActiveFocus(Qt.TabFocusReason);
                                }
                                Component.onCompleted: if (current) {
                                    indicator.placed = false;
                                    indicator.at = item;
                                }

                                // Keyboard: the list is one Tab stop (the open
                                // page); Up and Down open the next page
                                activeFocusOnTab: current
                                Keys.onPressed: event => {
                                    const all = root.win.sections;
                                    const i = all.findIndex(x => x.id === modelData.id);
                                    const k = event.key;
                                    const j = k === Qt.Key_Up ? i - 1 : k === Qt.Key_Down ? i + 1 : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? all.length - 1 : -2;
                                    if (j === -2 || j < 0 || j >= all.length)
                                        return;
                                    root.win.sidebarKeys = true;
                                    root.win.go(all[j].id);
                                    Qt.callLater(() => root.win.sidebarKeys = false);
                                    event.accepted = true;
                                }
                                Accessible.role: Accessible.PageTab
                                Accessible.name: modelData.name
                                Accessible.checked: current
                                Accessible.onPressAction: root.win.go(modelData.id)
                                FocusRing {
                                    radius: 32
                                }

                                // State layer
                                Rectangle {
                                    anchors.fill: parent
                                    radius: 28
                                    color: Qt.alpha(item.current ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0)
                                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                }
                                // The icon in its tonal circle; the open page's
                                // turns primary and squares off (Expressive's
                                // selected shape)
                                Rectangle {
                                    id: tile
                                    x: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 40
                                    height: 40
                                    radius: item.current ? 14 : 20
                                    color: item.current ? Theme.m3.primary : Theme.m3[item.tone.toLowerCase() + "Container"]
                                    Behavior on radius { SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping } }
                                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                    Icon {
                                        anchors.centerIn: parent
                                        name: item.modelData.icon
                                        fill: item.current
                                        size: 22
                                        color: item.current ? Theme.m3.onPrimary : Theme.m3["on" + item.tone + "Container"]
                                    }
                                }
                                Text {
                                    anchors { left: tile.right; leftMargin: 16; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                    text: item.modelData.name
                                    elide: Text.ElideRight
                                    color: item.current ? Theme.m3.onSecondaryContainer : Theme.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: 16; weight: item.current ? Font.DemiBold : Font.Normal }
                                }
                                MouseArea {
                                    id: mouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: root.win.go(item.modelData.id)
                                }
                            }
                        }
                    }

                    // Search results: pages first, then the settings in them
                    Repeater {
                        model: root.win.results
                        Item {
                            id: result
                            required property var modelData
                            readonly property var section: root.win.sections.find(s => s.id === modelData.page.split("/")[0]) ?? null
                            readonly property string tone: root.tone(section?.id ?? "")
                            width: entries.width
                            height: Math.max(56, texts.implicitHeight + 16)
                            activeFocusOnTab: true
                            Keys.onPressed: event => {
                                const k = event.key;
                                if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) {
                                    root.win.go(modelData.page, modelData.section ? "" : modelData.text);
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
                            Accessible.onPressAction: root.win.go(modelData.page, modelData.section ? "" : modelData.text)
                            FocusRing {
                                radius: 32
                            }

                            Rectangle {
                                anchors.fill: parent
                                radius: Math.min(28, height / 2)
                                color: Qt.alpha(Theme.m3.onSurface, resultMouse.pressed ? 0.1 : resultMouse.containsMouse || result.activeFocus ? 0.08 : 0)
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                            }
                            Rectangle {
                                id: resultTile
                                x: 8
                                anchors.verticalCenter: parent.verticalCenter
                                width: 40
                                height: 40
                                radius: 20
                                color: Theme.m3[result.tone.toLowerCase() + "Container"]
                                Icon {
                                    anchors.centerIn: parent
                                    name: result.section?.icon ?? "gear"
                                    size: 22
                                    color: Theme.m3["on" + result.tone + "Container"]
                                }
                            }
                            Column {
                                id: texts
                                anchors { left: resultTile.right; leftMargin: 16; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                                spacing: 0
                                Text {
                                    width: parent.width
                                    text: result.modelData.text
                                    elide: Text.ElideRight
                                    color: Theme.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: 16 }
                                }
                                Text {
                                    width: parent.width
                                    visible: !result.modelData.section
                                    text: result.section?.name ?? ""
                                    elide: Text.ElideRight
                                    color: Theme.m3.onSurfaceVariant
                                    font { family: Theme.font.family; pixelSize: 14 }
                                }
                            }
                            MouseArea {
                                id: resultMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.win.go(result.modelData.page, result.modelData.section ? "" : result.modelData.text)
                            }
                        }
                    }

                    // No results
                    Column {
                        visible: root.win.query.trim() !== "" && root.win.results.length === 0
                        width: entries.width
                        topPadding: 32
                        spacing: 8
                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: "magnifying-glass"
                            size: 32
                            color: Theme.m3.onSurfaceVariant
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            text: "No results"
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.Wrap
                            text: "Try a shorter word, like \"dock\" or \"sound\"."
                            color: Theme.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 14 }
                        }
                    }
                }
            }
        }
        ScrollIndicator {
            target: sideScroll
            anchors { right: sideScroll.right; top: sideScroll.top; bottom: sideScroll.bottom; rightMargin: 2 }
        }
    }

    // ── The page pane ──
    Item {
        id: detail
        anchors { left: pane.right; right: parent.right; top: parent.top; bottom: parent.bottom }

        readonly property string title: root.win.isSub ? (pageLoader.item?.pageTitle ?? "") : (root.win.section?.name ?? "")
        // The headline has scrolled under the top bar: the bar takes the title
        readonly property bool collapsed: scroller.contentY > headline.y + headline.height - 8

        // Top app bar: back and forward, and the title once the headline is gone
        Rectangle {
            id: toolbar
            anchors { left: parent.left; right: parent.right; top: parent.top }
            height: 64
            z: 2
            color: scroller.contentY > 2 ? Theme.m3.surfaceContainer : Theme.m3.surface
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

            QQ.Row {
                id: arrows
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                IconButton {
                    width: 40
                    height: 40
                    icon: "caret-left"
                    iconSize: 22
                    iconColor: Theme.m3.onSurfaceVariant
                    tooltip: "Back"
                    enabled: root.win.back.length > 0
                    opacity: enabled ? 1 : 0.38
                    onClicked: root.win.goBack()
                }
                IconButton {
                    width: 40
                    height: 40
                    icon: "caret-right"
                    iconSize: 22
                    iconColor: Theme.m3.onSurfaceVariant
                    tooltip: "Forward"
                    enabled: root.win.forward.length > 0
                    opacity: enabled ? 1 : 0.38
                    onClicked: root.win.goForward()
                }
            }
            Text {
                anchors { left: arrows.right; leftMargin: 16; right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
                text: detail.title
                elide: Text.ElideRight
                color: Theme.m3.onSurface
                font { family: Theme.font.family; pixelSize: 22 }
                opacity: detail.collapsed ? 1 : 0
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
                width: Math.min(880, scroller.width - 64)
                x: Math.round((scroller.width - width) / 2)
                y: 4

                // A subpage names its section above the headline; the
                // section's name goes back up to it
                Text {
                    id: crumb
                    visible: root.win.isSub
                    bottomPadding: 4
                    text: root.win.section?.name ?? ""
                    color: crumbMouse.containsMouse ? Theme.m3.onSurface : Theme.m3.primary
                    font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; letterSpacing: 0.1 }
                    Accessible.role: Accessible.Link
                    Accessible.name: text
                    Accessible.onPressAction: root.win.go(root.win.sectionId)
                    MouseArea {
                        id: crumbMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.win.go(root.win.sectionId)
                    }
                }
                // Headline (large, emphasized), like Android's collapsing
                // app bar: it scrolls up into the top bar
                Text {
                    id: headline
                    width: parent.width
                    bottomPadding: 24
                    text: detail.title
                    wrapMode: Text.Wrap
                    color: Theme.m3.onSurface
                    font { family: Theme.font.family; pixelSize: 32; weight: Font.Medium }
                    Accessible.role: Accessible.Heading
                    Accessible.name: text
                }

                Loader {
                    id: pageLoader
                    width: parent.width
                    source: Qt.resolvedUrl("pages/" + Panels.settingsPage.replace("/", "-") + ".qml")
                    onLoaded: {
                        scroller.contentY = 0;
                        Qt.callLater(root.win.reveal);
                    }
                }
            }

            // Where a search result landed: a primary tint that fades, as
            // Android marks the setting a search opened
            Rectangle {
                id: flash
                radius: 16
                color: Qt.alpha(Theme.m3.primary, 0.16)
                border.width: 2
                border.color: Theme.m3.primary
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
