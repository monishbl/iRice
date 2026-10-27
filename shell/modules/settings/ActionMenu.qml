import QtQuick
import qs.common

// One pop-up menu for a whole Settings page, drawn above everything else on
// it (a menu inside a row would sit under the rows after it). Put it last in
// the page's Column, then call open(anchorItem, items) with
//   [{ text, icon?, danger?, checked?, enabled?, action: function }]
// Keyboard: ↑ ↓ to move, Enter to choose, Esc to close.
Item {
    id: root

    property var items: []
    property Item anchor: null
    property int current: -1
    readonly property bool shown: menu.visible

    width: parent?.width ?? 0
    // (not 0: Qt skips drawing a zero-size item, children and all)
    height: 1
    z: 1000

    function open(anchorItem, list) {
        anchor = anchorItem;
        items = list;
        current = -1;
        const p = anchorItem.mapToItem(root, 0, 0);
        const page = root.parent;
        const below = p.y + anchorItem.height + 4;
        const h = Math.min(320, list.length * 32 + 10);
        // Below the anchor, or above it when there's no room under it
        const roomBelow = page ? page.height - root.y - below : 1e6;
        menu.y = roomBelow < h + 8 && p.y - h - 4 > -root.y ? p.y - h - 4 : below;
        menu.x = Math.max(0, Math.min(root.width - menu.width, p.x + anchorItem.width - menu.width));
        menu.visible = true;
        menu.forceActiveFocus();
    }
    function close() {
        if (!menu.visible)
            return;
        menu.visible = false;
        if (anchor)
            anchor.forceActiveFocus();
    }
    function choose(i) {
        const it = items[i];
        if (!it || it.enabled === false)
            return;
        menu.visible = false;
        if (anchor)
            anchor.forceActiveFocus();
        it.action();
    }

    // Click anywhere else on the page to close
    MouseArea {
        visible: menu.visible
        x: 0
        y: -root.y
        width: root.width
        height: root.parent?.height ?? 0
        onPressed: root.close()
    }

    Rectangle {
        id: menu
        visible: false
        width: Math.max(220, list.implicitWidth + 10)
        // (from the item count: every entry is 32 high, and the list's own
        // height isn't known yet while the menu first appears)
        height: Math.min(320, root.items.length * 32 + 10)
        radius: Theme.radius.menu
        color: Theme.dark ? Qt.rgba(0.16, 0.16, 0.18, 0.99) : Qt.rgba(0.99, 0.99, 1, 0.99)
        border.width: 1
        border.color: Theme.color.separator
        clip: true
        opacity: visible ? 1 : 0
        // (not an OpacityAnimator: it shows in the same frame it fades in, and
        // an Animator started then can stay at 0: an invisible menu that
        // lets clicks through to the settings under it)
        Behavior on opacity { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }

        Keys.onPressed: event => {
            const n = root.items.length;
            if (event.key === Qt.Key_Escape) {
                root.close();
            } else if (event.key === Qt.Key_Down) {
                for (let k = 1; k <= n; k++) {
                    const i = (root.current + k) % n;
                    if (root.items[i].enabled !== false) { root.current = i; break; }
                }
            } else if (event.key === Qt.Key_Up) {
                for (let k = 1; k <= n; k++) {
                    const i = ((root.current < 0 ? 0 : root.current) - k + n * 2) % n;
                    if (root.items[i].enabled !== false) { root.current = i; break; }
                }
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                root.choose(root.current);
            } else
                return;
            event.accepted = true;
        }

        Flickable {
            id: flick
            anchors { fill: parent; margins: 5 }
            contentHeight: list.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: list
                width: flick.width
                Repeater {
                    model: root.items
                    Rectangle {
                        id: entry
                        required property var modelData
                        required property int index
                        readonly property bool usable: modelData.enabled !== false
                        readonly property bool lit: usable && (itemMouse.containsMouse || root.current === index)
                        width: list.width
                        implicitWidth: label.implicitWidth + 44
                        height: 32
                        radius: Theme.radius.item + 2
                        color: lit ? (modelData.danger ? Theme.color.danger : Theme.accent) : "transparent"
                        Icon {
                            x: 8
                            anchors.verticalCenter: parent.verticalCenter
                            visible: entry.modelData.checked === true || (entry.modelData.icon ?? "") !== ""
                            name: entry.modelData.checked === true ? "check" : (entry.modelData.icon ?? "")
                            bold: entry.modelData.checked === true
                            size: 12
                            color: label.color
                        }
                        Text {
                            id: label
                            x: 28
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.modelData.text
                            color: entry.lit ? Theme.color.onAccent : !entry.usable ? Theme.color.tertiary : entry.modelData.danger ? Theme.color.danger : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: entry.usable ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: root.choose(entry.index)
                        }
                    }
                }
            }
        }
    }
}
