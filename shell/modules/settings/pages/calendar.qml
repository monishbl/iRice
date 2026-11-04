import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Calendar: which calendars show (from your accounts, and subscriptions to
// iCalendar links), where new to-dos go, and reminder alerts. This is the
// one home of these choices; Accounts signs in and links here.
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    property var removed: null // last removed subscription, for Undo

    // Accounts with calendars or task lists (Google, iCloud, Microsoft 365)
    readonly property bool cloud: CloudAccounts.accounts.length > 0
    readonly property bool microsoft: CloudAccounts.accounts.some(a => a.provider === "microsoft")
    function providerOf(c) {
        const p = CloudAccounts.account(c.account)?.provider;
        return p === "icloud" ? "iCloud" : p === "microsoft" ? "Outlook" : "";
    }

    // ── From your accounts: shown or hidden, one switch per calendar ───
    Group {
        width: parent.width
        visible: Google.connected || page.cloud
        title: "From your accounts"
        footer: "Shown calendars appear in Calendar, Today and Search. Open an event in Notification Center's calendar to change it, or drag it to another day; calendars you can only view stay read-only. Repeating iCloud and Outlook events are changed on your iPhone, at icloud.com or in Outlook."
        Row {
            visible: Google.calendars.length === 0 && CloudAccounts.calendars.length === 0
            label: "No calendars yet"
            detail: Google.busy || CloudAccounts.busy ? "Syncing…" : "They appear after the first sync"
            labelColor: Theme.color.secondary
        }
        Repeater {
            model: Google.connected ? Google.calendars : []
            Row {
                id: gcal
                required property var modelData
                label: modelData.title + (modelData.primary ? " (primary)" : "")
                detail: ["Google", Google.accounts.length > 1 ? Google.identity(modelData.account) : "", modelData.writable ? "" : "View only"].filter(x => x).join(" · ")
                readonly property string ref: modelData.account + "|" + modelData.id
                readonly property bool shown: Google.shown(modelData)
                Switch {
                    checked: gcal.shown
                    onToggled: {
                        // (a bare id is how it was kept before several accounts)
                        const hidden = (Config.options.google.hiddenCalendars ?? []).filter(id => id !== gcal.ref && id !== gcal.modelData.id);
                        Config.options.google.hiddenCalendars = gcal.shown ? hidden.concat([gcal.ref]) : hidden;
                    }
                }
            }
        }
        Repeater {
            model: CloudAccounts.calendars.filter(c => page.providerOf(c) !== "")
            Row {
                id: ccal
                required property var modelData
                readonly property string ref: modelData.account + "|" + modelData.id
                label: modelData.title
                detail: [page.providerOf(modelData), CloudAccounts.accounts.length > 1 ? CloudAccounts.account(modelData.account)?.identity ?? "" : "", modelData.writable ? "" : "View only"].filter(x => x).join(" · ")
                Switch {
                    checked: CloudAccounts.shown(ccal.modelData)
                    onToggled: {
                        const hidden = (Config.options.cloud.hiddenCalendars ?? []).filter(x => x !== ccal.ref);
                        Config.options.cloud.hiddenCalendars = checked ? hidden.concat([ccal.ref]) : hidden;
                    }
                }
            }
        }
    }

    // ── To-dos: where new ones go (Google Tasks or Microsoft To Do) ────
    Group {
        width: parent.width
        visible: Google.connected || page.microsoft
        title: "To-dos"
        footer: "Tasks from your accounts appear in To-do with your local reminders. Changes show at once and reach Google or Microsoft in the background; deleting can be undone for 8 seconds. Google Tasks has due dates but not times. If a task changes on another device before your change reaches Google, Google's version is kept and a notification shows what you had."
        Row {
            label: "New to-dos go to"
            Dropdown {
                width: 260
                options: [{ value: "", label: "This computer only" }].concat(Google.taskLists.map(l => ({ value: l.account + "|" + l.id,
                    label: l.title + (Google.accounts.length > 1 ? " · " + Google.identity(l.account) : "") })))
                    .concat(CloudAccounts.taskLists.map(l => ({ value: "cloud|" + l.account + "|" + l.id, label: l.title + " · To Do" })))
                // (a bare list id is from before several accounts; "cloud|…" is Microsoft To Do)
                value: { const v = Config.options.google.taskListId; if (v.startsWith("cloud|")) return v; const l = Google.find(Google.taskLists, v); return l ? l.account + "|" + l.id : ""; }
                onChosen: v => Config.options.google.taskListId = v
            }
        }
    }

    // ── Subscriptions: iCalendar links ──────────────────────────────────
    Group {
        width: parent.width
        title: "Subscribed calendars"
        footer: Calendar.lastError !== "" ? Calendar.lastError : "Events refresh every 15 minutes. Reminders you add in Notification Center or Search are kept on this computer."
        Row {
            visible: Calendar.sources.length === 0
            label: "No subscriptions yet"
            detail: "Add a link below to see its events in Notification Center and Search."
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
            color: Theme.accentInk
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
            SettingsButton {
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
                text: "Add calendar"
                primary: urlField.text.trim() !== ""
                onClicked: add()
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
