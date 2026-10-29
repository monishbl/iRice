import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Notifications
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Glass banners in the top-right corner of the focused screen. Several at once
// collapse into a stack (newest in front, two peeking behind); the "more" pill
// fans them out into a list and back. Only a new banner slides in; the others
// glide to make room. Banners expire after a few seconds (paused while the
// pointer is on them) and stay listed in Notification Center.
Scope {
    id: root

    // The window exists only while banners are showing (plus a moment)
    property bool windowAlive: false
    readonly property bool windowWanted: Notifications.popupModel.count > 0
    onWindowWantedChanged: windowWanted ? (windowAlive = true, windowUnload.stop()) : windowUnload.restart()
    Timer { id: windowUnload; interval: 800; onTriggered: root.windowAlive = false }

    LazyLoader {
        active: root.windowAlive

        GlassWindow {
            id: win

            readonly property int cardWidth: 360
            readonly property int m: 24 // Glass shadow margin
            readonly property int gap: 10
            readonly property int count: Notifications.popupModel.count
            property bool expanded: false
            onCountChanged: if (count <= 1) expanded = false
            onVisibleChanged: if (!visible) expanded = false
            readonly property bool hovered: hover.hovered

            // Card heights by index (delegates report theirs)
            property var heights: []
            function setHeight(i, h) {
                if (heights[i] === h)
                    return;
                const a = heights.slice();
                a[i] = h;
                heights = a;
            }
            function heightAt(i) {
                return heights[i] ?? 90;
            }
            function offsetOf(i) {
                let y = 0;
                for (let j = 0; j < i; j++)
                    y += heightAt(j) + gap;
                return y;
            }
            // Collapsed: older cards peek 8px below the front one
            function stackedY(i) {
                return i === 0 ? 0 : heightAt(0) + 8 * Math.min(i, 2) - heightAt(i);
            }
            readonly property real contentHeight: expanded ? offsetOf(count) - gap : heightAt(0) + 8 * Math.min(count - 1, 2)

            name: "notifications"
            // (visible reads this, not `screen`: the window re-announces its
            // screen when it shows or hides, a binding loop)
            readonly property var shownOn: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            screen: shownOn
            // (not over Notification Center, which lists them anyway, or over
            // Control Center, which opens in the same corner)
            visible: count > 0 && !Panels.isOpen("notificationCenter", shownOn) && !Panels.isOpen("controlCenter", shownOn)
            WlrLayershell.layer: WlrLayer.Overlay
            // Banners never take the keyboard, except while a reply is being
            // written in one
            WlrLayershell.keyboardFocus: Notifications.replying > 0 ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
            anchors { top: true; right: true }
            margins.top: 30 + 6 - m + (Panels.osdVisible ? 62 : 0)
            margins.right: 8 - m
            implicitWidth: cardWidth + 2 * m
            implicitHeight: Math.max(1, contentHeight + 2 * m + (count > 1 ? 36 : 0))

            HoverHandler { id: hover }
            // Input only where the cards and the pill row are: the window's
            // shadow margin overlaps the menu bar (clock, Control Center,
            // tray), and without this it took their clicks while a banner showed
            // (from 8 px up and left: the close button sits over the corner)
            mask: Region {
                x: win.m - 8
                y: win.m - 8
                width: win.cardWidth + 8
                height: win.contentHeight + 8 + (win.count > 1 ? 8 + 26 : 0)
            }

            Item {
                id: stack
                x: win.m
                y: win.m
                width: win.cardWidth
                height: win.contentHeight

                Repeater {
                    model: Notifications.popupModel

                    Item {
                        id: banner
                        required property int nid
                        required property int index
                        readonly property var notification: Notifications.byId[nid] ?? null
                        readonly property bool front: index === 0
                        readonly property bool shown: win.expanded || index < 3

                        width: win.cardWidth
                        height: card.implicitHeight
                        onHeightChanged: win.setHeight(index, height)
                        onIndexChanged: win.setHeight(index, height)
                        z: -index

                        // Position: list when expanded, stack when collapsed (springs)
                        y: win.expanded ? win.offsetOf(index) : win.stackedY(index)
                        Behavior on y { SpringAnimation { spring: 3.5; damping: 0.32; epsilon: 0.2 } }
                        scale: win.expanded || front ? 1 : 1 - 0.05 * Math.min(index, 2)
                        Behavior on scale { SpringAnimation { spring: 3.5; damping: 0.32; epsilon: 0.002 } }
                        transformOrigin: Item.Bottom

                        // Entrance (only for a new banner) and exit
                        property real enter: 0
                        property bool leaving: false
                        Component.onCompleted: {
                            win.setHeight(index, height);
                            enter = 1;
                        }
                        Behavior on enter {
                            NumberAnimation {
                                duration: banner.leaving ? 200 : 420
                                easing.type: banner.leaving ? Easing.InCubic : Easing.OutBack
                                easing.overshoot: 0.9
                            }
                        }
                        opacity: (shown ? (front || win.expanded ? 1 : 0.9 - 0.25 * (index - 1)) : 0) * enter
                        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                        transform: Translate { x: (1 - banner.enter) * win.cardWidth * 0.55 }
                        // The glass is drawn by the compositor from where it is: these
                        // move or fade it without moving the glass item itself (the
                        // fade-in can start after the window's first sync has ended)
                        onEnterChanged: win.glassChanged()
                        onOpacityChanged: win.glassChanged()
                        onYChanged: win.glassChanged()
                        onScaleChanged: win.glassChanged()

                        function leave() {
                            if (leaving)
                                return;
                            leaving = true;
                            enter = 0;
                            leaveTimer.start();
                        }
                        Timer {
                            id: leaveTimer
                            interval: 210
                            onTriggered: Notifications.dismissPopup(banner.notification)
                        }

                        Connections {
                            target: banner.notification
                            function onClosed() { Notifications.dismissPopup(banner.notification); }
                        }

                        Timer {
                            interval: (banner.notification?.expireTimeout ?? 0) > 0 ? banner.notification.expireTimeout : 5000
                            // (held while a reply is being written, or just sent)
                            running: banner.notification !== null && banner.notification.urgency !== NotificationUrgency.Critical && !win.hovered && !win.expanded && !card.replying && !card.sent
                            onTriggered: banner.leave()
                        }

                        Glass {
                            id: glass
                            x: -win.m
                            y: -win.m
                            width: parent.width + 2 * win.m
                            height: parent.height + 2 * win.m
                            radius: 22
                            bevel: 18
                            shadowMargin: win.m
                        }

                        NotificationCard {
                            id: card
                            width: win.cardWidth
                            visible: banner.notification !== null
                            notification: banner.notification
                            // Back cards show only their edge; clicking the stack expands it
                            enabled: banner.front || win.expanded
                            onCloseRequested: banner.leave()
                            // Counted, so the window takes the keyboard while any reply is open
                            property bool counted: false
                            onReplyingChanged: {
                                if (replying !== counted) {
                                    Notifications.replying += replying ? 1 : -1;
                                    counted = replying;
                                }
                            }
                            Component.onDestruction: if (counted) Notifications.replying -= 1
                        }
                        MouseArea {
                            anchors.fill: parent
                            visible: !banner.front && !win.expanded
                            onClicked: win.expanded = true
                        }
                    }
                }
            }

            // Expand / collapse the stack
            Row {
                visible: win.count > 1
                anchors.right: parent.right
                anchors.rightMargin: win.m
                y: win.m + win.contentHeight + 8
                spacing: 6
                Repeater {
                    model: win.expanded ? [{ label: "Clear all", act: () => Notifications.clearAll() }, { label: "Show less", icon: "caret-up", act: () => win.expanded = false }] : [{ label: (win.count - 1) + " more", icon: "caret-down", act: () => win.expanded = true }]
                    Rectangle {
                        required property var modelData
                        width: pillRow.implicitWidth + 20
                        height: 26
                        radius: 13
                        color: pillMouse.pressed ? Theme.color.fillPressed : pillMouse.containsMouse ? Theme.color.fillHover : (Theme.dark ? Qt.rgba(0.14, 0.14, 0.16, 0.9) : Qt.rgba(1, 1, 1, 0.9))
                        border.width: 1
                        border.color: Theme.color.separator
                        Row {
                            id: pillRow
                            anchors.centerIn: parent
                            spacing: 4
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.parent.modelData.label
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                            }
                            Icon {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: !!parent.parent.modelData.icon
                                name: parent.parent.modelData.icon ?? ""
                                bold: true
                                size: 11
                                color: Theme.color.secondary
                            }
                        }
                        MouseArea {
                            id: pillMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: parent.modelData.act()
                        }
                    }
                }
            }
        }
    }
}
