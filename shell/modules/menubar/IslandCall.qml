import QtQuick
import Quickshell.Widgets
import qs.common
import qs.services

// A ringing call in the open island, like the iPhone's: the caller's photo
// (or initials) with a soft ring pulsing out of it, the name, the app, and
// round Decline (red) and Answer (green) buttons. Answer reads Open for an
// app that can't take the call here (WhatsApp: "check your phone").
// The card itself is Island event { kind: "ringing", call: Calls.describe() };
// what the buttons do is Calls.answer() / Calls.decline().
Item {
    id: root

    property var event: null
    property real t: 1 // the stagger in (IslandEvent's clock)
    property bool still: false // nobody can see it: the pulse holds
    readonly property var call: event?.call ?? null
    readonly property bool reduce: Config.options.appearance.reduceMotion === true
    readonly property color ink: "#F5F5F7"
    readonly property color ink2: Qt.rgba(1, 1, 1, 0.6)

    implicitHeight: 68

    function step(from) {
        return Math.max(0, Math.min(1, (t - from) / 0.45));
    }
    function ease(x) {
        return 1 - Math.pow(1 - x, 3);
    }

    // ── Photo, with the ringing pulse ──────────────────────────────────
    Item {
        id: photo
        width: 52
        height: 52
        x: 4
        anchors.verticalCenter: parent.verticalCenter
        scale: 0.7 + 0.3 * root.ease(root.step(0.1))

        // Two rings swell out of the photo and fade, half a beat apart
        // (1.6 s a beat, linear: it's a loop). Reduce motion: one still ring.
        Ticker {
            id: pulseClock
            running: root.visible && !root.still && !root.reduce
            fps: 30
        }
        Repeater {
            model: root.reduce ? 0 : 2
            Rectangle {
                required property int index
                readonly property real u: ((pulseClock.time / 1.6) + index * 0.5) % 1
                anchors.centerIn: parent
                width: photo.width * (1 + 0.45 * u)
                height: width
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: Theme.color.success
                opacity: 0.55 * (1 - u)
            }
        }
        Rectangle {
            visible: root.reduce
            anchors.centerIn: parent
            width: photo.width + 8
            height: width
            radius: width / 2
            color: "transparent"
            border.width: 2
            border.color: Qt.alpha(Theme.color.success, 0.6)
        }

        ClippingRectangle {
            anchors.fill: parent
            radius: width / 2
            color: "#636366"
            Text {
                anchors.centerIn: parent
                visible: !photoImage.visible
                text: root.call?.initials ?? ""
                color: "white"
                font { family: Theme.font.family; pixelSize: 19; weight: Font.DemiBold }
            }
            // No initials either: a person
            Icon {
                anchors.centerIn: parent
                visible: !photoImage.visible && (root.call?.initials ?? "") === ""
                name: "user"
                fill: true
                size: 24
                color: "white"
            }
            Image {
                id: photoImage
                anchors.fill: parent
                visible: (root.call?.photo ?? "") !== "" && status === Image.Ready
                source: root.call?.photo ?? ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(104, 104)
                asynchronous: true
            }
        }
        // The app's badge
        AppIcon {
            visible: (root.call?.appIcon ?? "") !== ""
            anchors { right: parent.right; bottom: parent.bottom; rightMargin: -4; bottomMargin: -4 }
            size: 22
            source: root.call?.appIcon ?? ""
        }
    }

    // ── Name and app ───────────────────────────────────────────────────
    Column {
        anchors { left: photo.right; leftMargin: 14; right: buttons.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
        spacing: 4
        opacity: root.ease(root.step(0.15))
        transform: Translate { x: root.reduce ? 0 : (1 - root.ease(root.step(0.15))) * 12 }
        Text {
            width: parent.width
            text: root.call?.name ?? ""
            elide: Text.ElideRight
            color: root.ink
            font { family: Theme.font.family; pixelSize: 17; weight: Font.DemiBold }
        }
        Row {
            width: parent.width
            spacing: 6
            AppIcon {
                id: appSmall
                visible: (root.call?.appIcon ?? "") !== ""
                anchors.verticalCenter: parent.verticalCenter
                size: 14
                source: root.call?.appIcon ?? ""
            }
            Text {
                width: parent.width - (appSmall.visible ? appSmall.width + parent.spacing : 0)
                anchors.verticalCenter: parent.verticalCenter
                text: (root.call?.app ?? "Call") + (root.call?.openOnly ? " · on your phone" : " · incoming call")
                elide: Text.ElideRight
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.body }
            }
        }
    }

    // ── Decline, Answer ────────────────────────────────────────────────
    Row {
        id: buttons
        anchors { right: parent.right; rightMargin: 2; verticalCenter: parent.verticalCenter }
        spacing: 16
        opacity: root.ease(root.step(0.3))
        scale: 0.8 + 0.2 * root.ease(root.step(0.3))
        CallButton {
            label: "Decline"
            icon: "phone-disconnect"
            fillColor: Theme.color.danger
            onActivated: Calls.decline()
        }
        CallButton {
            label: root.call?.openOnly ? "Open" : "Answer"
            icon: root.call?.openOnly ? "arrow-square-out" : "phone"
            fillColor: Theme.color.success
            onActivated: Calls.answer()
        }
    }

    component CallButton: Column {
        id: btn
        property string label
        property string icon
        property color fillColor
        signal activated
        spacing: 4
        Rectangle {
            id: circle
            anchors.horizontalCenter: parent.horizontalCenter
            width: 44
            height: 44
            radius: 22
            color: mouse.pressed ? Qt.darker(btn.fillColor, 1.2) : mouse.containsMouse ? Qt.lighter(btn.fillColor, 1.1) : btn.fillColor
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            scale: mouse.pressed ? 0.94 : 1
            Behavior on scale { NumberAnimation { duration: 100; easing.type: Easing.OutCubic } }
            Icon {
                anchors.centerIn: parent
                name: btn.icon
                fill: true
                size: 22
                color: "white"
            }
            MouseArea {
                id: mouse
                anchors { fill: parent; margins: -4 }
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: btn.activated()
            }
            Accessible.role: Accessible.Button
            Accessible.name: btn.label + " call"
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: btn.label
            color: root.ink2
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
        }
    }
}
