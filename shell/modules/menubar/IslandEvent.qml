import QtQuick
import QtQuick.Shapes
import qs.common
import qs.services

// One event in the expanded (black) island. Two layouts:
//  alert  an animated glyph, a title and detail that slide in one after the
//         other, and on the right a battery ring or an action ("Open")
//  card   like iOS: a line of large text, then a glass card holding an
//         avatar, a name, a time and a message (notifications)
// Volume and brightness use a slim HUD row.
Item {
    id: root

    property var event: null
    property int delay: 0 // ms before it appears (stacked alerts stagger in)
    readonly property string kind: event?.kind ?? ""
    readonly property bool isCard: !!event?.card
    readonly property bool isHud: kind === "hud"
    signal acted

    readonly property color ink: "#F5F5F7"
    readonly property color ink2: Qt.rgba(1, 1, 1, 0.6)

    // More buttons than the two beside the title (a notification's own,
    // added to the alert): further rows under it
    readonly property var more: event?.more ?? []
    implicitHeight: isHud ? 34 : isCard ? cardLayout.implicitHeight : 52 + (more.length > 0 ? moreRow.implicitHeight + 8 : 0)

    // Appear after `delay`, sliding down into place; then the text staggers.
    // A new event (a different uid) replays it; an update in place doesn't.
    property real t: 0
    property real appear: 0
    property int playedUid: -1
    onEventChanged: {
        if (!event || event.uid === playedUid)
            return;
        playedUid = event.uid ?? -1;
        t = 0;
        appear = 0;
        intro.restart();
    }
    SequentialAnimation {
        id: intro
        PauseAnimation { duration: root.delay }
        ScriptAction { script: glyph.play() }
        ParallelAnimation {
            NumberAnimation { target: root; property: "appear"; to: 1; duration: 320; easing.type: Easing.OutCubic }
            NumberAnimation { target: root; property: "t"; from: 0; to: 1; duration: 700 }
        }
    }
    opacity: appear
    transform: Translate { y: (1 - root.appear) * -10 }
    // 0..1 progress of an element that starts at `from` (fraction of the stagger)
    function step(from) {
        return Math.max(0, Math.min(1, (t - from) / 0.45));
    }
    function ease(x) {
        return 1 - Math.pow(1 - x, 3);
    }

    // ── HUD ─────────────────────────────────────────────────────────────
    Item {
        visible: root.isHud
        anchors.fill: parent
        Icon {
            id: hudIcon
            anchors.verticalCenter: parent.verticalCenter
            x: 6
            name: root.event?.icon ?? ""
            fill: true
            size: 18
            color: root.ink
        }
        Rectangle {
            anchors { left: hudIcon.right; leftMargin: 12; right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
            height: 8
            radius: 4
            color: Qt.rgba(1, 1, 1, 0.16)
            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, root.event?.value ?? 0))
                height: parent.height
                radius: 4
                color: root.ink
                Behavior on width { SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.5 } }
            }
        }
    }

    // ── Alert ───────────────────────────────────────────────────────────
    Flow {
        id: moreRow
        visible: !root.isHud && !root.isCard && root.more.length > 0
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: 4; rightMargin: 4 }
        layoutDirection: Qt.RightToLeft // under the buttons beside the title
        spacing: 6
        opacity: root.ease(root.step(0.45))
        Repeater {
            model: root.more
            Rectangle {
                required property var modelData
                width: moreText.implicitWidth + 24
                height: 28
                radius: 14
                color: moreMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : moreMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                Text {
                    id: moreText
                    anchors.centerIn: parent
                    text: parent.modelData.text
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    id: moreMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        parent.modelData.act?.();
                        root.acted();
                    }
                }
            }
        }
    }
    Item {
        visible: !root.isHud && !root.isCard
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 52

        IslandGlyph {
            id: glyph
            anchors.verticalCenter: parent.verticalCenter
            event: !root.isHud && !root.isCard ? root.event : null
        }
        Column {
            anchors { left: glyph.right; leftMargin: root.kind === "audio" ? 26 : 14; right: trailing.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                width: parent.width
                text: root.event?.title ?? ""
                elide: Text.ElideRight
                color: root.ink
                font { family: Theme.font.family; pixelSize: 15; weight: Font.DemiBold }
                opacity: root.ease(root.step(0.1))
                transform: Translate { x: (1 - root.ease(root.step(0.1))) * 14 }
            }
            Text {
                width: parent.width
                visible: text !== ""
                text: root.event?.subtitle ?? ""
                elide: Text.ElideRight
                color: root.event?.subtitleColor ?? root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                opacity: root.ease(root.step(0.25))
                transform: Translate { x: (1 - root.ease(root.step(0.25))) * 14 }
            }
        }

        // Trailing: a device battery ring and/or an action (and a second one,
        // e.g. Decline before Accept); the ring sits before the buttons
        Item {
            id: trailing
            readonly property real pills: root.event?.action ? actionPill.width + (root.event?.action2 ? secondPill.width + 6 : 0) : 0
            anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
            width: (root.event?.battery !== undefined ? 34 + (pills > 0 ? 8 : 0) : 0) + pills
            height: 34
            opacity: root.ease(root.step(0.4))
            scale: 0.7 + 0.3 * root.ease(root.step(0.4))

            Shape {
                id: batRing
                width: 34
                height: 34
                visible: root.event?.battery !== undefined
                preferredRendererType: Shape.CurveRenderer
                readonly property real level: Math.max(0, Math.min(1, root.event?.battery ?? 0))
                readonly property color col: level <= 0.2 ? Theme.color.danger : Theme.color.success
                ShapePath {
                    strokeColor: Qt.rgba(1, 1, 1, 0.16)
                    strokeWidth: 3
                    fillColor: "transparent"
                    PathAngleArc { centerX: 17; centerY: 17; radiusX: 15; radiusY: 15; startAngle: -90; sweepAngle: 360 }
                }
                ShapePath {
                    strokeColor: batRing.col
                    strokeWidth: 3
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathAngleArc { centerX: 17; centerY: 17; radiusX: 15; radiusY: 15; startAngle: -90; sweepAngle: 360 * batRing.level * root.ease(root.step(0.4)) }
                }
                Text {
                    anchors.centerIn: parent
                    text: Math.round(batRing.level * 100)
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: 11; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
            }

            Rectangle {
                id: secondPill
                visible: !!root.event?.action2
                anchors.verticalCenter: parent.verticalCenter
                width: secondText.implicitWidth + 24
                height: 28
                radius: 14
                color: secondMouse.pressed ? Qt.rgba(1, 1, 1, 0.16) : secondMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.2)
                Text {
                    id: secondText
                    anchors.centerIn: parent
                    text: root.event?.action2 ?? ""
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                MouseArea {
                    id: secondMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.event?.onAction2?.();
                        root.acted();
                    }
                }
            }
            Rectangle {
                id: actionPill
                visible: !!root.event?.action
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: actionText.implicitWidth + 24
                height: 28
                radius: 14
                color: actionMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : actionMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                Text {
                    id: actionText
                    anchors.centerIn: parent
                    text: root.event?.action ?? ""
                    color: root.ink
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea {
                    id: actionMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.event?.onAction?.();
                        root.acted();
                    }
                }
            }
        }
    }

    // ── Card (iOS style) ────────────────────────────────────────────────
    Column {
        id: cardLayout
        visible: root.isCard
        width: parent.width
        spacing: 12

        Text {
            width: parent.width
            visible: text !== ""
            text: root.event?.headline ?? ""
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: root.ink
            font { family: Theme.font.family; pixelSize: 18; weight: Font.Medium }
            lineHeight: 1.1
            opacity: root.ease(root.step(0.05))
            transform: Translate { y: (1 - root.ease(root.step(0.05))) * 8 }
        }

        // The glass card: lighter than the island, with a soft rim
        Rectangle {
            id: card
            width: parent.width
            height: cardRow.implicitHeight + 24
            radius: 20
            color: Qt.rgba(1, 1, 1, 0.14)
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.22)
            opacity: root.ease(root.step(0.2))
            transform: Translate { y: (1 - root.ease(root.step(0.2))) * 18 }
            // Top highlight, like light catching the glass edge
            Rectangle {
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 1 }
                height: parent.height / 2
                radius: parent.radius - 1
                gradient: Gradient {
                    GradientStop { position: 0; color: Qt.rgba(1, 1, 1, 0.08) }
                    GradientStop { position: 1; color: Qt.rgba(1, 1, 1, 0) }
                }
            }

            Item {
                id: cardRow
                x: 12
                y: 12
                width: parent.width - 24
                implicitHeight: Math.max(avatar.height, texts.implicitHeight)

                // Avatar: a picture, or initials on grey, with the app's badge
                Item {
                    id: avatar
                    width: 46
                    height: 46
                    scale: 0.6 + 0.4 * root.ease(root.step(0.3))
                    Rectangle {
                        anchors.fill: parent
                        radius: width / 2
                        color: "#8E8E93"
                        clip: true
                        Text {
                            anchors.centerIn: parent
                            visible: !root.event?.card?.avatar
                            text: root.event?.card?.initials ?? ""
                            color: "white"
                            font { family: Theme.font.family; pixelSize: 17; weight: Font.DemiBold }
                        }
                        Image {
                            anchors.fill: parent
                            visible: !!root.event?.card?.avatar
                            source: root.event?.card?.avatar ?? ""
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(92, 92)
                            asynchronous: true
                        }
                    }
                    AppIcon {
                        visible: (root.event?.card?.appIcon ?? "") !== ""
                        anchors { right: parent.right; bottom: parent.bottom; rightMargin: -3; bottomMargin: -3 }
                        size: 20
                        source: root.event?.card?.appIcon ?? ""
                    }
                }

                Column {
                    id: texts
                    anchors { left: avatar.right; leftMargin: 12; right: parent.right }
                    spacing: 2
                    Item {
                        width: parent.width
                        height: nameText.implicitHeight
                        Text {
                            id: nameText
                            width: parent.width - timeText.implicitWidth - 8
                            text: root.event?.card?.name ?? ""
                            elide: Text.ElideRight
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: 15; weight: Font.DemiBold }
                        }
                        Text {
                            id: timeText
                            anchors.right: parent.right
                            text: root.event?.card?.time ?? ""
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                    Text {
                        width: parent.width
                        text: root.event?.card?.text ?? ""
                        wrapMode: Text.Wrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        color: Qt.rgba(1, 1, 1, 0.85)
                        font { family: Theme.font.family; pixelSize: 14 }
                        opacity: root.ease(root.step(0.35))
                    }
                    // The notification's own actions (Reply, OK…), up to two
                    Row {
                        visible: buttons.count > 0
                        topPadding: 6
                        spacing: 6
                        opacity: root.ease(root.step(0.45))
                        Repeater {
                            id: buttons
                            model: (root.event?.card?.buttons ?? []).slice(0, 2)
                            Rectangle {
                                required property var modelData
                                width: buttonText.implicitWidth + 24
                                height: 28
                                radius: 14
                                color: buttonMouse.pressed ? Qt.rgba(1, 1, 1, 0.3) : buttonMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.24) : Qt.rgba(1, 1, 1, 0.16)
                                Text {
                                    id: buttonText
                                    anchors.centerIn: parent
                                    text: parent.modelData.text
                                    color: root.ink
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: buttonMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        parent.modelData.invoke?.();
                                        root.acted();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // The whole card opens it (under the buttons, so they get their clicks)
            MouseArea {
                z: -1
                anchors.fill: parent
                cursorShape: root.event?.onAction ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: {
                    root.event?.onAction?.();
                    root.acted();
                }
            }
        }
    }
}
