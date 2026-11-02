import QtQuick
import Quickshell.Services.UPower
import qs.common
import qs.services

// Smart Stack: several widgets in one place. Scroll or swipe up and down to
// flip through them, or click a dot on the side (they show where you are). Smart Rotate
// brings forward what matters now: Now Playing while music plays, the
// calendar shortly before an event, the battery when it's low. Flipping by
// hand pauses that for 15 minutes. Right-click → Edit Stack picks the widgets.
Item {
    id: root

    property string size: "m"
    property bool preview: false
    property string widgetId: ""
    WidgetDefs { id: defs }
    readonly property var defaults: ["weather", "calendar", "reminders", "nowPlaying", "batteries"]
    // The widgets in this stack that come in its size
    readonly property var kinds: (UiState.data.stacks[widgetId] ?? defaults).filter(k => k !== "stack" && (defs.kind(k)?.sizes ?? []).includes(size))
    property int current: 0
    readonly property int shown: Math.min(current, Math.max(0, kinds.length - 1))
    property real manualAt: 0

    function go(i, byHand) {
        if (kinds.length === 0)
            return;
        const n = (i + kinds.length) % kinds.length;
        if (n === shown)
            return;
        dir = n > shown ? 1 : -1;
        current = n;
        if (byHand)
            manualAt = Date.now();
    }
    property int dir: 1

    // ── Smart Rotate ────────────────────────────────────────────────────
    readonly property string relevant: {
        if (Island.musicPlaying && kinds.includes("nowPlaying"))
            return "nowPlaying";
        const next = Calendar.upcoming(1)[0];
        if (next && !next.allDay && next.start - Date.now() < 30 * 60000 && kinds.includes("calendar"))
            return "calendar";
        const d = UPower.displayDevice;
        const level = d ? (d.percentage > 1 ? d.percentage / 100 : d.percentage) : 1;
        if (UPower.onBattery && level < 0.2 && kinds.includes("batteries"))
            return "batteries";
        return "";
    }
    function rotate() {
        if (!Config.options.desktop.smartRotate || Date.now() - manualAt < 15 * 60000 || relevant === "")
            return;
        go(kinds.indexOf(relevant), false);
    }
    onRelevantChanged: rotate()
    Timer {
        interval: 5 * 60000
        repeat: true
        running: !root.preview
        onTriggered: root.rotate()
    }

    // Right-click menu
    function menuItems() {
        const on = new Set(UiState.data.stacks[widgetId] ?? defaults);
        const choices = defs.kinds.filter(k => k.id !== "stack" && k.id !== "notes" && k.sizes.includes(size));
        const child = page.item?.menuItems?.() ?? [];
        return child.concat([{ separator: true }]).concat(choices.map(k => ({
            text: (on.has(k.id) ? "✓ " : "    ") + k.label,
            action: () => {
                const l = on.has(k.id) ? [...on].filter(x => x !== k.id) : [...on, k.id];
                UiState.data.stacks = Object.assign({}, UiState.data.stacks, { [widgetId]: l });
            }
        }))).concat([{ separator: true }, {
            text: (Config.options.desktop.smartRotate ? "✓ " : "    ") + "Smart Rotate",
            action: () => Config.options.desktop.smartRotate = !Config.options.desktop.smartRotate
        }]);
    }

    // The widget on top; the next slides in from below (or above)
    Item {
        id: viewport
        anchors.fill: parent
        anchors.rightMargin: root.kinds.length > 1 ? 10 : 0
        clip: true
        Loader {
            id: page
            width: parent.width
            height: parent.height
            readonly property string kind: root.kinds[root.shown] ?? ""
            source: kind ? Qt.resolvedUrl(defs.kind(kind)?.file ?? "") : ""
            onLoaded: {
                item.size = Qt.binding(() => root.size);
                if (item.preview !== undefined)
                    item.preview = root.preview;
                if (item.widgetId !== undefined)
                    item.widgetId = root.widgetId + "-" + kind;
                slide.restart();
            }
            SequentialAnimation {
                id: slide
                ParallelAnimation {
                    NumberAnimation { target: page; property: "y"; from: 24 * root.dir; to: 0; duration: 300; easing.type: Easing.OutCubic }
                    NumberAnimation { target: page; property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
                }
            }
        }
    }
    Text {
        anchors.centerIn: parent
        visible: root.kinds.length === 0
        text: "Right-click to add widgets"
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    // Page dots: click one to jump to that widget (each has a wider hit area
    // than the 5px dot)
    Column {
        visible: root.kinds.length > 1
        anchors { right: parent.right; rightMargin: -12; verticalCenter: parent.verticalCenter }
        Repeater {
            model: root.kinds.length
            Item {
                id: dotSlot
                required property int index
                width: 16
                height: dot.height + 4
                Rectangle {
                    id: dot
                    anchors.centerIn: parent
                    width: 5
                    height: dotSlot.index === root.shown ? 12 : 5
                    radius: 2.5
                    color: dotSlot.index === root.shown || dotMouse.containsMouse ? Theme.color.label : Theme.color.tertiary
                    Behavior on height { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                }
                MouseArea {
                    id: dotMouse
                    anchors.fill: parent
                    enabled: !root.preview
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.go(dotSlot.index, true)
                }
            }
        }
    }

    // Scroll or swipe to flip
    WheelHandler {
        enabled: !root.preview && root.kinds.length > 1
        property real acc: 0
        onWheel: e => {
            acc += e.angleDelta.y;
            if (Math.abs(acc) >= 120) {
                root.go(root.shown + (acc < 0 ? 1 : -1), true);
                acc = 0;
            }
        }
    }
}
