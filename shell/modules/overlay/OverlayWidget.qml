import QtQuick
import qs.common
import qs.glass

// Frame for an overlay widget: glass panel with a title bar (drag to move),
// pin and click-through toggles, close button, and a resize grip.
Item {
    id: root

    required property string widgetId
    required property string title
    required property string icon
    property bool overlayOpen: false
    property int minWidth: 220
    property int minHeight: 140
    default property alias content: body.data

    readonly property var st: UiState.widget(widgetId)
    readonly property int m: 18
    property bool allowed: true // (gaming widgets: only with Settings' gaming overlays on)
    readonly property bool shownNow: allowed && st.open && (overlayOpen || st.pinned)

    visible: shownNow
    x: st.x - m
    y: st.y - m
    width: st.w + 2 * m
    height: st.h + 2 * m
    opacity: !overlayOpen && st.clickthrough ? 0.55 : 1
    Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
    onXChanged: glass.updatePosition()
    onYChanged: glass.updatePosition()

    function save() {
        UiState.setWidget(widgetId, { x: Math.round(x + m), y: Math.round(y + m), w: Math.round(width - 2 * m), h: Math.round(height - 2 * m) });
    }

    Glass {
        id: glass
        anchors.fill: parent
        radius: 20
        bevel: 16
        shadowMargin: root.m
    }

    // Title bar
    Item {
        id: bar
        x: root.m
        y: root.m
        width: parent.width - 2 * root.m
        height: 34

        MouseArea {
            anchors.fill: parent
            enabled: root.overlayOpen
            cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            drag.target: root
            onReleased: root.save()
        }
        Icon {
            x: 12
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            fill: true
            size: 14
            color: Theme.color.secondary
        }
        Text {
            x: 32
            anchors.verticalCenter: parent.verticalCenter
            text: root.title
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        Row {
            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
            spacing: 2
            visible: root.overlayOpen
            Repeater {
                model: [
                    { icon: "cursor-click", on: root.st.clickthrough, show: root.st.pinned, act: () => UiState.setWidget(root.widgetId, { clickthrough: !root.st.clickthrough }) },
                    { icon: "push-pin", on: root.st.pinned, show: true, act: () => UiState.setWidget(root.widgetId, { pinned: !root.st.pinned }) },
                    { icon: "x", on: false, show: true, act: () => UiState.setWidget(root.widgetId, { open: false }) }
                ]
                Rectangle {
                    required property var modelData
                    visible: modelData.show
                    width: 24
                    height: 24
                    radius: 12
                    color: modelData.on ? Theme.accent : barMouse.containsMouse ? Theme.color.fillHover : "transparent"
                    Icon {
                        anchors.centerIn: parent
                        name: parent.modelData.icon
                        fill: parent.modelData.on
                        bold: !parent.modelData.on
                        size: 12
                        color: parent.modelData.on ? Theme.color.onAccent : Theme.color.secondary
                    }
                    MouseArea { id: barMouse; anchors.fill: parent; hoverEnabled: true; onClicked: parent.modelData.act() }
                }
            }
        }
    }

    Item {
        id: body
        x: root.m + 12
        y: root.m + bar.height
        width: parent.width - 2 * root.m - 24
        height: parent.height - 2 * root.m - bar.height - 12
        clip: true
    }

    // Resize grip
    MouseArea {
        visible: root.overlayOpen
        width: 18
        height: 18
        x: parent.width - root.m - 18
        y: parent.height - root.m - 18
        cursorShape: Qt.SizeFDiagCursor
        property point origin
        property size start
        onPressed: m => {
            origin = mapToItem(null, m.x, m.y);
            start = Qt.size(root.width, root.height);
        }
        onPositionChanged: m => {
            if (!pressed)
                return;
            const p = mapToItem(null, m.x, m.y);
            root.width = Math.max(root.minWidth + 2 * root.m, start.width + p.x - origin.x);
            root.height = Math.max(root.minHeight + 2 * root.m, start.height + p.y - origin.y);
        }
        onReleased: root.save()
        Icon { anchors.centerIn: parent; name: "arrows-out-simple"; size: 10; color: Theme.color.tertiary; rotation: 90 }
    }
}
