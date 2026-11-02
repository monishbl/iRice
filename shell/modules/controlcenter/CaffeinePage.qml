import QtQuick
import qs.common
import qs.services

// Caffeine: keep the screen awake, until turned off or for a while. The
// choice that's running shows a tick and when it ends.
DetailPage {
    id: root

    title: "Caffeine"
    hasSwitch: true
    switchOn: System.keepAwake
    onSwitchToggled: System.setKeepAwake(!System.keepAwake, 0)

    Repeater {
        model: [
            { minutes: 60, title: "For 1 hour" },
            { minutes: 120, title: "For 2 hours" },
            { minutes: 0, title: "Until turned off" }
        ]
        ListRow {
            required property var modelData
            readonly property bool isActive: System.keepAwake && System.keepAwakeMinutes === modelData.minutes
            icon: modelData.minutes > 0 ? "timer" : "coffee"
            title: modelData.title
            highlighted: isActive
            status: isActive ? System.keepAwakeStatus : ""
            trailingIcon: isActive ? "check" : ""
            onClicked: {
                if (isActive)
                    System.setKeepAwake(false, 0);
                else
                    System.setKeepAwake(true, modelData.minutes);
            }
        }
    }

    Item {
        width: parent.width
        height: 6
    }
    Text {
        width: parent.width
        leftPadding: 6
        rightPadding: 6
        wrapMode: Text.Wrap
        text: "The screen won't dim, lock or sleep while you're away. Closing the lid still sleeps."
        color: Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
}
