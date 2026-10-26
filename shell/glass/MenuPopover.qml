import QtQuick
import qs.common

// A glass dropdown menu under a menu bar item. `items` entries:
//   { text, action, icon?, shortcut?, enabled?, checked?, keepOpen? }  or  { separator: true }
//   or { header: "Caption" }  or  { tiles: [{ frac, label, action }] } (a row of layout glyphs)
// Hover or arrow keys highlight; click or Enter activates and closes.
Popover {
    id: root

    property var items: []
    property int current: -1

    targetX: Panels.anchorX
    contentWidth: Math.max(220, ...widths.map(w => w))
    padding: 5
    radius: Theme.radius.menu + 2
    readonly property var widths: items.map(i => (i.text ?? "").length * 7.5 + (i.shortcut ? 60 : 0) + 48)

    onShownChanged: current = -1

    function activate(i) {
        const item = items[i];
        if (!item || item.separator || item.enabled === false)
            return;
        if (!item.keepOpen)
            Panels.close();
        item.action?.();
    }

    function step(d) {
        let i = current;
        for (let n = 0; n < items.length; n++) {
            i = (i + d + items.length) % items.length;
            if (!items[i].separator && !items[i].header && !items[i].tiles && items[i].enabled !== false) {
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
        Keys.onEscapePressed: Panels.close()
    }

    Column {
        width: root.contentWidth

        Repeater {
            model: root.items

            Item {
                id: row
                required property var modelData
                required property int index
                readonly property bool enabled_: modelData.enabled !== false
                readonly property bool hot: root.current === index && enabled_
                width: parent.width
                height: modelData.separator ? 11 : modelData.header ? 22 : modelData.tiles ? 36 : 26

                Rectangle {
                    visible: !!row.modelData.separator
                    anchors.centerIn: parent
                    width: parent.width - 18
                    height: 1
                    color: Theme.color.separator
                }

                Text {
                    visible: !!row.modelData.header
                    x: 10
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 3
                    text: row.modelData.header ?? ""
                    color: Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }

                Row {
                    visible: !!row.modelData.tiles
                    x: 6
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4
                    Repeater {
                        model: row.modelData.tiles ?? []
                        Rectangle {
                            id: tile
                            required property var modelData
                            width: 44
                            height: 32
                            radius: Theme.radius.item + 2
                            color: tileMouse.containsMouse ? Theme.accent : "transparent"
                            TileGlyph {
                                anchors.centerIn: parent
                                frac: tile.modelData.frac
                                hot: tileMouse.containsMouse
                            }
                            MouseArea {
                                id: tileMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    Panels.close();
                                    tile.modelData.action();
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    visible: !row.modelData.separator && !row.modelData.header && !row.modelData.tiles
                    anchors.fill: parent
                    radius: Theme.radius.item + 2
                    color: row.hot ? Theme.accent : "transparent"

                    Icon {
                        visible: !!row.modelData.checked
                        x: 6
                        anchors.verticalCenter: parent.verticalCenter
                        name: "check"
                        bold: true
                        size: 12
                        color: row.hot ? Theme.color.onAccent : Theme.color.label
                    }
                    Text {
                        x: 22
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.text ?? ""
                        color: row.hot ? Theme.color.onAccent : row.enabled_ ? Theme.color.label : Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    Text {
                        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                        text: row.modelData.shortcut ?? ""
                        color: row.hot ? Qt.alpha(Theme.color.onAccent, 0.8) : Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: row.enabled_
                        onEntered: root.current = row.index
                        onExited: if (root.current === row.index) root.current = -1
                        onClicked: root.activate(row.index)
                    }
                }
            }
        }
    }
}
