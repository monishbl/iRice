import QtQuick
import qs.common

// A grouped form section: optional caption above a rounded platter whose rows
// are separated by hairlines. Material: an Expressive segmented list, each
// row its own tile 2 px from the next, the group's outer corners round (24)
// and the inner ones nearly square (4); the caption in primary.
// With none of its rows on show (each hidden by its own condition), the
// whole group folds to nothing: no caption, platter or footer, and no gap
// (a Column skips items of zero height).
Column {
    id: root

    property string title
    property string footer
    default property alias rows: body.data
    // (rows' own visibility, read while the platter stays visible: hiding the
    // platter would hide the rows with it, and the group would never return)
    readonly property bool hasRows: {
        const c = body.children;
        for (let i = 0; i < c.length; i++)
            if (c[i].visible && c[i].height > 0)
                return true;
        return false;
    }

    spacing: Theme.material ? 8 : 6

    Text {
        visible: root.title !== "" && root.hasRows
        leftPadding: Theme.material ? 16 : 4
        text: root.title
        color: Theme.material ? Theme.m3.primary : Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : Font.DemiBold; letterSpacing: Theme.material ? 0.1 : 0 }
    }

    Squircle {
        width: root.width
        height: body.implicitHeight
        opacity: root.hasRows ? 1 : 0 // (not visible: false, see hasRows)
        radius: 12
        color: Theme.material ? "transparent" : Theme.dark ? Qt.rgba(1, 1, 1, 0.05) : "white"
        border.width: Theme.material ? 0 : 1
        border.color: Theme.color.separator

        // Material: a tile behind each visible row, under the rows
        Loader {
            anchors.fill: parent
            active: Theme.material
            sourceComponent: Item {
                id: tiles
                // The first and last rows on show take the outer corners
                readonly property int first: {
                    const c = body.children;
                    for (let i = 0; i < c.length; i++)
                        if (c[i].visible && c[i].height > 0)
                            return i;
                    return -1;
                }
                readonly property int last: {
                    const c = body.children;
                    for (let i = c.length - 1; i >= 0; i--)
                        if (c[i].visible && c[i].height > 0)
                            return i;
                    return -1;
                }
                Repeater {
                    model: body.children.length
                    Rectangle {
                        id: tile
                        required property int index
                        readonly property Item row: body.children[index] ?? null
                        // (the pointer is over this row: a hover state layer)
                        readonly property bool lit: row !== null && rowHover.hovered && rowHover.point.position.y >= row.y && rowHover.point.position.y < row.y + row.height
                        visible: row !== null && row.visible && row.height > 0
                        y: row?.y ?? 0
                        width: parent.width
                        height: row?.height ?? 0
                        topLeftRadius: index === tiles.first ? 24 : 4
                        topRightRadius: topLeftRadius
                        bottomLeftRadius: index === tiles.last ? 24 : 4
                        bottomRightRadius: bottomLeftRadius
                        color: lit ? Qt.tint(Theme.m3.surfaceContainerHigh, Qt.alpha(Theme.m3.onSurface, 0.08)) : Theme.m3.surfaceContainerHigh
                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                    }
                }
            }
        }

        Column {
            id: body
            width: parent.width
            spacing: Theme.material ? 2 : 0
            HoverHandler {
                id: rowHover
                enabled: Theme.material
            }
        }

        // Hairlines between visible rows
        Repeater {
            model: Theme.material ? 0 : body.children.length
            Rectangle {
                required property int index
                readonly property var row: body.children[index]
                visible: index > 0 && row.visible && row.height > 0
                x: 14
                y: row.y
                width: parent.width - 28
                height: 1
                color: Theme.color.separator
            }
        }
    }

    Text {
        visible: root.footer !== "" && root.hasRows
        width: root.width
        leftPadding: Theme.material ? 16 : 4
        rightPadding: Theme.material ? 16 : 4
        wrapMode: Text.Wrap
        text: root.footer
        color: Theme.material ? Theme.m3.onSurfaceVariant : Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.material ? 12 : Theme.font.caption; letterSpacing: Theme.material ? 0.4 : 0 }
    }
}
