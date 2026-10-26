import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Calendar subscriptions (iCalendar links) and reminder alerts.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property var removed: null // last removed subscription, for Undo

    Group {
        width: parent.width
        title: "Calendars"
        footer: Calendar.lastError !== "" ? Calendar.lastError : "Events refresh every 15 minutes. Reminders you add in Notification Center or Spotlight are kept on this computer."
        Row {
            visible: Calendar.sources.length === 0
            label: "No calendars yet"
            detail: "Add a link below to see your events in Notification Center and Spotlight."
        }
        Repeater {
            model: Calendar.sources
            Row {
                id: srcRow
                required property var modelData
                label: modelData.name
                detail: modelData.url.replace(/^https?:\/\//, "").slice(0, 60) + (modelData.url.length > 68 ? "…" : "")
                Item {
                    width: controls.implicitWidth
                    height: controls.implicitHeight
                    // Plain Row from QtQuick (the settings Row is a form row)
                    Flow {
                        id: controls
                        spacing: 10
                        Item {
                            width: 12
                            height: 28
                            Rectangle {
                                anchors.centerIn: parent
                                width: 12
                                height: 12
                                radius: 6
                                color: srcRow.modelData.color
                            }
                        }
                        Switch {
                            checked: srcRow.modelData.enabled !== false
                            onToggled: Calendar.setSource(srcRow.modelData.url, { enabled: !checked })
                        }
                        IconButton {
                            icon: "trash"
                            onClicked: {
                                page.removed = srcRow.modelData;
                                Calendar.removeSource(srcRow.modelData.url);
                            }
                        }
                    }
                }
            }
        }
    }

    Row {
        visible: page.removed !== null
        label: "Removed “" + (page.removed?.name ?? "") + "”"
        Text {
            text: "Undo"
            color: Theme.accent
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    Calendar.addSource(page.removed.name, page.removed.url);
                    page.removed = null;
                }
            }
        }
        Timer {
            running: page.removed !== null
            interval: 8000
            onTriggered: page.removed = null
        }
    }

    Group {
        width: parent.width
        title: "Add a calendar"
        footer: "Google Calendar: Settings → your calendar → Integrate calendar → Secret address in iCal format. iCloud: share the calendar publicly and copy the link. Outlook: Settings → Shared calendars → Publish → ICS. A local .ics file path works too."
        Row {
            label: "Name"
            TextField {
                id: nameField
                width: 260
                placeholder: "Work"
                commitOnBlur: false
            }
        }
        Row {
            label: "Link"
            detail: addError.text
            TextField {
                id: urlField
                width: 260
                placeholder: "https://… .ics"
                commitOnBlur: false
                onCommitted: addButton.add()
            }
        }
        Row {
            label: ""
            Rectangle {
                id: addButton
                function add() {
                    const url = urlField.text.trim();
                    if (!/^(https?:|webcal:|\/|file:)/.test(url)) {
                        addError.text = "Paste a link that starts with https:// or webcal://, or a file path.";
                        return;
                    }
                    addError.text = "";
                    Calendar.addSource(nameField.text.trim(), url);
                    nameField.text = "";
                    urlField.text = "";
                }
                width: addLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: urlField.text.trim() !== "" ? Theme.accent : Theme.color.fill
                Text {
                    id: addLabel
                    anchors.centerIn: parent
                    text: "Add calendar"
                    color: urlField.text.trim() !== "" ? Theme.color.onAccent : Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: addButton.add() }
            }
        }
        Text { id: addError; visible: false }
    }

    Group {
        width: parent.width
        title: "Alerts"
        Row {
            label: "Remind me before events"
            detail: "For events without their own alert"
            Dropdown {
                options: [{ value: -1, label: "Never" }, { value: 0, label: "At start time" }, { value: 5, label: "5 minutes" }, { value: 10, label: "10 minutes" }, { value: 15, label: "15 minutes" }, { value: 30, label: "30 minutes" }, { value: 60, label: "1 hour" }]
                value: Calendar.defaultAlert
                onChosen: v => {
                    Calendar.defaultAlert = v;
                    Calendar.saveSources();
                }
            }
        }
    }
}
