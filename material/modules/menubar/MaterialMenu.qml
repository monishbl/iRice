import QtQuick
import qs.common
import qs.glass

// A menu bar menu in the Material look (Glass: MenuPopover, with the same
// `items`): Material 3 Expressive's vertical menu. On the menu's surface
// (surfaceContainer, corners 16, elevation 2's shadow) separators split the
// items into groups in surfaceContainerLow, 2 px apart (corners 12 outside,
// 4 where two groups meet). Rows are 40 px: hovered, a state layer; checked,
// a tick on tertiaryContainer with fully round corners; section headers in
// labelMedium. Hover or arrow keys highlight; click or Enter activates and
// closes.
//   items: [{ text, action, shortcut?, enabled?, checked?, keepOpen? } |
//           { separator: true } | { header } | { tiles: [{ frac, label, action }] }]
Popover {
    id: root
    lens: 0.35
    autoClose: 5000
    padding: 4
    radius: 16

    property var items: []
    // The rows shown: kept when a rebuilt list shows the same thing (see
    // MenuPopover: live menus rebuilt the rows under the pointer)
    property var rows: []
    function rowKey(i) {
        return [i.text, i.checked, i.enabled, i.separator, i.header, i.shortcut, i.icon, i.keepOpen, (i.tiles ?? []).map(t => t.label).join("|")].join("\u0001");
    }
    function sameRows(a, b) {
        if (a.length !== b.length)
            return false;
        for (let k = 0; k < a.length; k++)
            if (rowKey(a[k]) !== rowKey(b[k]) || a[k].entry !== b[k].entry)
                return false;
        return true;
    }
    onItemsChanged: if (!sameRows(rows, items)) rows = items
    Component.onCompleted: rows = items
    property int current: -1

    // The groups: indices into `rows`, split at the separators
    readonly property var groups: {
        const out = [];
        let g = [];
        for (let i = 0; i < rows.length; i++) {
            if (rows[i].separator) {
                if (g.length > 0)
                    out.push(g);
                g = [];
            } else {
                g.push(i);
            }
        }
        if (g.length > 0)
            out.push(g);
        return out;
    }
    readonly property bool anyChecks: rows.some(i => i.checked === true) // (tray menus mark every entry checked or not)

    targetX: Panels.anchorX - 4
    contentWidth: Math.max(232, ...rows.map(i => (i.text ?? "").length * 7.4 + (i.shortcut ? i.shortcut.length * 7 + 28 : 0) + (anyChecks ? 28 : 0) + 40))

    onShownChanged: current = -1

    function activate(i) {
        const item = rows[i];
        if (!item || item.separator || item.enabled === false)
            return;
        if (!item.keepOpen)
            Panels.close();
        item.action?.();
    }

    function step(d) {
        let i = current;
        for (let n = 0; n < rows.length; n++) {
            i = (i + d + rows.length) % rows.length;
            if (!rows[i].separator && !rows[i].header && !rows[i].tiles && rows[i].enabled !== false) {
                current = i;
                return;
            }
        }
    }

    Item {
        focus: root.shown
        Keys.onDownPressed: root.step(1)
        Keys.onUpPressed: root.step(-1)
        Keys.onReturnPressed: root.activate(root.current)
        Keys.onEnterPressed: root.activate(root.current)
        Keys.onSpacePressed: root.activate(root.current)
        Keys.onEscapePressed: Panels.close()
    }

    Column {
        width: root.contentWidth
        spacing: 2

        Repeater {
            model: root.groups

            Rectangle {
                id: group
                required property var modelData
                required property int index
                readonly property bool first: index === 0
                readonly property bool last: index === root.groups.length - 1
                width: parent.width
                height: rowsCol.implicitHeight + 8
                color: Theme.veil(Theme.m3.surfaceContainerLow)
                topLeftRadius: first ? 12 : 4
                topRightRadius: first ? 12 : 4
                bottomLeftRadius: last ? 12 : 4
                bottomRightRadius: last ? 12 : 4

                Column {
                    id: rowsCol
                    x: 4
                    y: 4
                    width: parent.width - 8

                    Repeater {
                        model: group.modelData

                        Item {
                            id: row
                            required property int modelData // (an index into rows)
                            readonly property var entry: root.rows[modelData] ?? ({})
                            readonly property bool enabled_: entry.enabled !== false
                            readonly property bool hot: root.current === modelData && enabled_
                            readonly property bool checkedRow: entry.checked === true
                            width: parent.width
                            height: entry.header ? 32 : 40
                            Accessible.role: entry.header ? Accessible.StaticText : Accessible.MenuItem
                            Accessible.name: entry.text ?? entry.header ?? ""
                            Accessible.checkable: entry.checked !== undefined
                            Accessible.checked: row.checkedRow
                            Accessible.onPressAction: root.activate(modelData)

                            // Section header (labelMedium)
                            Text {
                                visible: !!row.entry.header
                                x: 12
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: 6
                                text: row.entry.header ?? ""
                                color: Theme.m3.onSurfaceVariant
                                font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium; letterSpacing: 0.5 }
                            }

                            // A row of layout glyphs
                            Row {
                                visible: !!row.entry.tiles
                                x: 4
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4
                                Repeater {
                                    model: row.entry.tiles ?? []
                                    Rectangle {
                                        id: tile
                                        required property var modelData
                                        width: 44
                                        height: 32
                                        radius: 8
                                        color: tileMouse.pressed ? Qt.alpha(Theme.m3.onSurface, 0.1) : tileMouse.containsMouse ? Theme.color.highlight : "transparent"
                                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                        TileGlyph {
                                            anchors.centerIn: parent
                                            frac: tile.modelData.frac
                                            hot: tileMouse.containsMouse
                                        }
                                        MouseArea {
                                            id: tileMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                Panels.close();
                                                tile.modelData.action();
                                            }
                                        }
                                        Accessible.role: Accessible.Button
                                        Accessible.name: tile.modelData.label ?? ""
                                    }
                                }
                            }

                            // An item: a state layer while hovered; checked, a
                            // pill on tertiaryContainer with a tick
                            Rectangle {
                                visible: !row.entry.header && !row.entry.tiles
                                anchors.fill: parent
                                radius: row.checkedRow ? height / 2 : 8
                                color: row.checkedRow ? (row.hot ? Qt.tint(Theme.m3.tertiaryContainer, Qt.alpha(Theme.m3.onTertiaryContainer, rowMouse.pressed ? 0.1 : 0.08)) : Theme.m3.tertiaryContainer)
                                    : rowMouse.pressed && row.hot ? Qt.alpha(Theme.m3.onSurface, 0.1) : row.hot ? Theme.color.highlight : "transparent"
                                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                                readonly property color content: !row.enabled_ ? Qt.alpha(Theme.m3.onSurface, 0.38) : row.checkedRow ? Theme.m3.onTertiaryContainer : Theme.m3.onSurface

                                Icon {
                                    visible: row.checkedRow
                                    x: 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: "check"
                                    bold: true
                                    size: 16
                                    color: parent.content
                                }
                                Text {
                                    x: root.anyChecks ? 40 : 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - x - (shortcutText.text !== "" ? shortcutText.implicitWidth + 24 : 12)
                                    elide: Text.ElideRight
                                    text: row.entry.text ?? ""
                                    color: parent.content
                                    font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                                }
                                Text {
                                    id: shortcutText
                                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                                    text: row.entry.shortcut ?? ""
                                    color: !row.enabled_ ? Qt.alpha(Theme.m3.onSurface, 0.38) : row.checkedRow ? Theme.m3.onTertiaryContainer : Theme.m3.onSurfaceVariant
                                    font { family: Theme.font.family; pixelSize: 13 }
                                }
                                MouseArea {
                                    id: rowMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    enabled: row.enabled_
                                    cursorShape: Qt.PointingHandCursor
                                    onEntered: root.current = row.modelData
                                    onExited: if (root.current === row.modelData) root.current = -1
                                    onClicked: root.activate(row.modelData)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
