import QtQuick
import Quickshell
import qs.common
import qs.services
import "NotificationGroups.js" as NotificationGroups

// Notification Center in the Material look: Android 16's notification shade
// in the right-hand panel. The time and date large on top; the
// notifications as one Expressive segmented list (cards 2 px apart, the
// block's ends round, the joins tight), grouped by app like the Glass list
// (a stack shows its newest with a count to expand it); swipe a card
// sideways to dismiss it. Under the list Manage and Clear all, then the
// Calendar, To-do and Focus widgets behind a connected button group.
Column {
    id: root

    spacing: 16
    // Notifications, or island alerts in the iRice group
    readonly property bool anything: Notifications.list.length > 0 || Island.history.length > 0

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // ── The time and date ──────────────────────────────────────────────
    Item {
        width: parent.width
        height: time.implicitHeight + date.implicitHeight
        Text {
            id: time
            x: 12
            text: Qt.formatTime(clock.date, TimeFormat.hmShort)
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: 40; weight: Font.Medium; letterSpacing: -0.4; features: { "tnum": 1 } }
        }
        Text {
            id: date
            x: 12
            anchors.top: time.bottom
            text: Qt.formatDate(clock.date, "dddd, d MMMM")
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
        }
    }

    // ── Nothing here: a calm shape with the bell ───────────────────────
    Column {
        visible: !root.anything
        width: parent.width
        topPadding: 8
        bottomPadding: 8
        spacing: 12
        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 80
            height: 80
            MaterialShape {
                anchors.fill: parent
                shape: "cookie9"
                color: Theme.m3.surfaceContainerHigh
            }
            Icon {
                anchors.centerIn: parent
                name: "bell-simple"
                size: 32
                color: Theme.m3.onSurfaceVariant
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "No notifications"
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.Medium }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: Focus.on
            text: (Focus.mode?.name ?? "Focus") + " is on"
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14 }
        }
    }

    // ── The notifications ──────────────────────────────────────────────
    Flickable {
        id: flick

        Momentum {}
        visible: root.anything
        width: parent.width
        height: Math.min(list.implicitHeight, 440)
        contentHeight: list.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: list
            width: parent.width
            property var expandedApps: ({})
            function toggle(key) {
                const e = Object.assign({}, expandedApps);
                if (e[key])
                    delete e[key];
                else
                    e[key] = true;
                expandedApps = e;
            }

            // Grouped by app, the island's alerts one more group (as in the
            // Glass list)
            readonly property var groups: NotificationGroups.byApp(Notifications.list, Notifications.received, Island.history)
            // The list's segments: a collapsed group is one card (its newest,
            // with the count); an open one is a header and every card. Open
            // groups are blocks of their own, set apart; everything else joins
            // into one block.
            readonly property var entries: {
                const out = [];
                let block = 0, lastOpen = false;
                for (const g of groups) {
                    const key = (g.island ? "island:" : "") + g.app;
                    const kind = g.island ? "alert" : "note";
                    const open = g.items.length > 1 && !!expandedApps[key];
                    if (open) {
                        block++;
                        out.push({ kind: "header", group: g, key: key, block: block });
                        for (const it of g.items)
                            out.push({ kind: kind, item: it, group: g, key: key, count: 1, open: true, block: block });
                    } else {
                        if (lastOpen || out.length === 0)
                            block++;
                        out.push({ kind: kind, item: g.items[0], group: g, key: key, count: g.items.length, open: false, block: block });
                    }
                    lastOpen = open;
                }
                for (let i = 0; i < out.length; i++) {
                    out[i].first = i === 0 || out[i - 1].block !== out[i].block;
                    out[i].last = i === out.length - 1 || out[i + 1].block !== out[i].block;
                }
                return out;
            }
            function dismiss(e) {
                if (e.kind === "alert") {
                    if (e.count > 1)
                        Island.clearHistory();
                    else
                        Island.forget(e.item.hid);
                } else if (e.kind === "note") {
                    if (e.count > 1)
                        for (const n of e.group.items.slice())
                            n.dismiss();
                    else
                        e.item.dismiss();
                }
            }

            Repeater {
                model: list.entries
                Item {
                    id: entry
                    required property var modelData
                    required property int index
                    readonly property real gap: index === 0 ? 0 : modelData.first ? 12 : 2
                    width: list.width
                    height: gap + seg.height

                    Rectangle {
                        id: seg
                        y: entry.gap
                        width: parent.width
                        height: entry.modelData.kind === "header" ? 52 : card.height
                        color: Theme.m3.surfaceContainerHigh
                        // The block's ends round, its joins tight; a card being
                        // swiped comes loose and rounds all over, as on Android
                        property real topR: swipe.active || dx !== 0 || entry.modelData.first ? 24 : 6
                        property real bottomR: swipe.active || dx !== 0 || entry.modelData.last ? 24 : 6
                        Behavior on topR { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                        Behavior on bottomR { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                        topLeftRadius: topR
                        topRightRadius: topR
                        bottomLeftRadius: bottomR
                        bottomRightRadius: bottomR

                        // Swipe sideways to dismiss: past a third of the way (or
                        // flung) it slides off, otherwise it springs back
                        property real dx: 0
                        property real vx: 0
                        transform: Translate { x: seg.dx }
                        opacity: 1 - 0.6 * Math.min(1, Math.abs(dx) / Math.max(1, width))
                        DragHandler {
                            id: swipe
                            enabled: entry.modelData.kind !== "header"
                            target: null
                            yAxis.enabled: false
                            onActiveTranslationChanged: if (active) {
                                seg.dx = activeTranslation.x;
                                seg.vx = centroid.velocity.x;
                            }
                            onActiveChanged: {
                                // The panel holds still while something is dragged in it
                                Panels.holdPull += active ? 1 : -1;
                                if (active) {
                                    back.stop();
                                    return;
                                }
                                if (Math.abs(seg.dx) > seg.width / 3 || (Math.abs(seg.vx) > 900 && Math.sign(seg.vx) === Math.sign(seg.dx))) {
                                    fly.to = (seg.dx > 0 ? 1 : -1) * (seg.width + 24);
                                    fly.start();
                                } else {
                                    back.start();
                                }
                            }
                        }
                        Component.onDestruction: if (swipe.active) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
                        SpringAnimation {
                            id: back
                            target: seg
                            property: "dx"
                            to: 0
                            spring: Theme.motion.springDefault.spring
                            damping: Theme.motion.springDefault.damping
                            epsilon: 0.5
                        }
                        NumberAnimation {
                            id: fly
                            target: seg
                            property: "dx"
                            duration: Theme.motion.hover
                            easing.type: Easing.InCubic
                            onFinished: list.dismiss(entry.modelData)
                        }

                        // An open group's header: its name, Clear, and the chevron
                        Loader {
                            anchors.fill: parent
                            active: entry.modelData.kind === "header"
                            sourceComponent: Item {
                                Text {
                                    anchors { left: parent.left; leftMargin: 16; right: headerButtons.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                    text: entry.modelData.group.app
                                    elide: Text.ElideRight
                                    color: Theme.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                                }
                                Row {
                                    id: headerButtons
                                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                                    spacing: 4
                                    IconButton {
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 32
                                        height: 32
                                        icon: "x"
                                        iconSize: 18
                                        iconColor: Theme.m3.onSurfaceVariant
                                        accessibleName: entry.modelData.group.island ? "Clear iRice alerts" : "Clear " + entry.modelData.group.app
                                        onClicked: {
                                            if (entry.modelData.group.island)
                                                Island.clearHistory();
                                            else
                                                for (const n of entry.modelData.group.items.slice())
                                                    n.dismiss();
                                        }
                                    }
                                    MaterialExpander {
                                        anchors.verticalCenter: parent.verticalCenter
                                        count: entry.modelData.group.items.length
                                        open: true
                                        onClicked: list.toggle(entry.modelData.key)
                                    }
                                }
                                MouseArea {
                                    anchors { left: parent.left; right: headerButtons.left; top: parent.top; bottom: parent.bottom }
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: list.toggle(entry.modelData.key)
                                }
                            }
                        }

                        // A notification, or an island alert
                        Loader {
                            id: card
                            width: parent.width
                            active: entry.modelData.kind !== "header"
                            sourceComponent: entry.modelData.kind === "alert" ? alertCard : noteCard
                            Component {
                                id: noteCard
                                MaterialNotificationCard {
                                    width: card.width
                                    notification: entry.modelData.item
                                    groupCount: entry.modelData.count
                                    tapExpands: entry.modelData.count > 1
                                    onGroupToggled: list.toggle(entry.modelData.key)
                                    onCloseRequested: entry.modelData.item.dismiss()
                                }
                            }
                            Component {
                                id: alertCard
                                MaterialAlertCard {
                                    width: card.width
                                    alert: entry.modelData.item
                                    groupCount: entry.modelData.count
                                    tapExpands: entry.modelData.count > 1
                                    onGroupToggled: list.toggle(entry.modelData.key)
                                    onCloseRequested: Island.forget(entry.modelData.item.hid)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Manage and Clear all ───────────────────────────────────────────
    Item {
        width: parent.width
        height: 40
        // Manage: a text button, to Settings → Notifications
        Rectangle {
            id: manage
            anchors.verticalCenter: parent.verticalCenter
            width: manageLabel.implicitWidth + 24
            height: 40
            transform: pressFx1.effects
            ElasticControl { id: pressFx1; pressed: manageMouse.pressed }
            radius: manageMouse.pressed ? 12 : 20
            Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
            color: manageMouse.pressed ? Qt.alpha(Theme.m3.primary, 0.1) : manageMouse.containsMouse ? Qt.alpha(Theme.m3.primary, 0.08) : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            activeFocusOnTab: true
            Keys.onReturnPressed: Panels.openSettings("notifications")
            Keys.onSpacePressed: Panels.openSettings("notifications")
            Accessible.role: Accessible.Button
            Accessible.name: "Manage notifications"
            FocusRing { radius: manage.radius + 5 }
            Text {
                id: manageLabel
                anchors.centerIn: parent
                text: "Manage"
                color: Theme.m3.primary
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
            MouseArea {
                id: manageMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Panels.openSettings("notifications")
            }
        }
        // Clear all: a tonal pill
        Rectangle {
            id: clearAll
            visible: root.anything
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            width: clearLabel.implicitWidth + 40
            height: 40
            transform: pressFx2.effects
            ElasticControl { id: pressFx2; pressed: clearMouse.pressed }
            radius: clearMouse.pressed ? 12 : 20
            Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
            color: clearMouse.pressed ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.1)) : clearMouse.containsMouse ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.08)) : Theme.m3.secondaryContainer
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            activeFocusOnTab: visible
            function act() {
                Notifications.clearAll();
                Island.clearHistory();
            }
            Keys.onReturnPressed: act()
            Keys.onSpacePressed: act()
            Accessible.role: Accessible.Button
            Accessible.name: "Clear all notifications"
            FocusRing { radius: clearAll.radius + 5 }
            Text {
                id: clearLabel
                anchors.centerIn: parent
                text: "Clear all"
                color: Theme.m3.onSecondaryContainer
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
            MouseArea {
                id: clearMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: clearAll.act()
            }
        }
    }

    // ── Calendar, To-do and Focus: one card, a connected button group on top
    Rectangle {
        width: parent.width
        height: tabs.y + tabs.height + 4 + widgetStack.height
        radius: 24
        color: Theme.m3.surfaceContainerHigh
        Segmented {
            id: tabs
            x: 12
            y: 12
            width: parent.width - 24
            accessibleName: "Widgets"
            options: ["Calendar", "To-do", "Focus"]
            current: Panels.ncTab
            onSelected: i => Panels.ncTab = i
        }
        Item {
            id: widgetStack
            y: tabs.y + tabs.height + 4
            width: parent.width
            height: Panels.ncTab === 0 ? calendarTab.height : Panels.ncTab === 1 ? todoTab.height : focusTab.height
            CalendarWidget {
                id: calendarTab
                width: parent.width
                visible: Panels.ncTab === 0
                color: "transparent"
            }
            TodoWidget {
                id: todoTab
                width: parent.width
                visible: Panels.ncTab === 1
                color: "transparent"
            }
            FocusWidget {
                id: focusTab
                width: parent.width
                visible: Panels.ncTab === 2
                color: "transparent"
            }
        }
    }
}
