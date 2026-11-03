import QtQuick
import QtQuick as QQ
import qs.common
import qs.services

// Settings in the Glass look: a translucent sidebar of sections on the left
// (the compositor blurs what's behind it), with the search on top, and the
// selected pane on the right under a toolbar of back, forward and where you
// are. The window (Settings.qml) keeps the history, search and paging; this
// is only the look, and it hands back its page loader, scroller and search
// field.
Item {
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
    Rectangle {
        id: sidebar
        width: 230
        height: parent.height
        color: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.78) : Qt.rgba(0.94, 0.94, 0.96, 0.8)

        // Search, sticky at the top. Expanded at the top of the list or
        // while in use; scrolled down and idle it folds into a magnifier
        // (hover or click opens it again)
        readonly property bool searchFolded: sideScroll.contentY > 4 && !search.editing && root.win.query === ""
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
                onTextChanged: root.win.query = text
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
                onCommitted: if (root.win.results.length > 0) root.win.go(root.win.results[0].page, root.win.results[0].section ? "" : root.win.results[0].text)
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
                    model: root.win.query.trim() === "" ? root.win.sections : []
                    Column {
                        id: entry
                        required property var modelData
                        required property int index
                        width: list.width
                        // A group heading above the first section of each group
                        Text {
                            visible: entry.index === 0 || root.win.sections[entry.index - 1].group !== entry.modelData.group
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
                        readonly property bool current: root.win.sectionId === modelData.id
                        width: list.width
                        height: 32
                        radius: 8
                        color: current ? Theme.accent : mouse.containsMouse ? Theme.color.fill : "transparent"
                        // Keyboard: the sidebar is one Tab stop (the open
                        // section); Up and Down open the next section
                        activeFocusOnTab: current
                        onCurrentChanged: if (current && root.win.sidebarKeys) forceActiveFocus(Qt.TabFocusReason)
                        Keys.onPressed: event => {
                            const i = root.win.sections.findIndex(x => x.id === modelData.id);
                            const k = event.key;
                            const j = k === Qt.Key_Up ? i - 1 : k === Qt.Key_Down ? i + 1 : k === Qt.Key_Home ? 0 : k === Qt.Key_End ? root.win.sections.length - 1 : -2;
                            if (j === -2 || j < 0 || j >= root.win.sections.length)
                                return;
                            root.win.sidebarKeys = true;
                            root.win.go(root.win.sections[j].id);
                            Qt.callLater(() => root.win.sidebarKeys = false);
                            event.accepted = true;
                        }
                        Accessible.role: Accessible.PageTab
                        Accessible.name: modelData.name
                        Accessible.checked: current
                        Accessible.onPressAction: root.win.go(modelData.id)
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
                            onClicked: root.win.go(item.modelData.id)
                        }
                    }
                    }
                }

                Repeater {
                    model: root.win.results
                    Squircle {
                        id: result
                        required property var modelData
                        readonly property var section: root.win.sections.find(s => s.id === modelData.page.split("/")[0])
                        width: list.width
                        height: Math.max(36, texts.implicitHeight + 12)
                        radius: 8
                        color: resultMouse.containsMouse ? Theme.color.fill : "transparent"
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
                            onClicked: root.win.go(result.modelData.page, result.modelData.section ? "" : result.modelData.text)
                        }
                    }
                }

                // No results
                Column {
                    visible: root.win.query.trim() !== "" && root.win.results.length === 0
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
                    enabled: root.win.back.length > 0
                    opacity: enabled ? 1 : 0.35
                    onClicked: root.win.goBack()
                }
                IconButton {
                    icon: "caret-right"
                    iconSize: 16
                    tooltip: "Forward"
                    enabled: root.win.forward.length > 0
                    opacity: enabled ? 1 : 0.35
                    onClicked: root.win.goForward()
                }
            }
            QQ.Row {
                anchors { left: arrows.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 6
                Text {
                    id: crumb
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.win.section?.name ?? ""
                    color: root.win.isSub ? (crumbMouse.containsMouse ? Theme.color.label : Theme.color.secondary) : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    MouseArea {
                        id: crumbMouse
                        anchors.fill: parent
                        enabled: root.win.isSub
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.win.go(root.win.sectionId)
                    }
                }
                Icon {
                    visible: root.win.isSub
                    anchors.verticalCenter: parent.verticalCenter
                    name: "caret-right"
                    bold: true
                    size: 12
                    color: Theme.color.tertiary
                }
                Text {
                    visible: root.win.isSub
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
                        Qt.callLater(root.win.reveal);
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
