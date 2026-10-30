import QtQuick
import qs.common
import qs.services

// Shortcuts: one-click buttons for things you do often (take a screenshot,
// dark mode, lock...). Right-click the widget → Change Buttons to pick them.
// Each Shortcuts widget keeps its own buttons.
Item {
    id: root

    property string size: "s"
    property bool preview: false
    property string widgetId: ""
    readonly property int slots: size === "s" ? 4 : 8
    readonly property var defaults: ["Take Screenshot", "Toggle Dark Mode", "Lock Screen", "Clipboard History", "Record Screen", "Mission Control", "App Library", "System Settings"]
    readonly property var chosen: (UiState.data.shortcutButtons[widgetId] ?? defaults)
    // Short labels for the buttons (the menu keeps the full names)
    readonly property var shortNames: ({ "Take Screenshot": "Screen\u00ADshot", "Toggle Dark Mode": "Dark Mode", "Lock Screen": "Lock", "Clipboard History": "Clip\u00ADboard", "Record Screen": "Record", "System Settings": "Settings", "Copy Text from Screen": "Copy Text", "Ask AI About the Screen": "Ask AI", "On-Screen Keyboard": "Keyboard", "Keyboard Shortcuts": "Shortcuts", "Notification Center": "Notifications", "Check for Updates": "Updates", "Change Wallpaper": "Wallpaper", "Overlay Widgets": "Overlay" })
    readonly property var buttons: chosen.map(t => Search.actions.find(a => a.title === t)).filter(a => a).slice(0, slots)

    // Right-click menu: tick the buttons you want
    function menuItems() {
        const on = new Set(chosen);
        return [{ separator: true }].concat(Search.actions.map(a => ({
            text: (on.has(a.title) ? "✓ " : "    ") + a.title.replace(/…$/, ""),
            action: () => {
                const l = on.has(a.title) ? chosen.filter(t => t !== a.title) : chosen.concat([a.title]);
                UiState.data.shortcutButtons = Object.assign({}, UiState.data.shortcutButtons, { [widgetId]: l });
            }
        })));
    }

    Grid {
        anchors.fill: parent
        anchors.margins: 10 // (drawn edge to edge: the frame adds no padding)
        columns: root.size === "s" ? 2 : 4
        rows: 2
        spacing: 10
        Repeater {
            model: root.buttons
            Squircle {
                id: btn
                required property var modelData
                required property int index
                readonly property var tints: [Theme.accents.blue, Theme.accents.purple, Theme.accents.orange, Theme.accents.green, Theme.accents.red, Theme.accents.pink, Theme.accents.yellow, Theme.accents.graphite]
                width: (parent.width - (parent.columns - 1) * parent.spacing) / parent.columns
                height: (parent.height - parent.spacing) / 2
                radius: 12
                color: mouse.pressed ? Qt.alpha(tints[index % 8], 0.32) : mouse.containsMouse ? Qt.alpha(tints[index % 8], 0.26) : Qt.alpha(tints[index % 8], 0.18)
                scale: mouse.pressed ? 0.95 : 1
                Behavior on scale { ScaleAnimator { duration: 100 } }
                Column {
                    anchors { left: parent.left; leftMargin: 8; right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
                    spacing: 4
                    Icon {
                        name: btn.modelData.glyph
                        fill: true
                        size: 20
                        color: btn.tints[btn.index % 8]
                    }
                    Text {
                        width: parent.width
                        text: root.shortNames[btn.modelData.title] ?? btn.modelData.title.replace(/…$/, "")
                        wrapMode: Text.WordWrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        lineHeight: 0.95
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                    }
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    enabled: !root.preview
                    onClicked: btn.modelData.run()
                }
            }
        }
    }
    Text {
        anchors.centerIn: parent
        visible: root.buttons.length === 0
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: "Right-click to choose buttons"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
