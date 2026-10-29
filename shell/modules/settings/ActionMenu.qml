import QtQuick
import QtQuick.Controls as QQC
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
    // The keyboard's choice stays in view
    onCurrentChanged: if (current >= 0) {
        const top = current * 32, bottom = top + 32;
        if (top < flick.contentY)
            flick.contentY = top;
        else if (bottom > flick.contentY + flick.height)
            flick.contentY = bottom - flick.height;
    }
    readonly property bool shown: menu.visible

    width: parent?.width ?? 0
    // (not 0: Qt skips drawing a zero-size item, children and all)
    height: 1
    z: 1000

    function open(anchorItem, list) {
        anchor = anchorItem;
        items = list;
        current = -1;
        flick.contentY = Math.max(0, Math.min(list.findIndex(it => it.checked === true) * 32 - 120, list.length * 32 - 310));
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

    QQC.Popup {
        id: menu
        width: Math.max(220, list.implicitWidth + 10)
        // (from the item count: every entry is 32 high, and the list's own
        // height isn't known yet while the menu first appears)
        height: Math.min(320, root.items.length * 32 + 10)
        padding: 0
        closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutside
        background: GlassPopupBackground { radius: Theme.radius.menu }

        contentItem: Item {
            focus: true
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
                clip: true // long lists scroll inside the menu, not past it
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
                            Accessible.role: Accessible.MenuItem
                            Accessible.name: modelData.text ?? ""
                            Accessible.checkable: modelData.checked !== undefined
                            Accessible.checked: modelData.checked === true
                            Accessible.onPressAction: root.choose(index)
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
            ScrollIndicator {
                target: flick
                anchors { right: parent.right; top: parent.top; bottom: parent.bottom; topMargin: 6; bottomMargin: 6; rightMargin: 1 }
            }
        }
    }
}
