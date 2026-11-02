import QtQuick
import qs.common
import qs.services

// Focus modes: pick how long, then a mode. The active mode shows when it ends.
DetailPage {
    id: root

    property int duration: 0 // ms; 0 until turned off, -1 until this evening
    title: "Focus"

    Repeater {
        model: Focus.modes
        ListRow {
            required property var modelData
            readonly property bool isActive: Focus.active === modelData.id
            icon: modelData.icon
            title: modelData.name
            highlighted: isActive
            status: isActive ? Focus.statusText : modelData.schedule?.enabled ? "Scheduled " + Focus.fmt(modelData.schedule.from) + "–" + Focus.fmt(modelData.schedule.to) : ""
            trailingIcon: isActive ? "check" : ""
            onClicked: isActive ? Focus.deactivate() : Focus.activate(modelData.id, root.duration, false)
        }
    }

    // How long the next mode stays on
    Item {
        width: parent.width
        height: 12
    }
    Text {
        leftPadding: 6
        text: "Turn on"
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
    }
    Segmented {
        width: parent.width
        options: ["Until off", "For 1 hour", "Until evening"]
        current: root.duration === 3600000 ? 1 : root.duration === -1 ? 2 : 0
        onSelected: i => root.duration = [0, 3600000, -1][i]
    }
    Item {
        width: parent.width
        height: 6
    }
    ListRow {
        icon: "gear-six"
        title: "Focus settings…"
        onClicked: Panels.openSettings("focus")
    }
}
