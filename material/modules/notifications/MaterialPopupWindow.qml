import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Notifications
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// The banners in the Material look (NotificationPopups loads this instead of
// its glass window): Android's heads-up notifications. Each is a
// surfaceContainerHigh card with 24 px corners and elevation 3's shadow that
// springs in from the top right. Several at once stack the same way as the
// glass banners (newest in front, two peeking behind; the count pill fans
// them out and back), on Material's springs. Banners expire after a few
// seconds (held while the pointer is on them or a reply is being written).
GlassWindow {
    id: win

    readonly property int cardWidth: 360
    readonly property int m: 24 // room for the shadow
    readonly property int gap: 8
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
        return heights[i] ?? 96;
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
    // (visible reads this, not `screen`: the window re-announces its screen
    // when it shows or hides, a binding loop)
    readonly property var shownOn: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
    screen: shownOn
    // (not over Notification Center, which lists them anyway, or over Quick
    // Settings, which opens in the same corner)
    visible: count > 0 && !Panels.isOpen("notificationCenter", shownOn) && !Panels.isOpen("controlCenter", shownOn)
    WlrLayershell.layer: WlrLayer.Overlay
    // Banners never take the keyboard, except while a reply is being written
    WlrLayershell.keyboardFocus: Notifications.replying > 0 ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
    anchors { top: true; right: true }
    margins.top: 30 + 8 - m + (Panels.osdVisible ? 62 : 0)
    margins.right: 8 - m
    implicitWidth: cardWidth + 2 * m
    implicitHeight: Math.max(1, contentHeight + 2 * m + (count > 1 ? 48 : 0))

    HoverHandler { id: hover }
    // Input only where the cards and the pill row are: the shadow's margin
    // overlaps the menu bar
    mask: Region {
        x: win.m
        y: win.m
        width: win.cardWidth
        height: win.contentHeight + (win.count > 1 ? 8 + 32 : 0)
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

                // Position: a list when expanded, a stack when collapsed
                y: win.expanded ? win.offsetOf(index) : win.stackedY(index)
                Behavior on y { SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.2 } }
                scale: win.expanded || front ? 1 : 1 - 0.05 * Math.min(index, 2)
                Behavior on scale { SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.002 } }
                transformOrigin: Item.Bottom

                // Entrance (a new banner only): slides in from the top right on
                // the default spatial spring and fades in; exit: quicker, eased in
                property real enter: 0
                property real fade: 0
                property bool leaving: false
                Component.onCompleted: {
                    win.setHeight(index, height);
                    enterAnim.start();
                    fadeIn.start();
                }
                SpringAnimation {
                    id: enterAnim
                    target: banner
                    property: "enter"
                    to: 1
                    spring: Theme.motion.springDefault.spring
                    damping: Theme.motion.springDefault.damping
                    epsilon: 0.002
                }
                NumberAnimation {
                    id: fadeIn
                    target: banner
                    property: "fade"
                    to: 1
                    duration: Theme.motion.popover
                    easing.type: Easing.OutCubic
                }
                ParallelAnimation {
                    id: exitAnim
                    NumberAnimation { target: banner; property: "enter"; to: 0; duration: 160; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.exit }
                    NumberAnimation { target: banner; property: "fade"; to: 0; duration: 160; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.exit }
                }
                opacity: (shown ? (front || win.expanded ? 1 : 0.9 - 0.25 * (index - 1)) : 0) * fade
                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                transform: Translate {
                    x: (1 - banner.enter) * win.cardWidth * 0.4
                    y: (1 - banner.enter) * -16
                }

                function leave() {
                    if (leaving)
                        return;
                    leaving = true;
                    enterAnim.stop();
                    fadeIn.stop();
                    exitAnim.start();
                    leaveTimer.start();
                }
                Timer {
                    id: leaveTimer
                    interval: 170
                    onTriggered: Notifications.dismissPopup(banner.notification)
                }

                Connections {
                    target: banner.notification
                    function onClosed() { Notifications.dismissPopup(banner.notification); }
                }

                // A ringing call stays until answered, dismissed or taken back by its app
                readonly property bool ringing: Notifications.isCall(notification)
                Timer {
                    interval: (banner.notification?.expireTimeout ?? 0) > 0 ? banner.notification.expireTimeout : 5000
                    // (held while a reply is being written, or just sent)
                    running: banner.notification !== null && !banner.ringing && banner.notification.urgency !== NotificationUrgency.Critical && !win.hovered && !win.expanded && !card.replying && !card.sent
                    onTriggered: banner.leave()
                }

                // Elevation 3: a soft shadow under the card (the front one,
                // or every card once fanned out; back cards only peek)
                RectangularShadow {
                    anchors.fill: parent
                    visible: banner.front || win.expanded
                    radius: 24
                    offset.y: 4
                    blur: 16
                    spread: 1
                    color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.42 : 0.18)
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 24
                    color: Theme.m3.surfaceContainerHigh
                }

                MaterialNotificationCard {
                    id: card
                    width: win.cardWidth
                    visible: banner.notification !== null
                    notification: banner.notification
                    // Back cards show only their edge; clicking the stack expands it
                    enabled: banner.front || win.expanded
                    onCloseRequested: banner.leave()
                    // Answered or declined: the call's banner goes (the app may keep it)
                    onActed: if (banner.ringing) banner.leave()
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

    // Expand / collapse the stack: tonal pills under it
    Row {
        visible: win.count > 1
        anchors.right: parent.right
        anchors.rightMargin: win.m
        y: win.m + win.contentHeight + 8
        spacing: 8
        Repeater {
            model: win.expanded ? [{ label: "Clear all", icon: "", act: () => Notifications.clearAll() }, { label: "Show less", icon: "caret-up", act: () => win.expanded = false }] : [{ label: (win.count - 1) + " more", icon: "caret-down", act: () => win.expanded = true }]
            Item {
                id: pill
                required property var modelData
                width: pillRow.implicitWidth + 32
                height: 32
                RectangularShadow {
                    anchors.fill: parent
                    radius: height / 2
                    offset.y: 2
                    blur: 8
                    color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.36 : 0.14)
                }
                Rectangle {
                    anchors.fill: parent
                    transform: pressFx1.effects
                    ElasticControl { id: pressFx1; pressed: pillMouse.pressed }
                    radius: pillMouse.pressed ? 8 : height / 2
                    Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
                    color: pillMouse.pressed ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.1)) : pillMouse.containsMouse ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.08)) : Theme.m3.secondaryContainer
                    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                }
                Row {
                    id: pillRow
                    anchors.centerIn: parent
                    spacing: 4
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: pill.modelData.label
                        color: Theme.m3.onSecondaryContainer
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: pill.modelData.icon !== ""
                        name: pill.modelData.icon
                        size: 18
                        color: Theme.m3.onSecondaryContainer
                    }
                }
                MouseArea {
                    id: pillMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: pill.modelData.act()
                }
            }
        }
    }
}
