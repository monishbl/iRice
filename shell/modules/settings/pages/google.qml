import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

// Settings → Google: connect once (one button: the browser opens for a new
// account; one already signed in on this computer is used as it is), then
// Calendar and Tasks stay in sync on their own (see Google.qml for when).
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 24
    property bool advanced: false

    readonly property var accounts: Google.account.goaAccounts ?? []
    // More than one Google account in Online Accounts: which one to use
    property string chosen: accounts[0]?.id ?? ""
    readonly property bool canSignIn: Google.account.signin === true

    // "2 min ago", kept current while the page is open
    property real now: Date.now()
    Timer {
        interval: 30000
        repeat: true
        running: page.visible
        onTriggered: page.now = Date.now()
    }
    function ago(t) {
        if (!t)
            return "not yet";
        const m = Math.round((now - t) / 60000);
        return m < 1 ? "just now" : m < 60 ? m + " min ago" : m < 1440 ? Math.round(m / 60) + " h ago" : new Date(t).toLocaleDateString();
    }
    // What the helper is doing, in words
    readonly property string working: ({
            status: "Checking your account…", sync: "Syncing Calendar and Tasks…",
            signin: page.accounts.length > 0 ? "Connecting your account…" : "Sign in to Google in your browser, then come back here…",
            connectDesktop: "Connecting your account…", connect: "Sign in to Google in your browser, then come back here…",
            configure: "Importing the client…", disconnect: "Disconnecting…", forget: "Removing iRice's copy of your account data…",
            eventAdd: "Adding the event…", taskAdd: "Adding the task…", taskUpdate: "Saving the task…", taskDelete: "Deleting the task…"
        })[Google.operation] ?? "Working…"

    Group {
        width: parent.width
        title: "Google account"
        footer: Google.lastError !== "" ? Google.lastError
            : Google.busy ? page.working
            : Google.connected ? "Calendar and Tasks sync when you sign in, unlock, or open Notification Center, and every 15 minutes while you use the computer. Only changes are fetched."
            : page.canSignIn || page.accounts.length > 0 ? "Your password stays with Google: iRice uses the account through GNOME Online Accounts and never sees it."
            : "Connecting from here needs iRice's native helpers (Software Update builds them). Until then, add your Google account in Online Accounts, then come back."

        // Not connected: one button
        Row {
            visible: !Google.connected
            label: "Google account"
            detail: page.accounts.length === 1 ? "Signed in on this computer as " + page.accounts[0].name
                : page.accounts.length > 1 ? "Choose which account to use"
                : "Calendar and Tasks from your Google account, kept in sync"
            SettingsButton {
                visible: page.accounts.length > 0 || page.canSignIn
                primary: true
                text: "Connect Google account"
                busy: Google.busy && ["signin", "connectDesktop"].includes(Google.operation)
                enabled: !Google.busy
                onClicked: page.accounts.length > 1 ? Google.run({ cmd: "connectDesktop", account: page.chosen }) : Google.connect()
            }
            SettingsButton {
                visible: page.accounts.length === 0 && !page.canSignIn
                text: "Open Online Accounts"
                onClicked: {
                    const app = Google.account.goaSettings;
                    if (!app) {
                        Google.lastError = "Install gnome-online-accounts and gnome-control-center (or gnome-online-accounts-gtk), then try again.";
                        return;
                    }
                    Launch.run(app.endsWith("gnome-control-center") ? ["env", "XDG_CURRENT_DESKTOP=GNOME", app, "online-accounts", "google"] : [app]);
                }
            }
        }
        Row {
            visible: !Google.connected && page.accounts.length > 1
            label: "Account"
            Dropdown {
                width: 260
                options: page.accounts.map(a => ({ value: a.id, label: a.name }))
                value: page.chosen
                onChosen: v => page.chosen = v
            }
        }
        Row {
            visible: !Google.connected && page.accounts.length === 0 && page.canSignIn
            label: "Not signed in yet"
            detail: "Your browser opens at Google's sign-in. Approve access to Calendar and Tasks, then come back here."
        }

        // Connected
        Row {
            visible: Google.connected
            label: Google.account.identity || "Google account"
            detail: Google.busy && Google.operation === "sync" ? "Syncing…" : "Last synced " + page.ago(Google.syncedAt)
            SettingsButton {
                text: "Sync now"
                busy: Google.busy && Google.operation === "sync"
                enabled: !Google.busy
                onClicked: Google.run({ cmd: "sync" })
            }
        }
        Row {
            visible: Google.connected
            label: "Disconnect account"
            detail: Google.account.backend === "goa" ? "iRice stops using it; the account stays in Online Accounts for other apps." : "Signs iRice out and revokes its access."
            SettingsButton {
                text: "Disconnect account"
                danger: true
                enabled: !Google.busy
                onClicked: Google.run({ cmd: "disconnect" })
            }
        }
    }

    Group {
        width: parent.width
        visible: Google.connected
        title: "Calendars"
        footer: Google.calendars.length === 0 ? "No calendars yet. They appear after the first sync." : "Shown calendars appear in Calendar, Today and Spotlight. Edit existing Google events in Google Calendar."
        Repeater {
            model: Google.calendars
            Row {
                id: cal
                required property var modelData
                label: modelData.title + (modelData.primary ? " (primary)" : "")
                readonly property bool shown: !(Config.options.google.hiddenCalendars ?? []).includes(modelData.id)
                Switch {
                    checked: cal.shown
                    onToggled: {
                        const hidden = (Config.options.google.hiddenCalendars ?? []).filter(id => id !== cal.modelData.id);
                        Config.options.google.hiddenCalendars = cal.shown ? hidden.concat([cal.modelData.id]) : hidden;
                    }
                }
            }
        }
        Row {
            label: "Add to primary calendar"
            detail: "For example: Lunch tomorrow at 1pm"
            QQ.Row { // (Row here is the settings row)
                spacing: 8
                TextField { id: eventText; width: 200; commitOnBlur: false; placeholder: "Event and time" }
                SettingsButton { text: "Add event"; enabled: !Google.busy && eventText.text.trim() !== ""; onClicked: Google.run({ cmd: "eventAdd", text: eventText.text }) }
            }
        }
    }

    Group {
        width: parent.width
        visible: Google.connected
        title: "Tasks"
        footer: "Google tasks appear with your local reminders. Changes show at once and reach Google in the background; deleting can be undone for 8 seconds. Google Tasks has due dates but not times. If a task changes here and on another device at once, the later change wins."
        Row {
            label: "New tasks go to"
            Dropdown {
                width: 260
                options: [{ value: "", label: "This computer only" }].concat(Google.taskLists.map(l => ({ value: l.id, label: l.title })))
                value: Config.options.google.taskListId
                onChosen: v => Config.options.google.taskListId = v
            }
        }
        Row {
            label: "Task lists"
            SettingsButton { text: "Open Google Tasks"; onClicked: Qt.openUrlExternally("https://tasks.google.com/") }
        }
    }

    Group {
        width: parent.width
        title: "Keep"
        footer: "Google doesn't offer personal Keep notes to other apps, so Keep opens in your browser. Quick Note keeps your own notes on this computer."
        Row {
            label: "Google Keep"
            SettingsButton { text: "Open Google Keep"; onClicked: Google.openKeep() }
        }
    }

    // Advanced: your own OAuth client instead of Online Accounts, and
    // removing iRice's copy without reaching Google
    Group {
        width: parent.width
        title: "Advanced"
        footer: page.advanced ? "A Desktop app OAuth client from your own Google Cloud project, with the Calendar and Tasks APIs enabled. iRice keeps its tokens privately on this computer." : ""
        Row {
            label: "Use your own OAuth client"
            detail: "Instead of GNOME Online Accounts"
            Switch { checked: page.advanced; onToggled: page.advanced = !checked }
        }
        Row {
            visible: page.advanced
            label: "Client file"
            detail: Google.configured ? "Imported" : "The JSON file Google Cloud gives you"
            PathField {
                placeholder: "client_secret_….json"
                filter: "OAuth client|*.json"
                start: "~/Downloads"
                onPicked: path => Google.run({ cmd: "configure", path: path })
            }
        }
        Row {
            visible: page.advanced
            label: "Connect with your client"
            SettingsButton {
                text: "Connect Google account"
                enabled: !Google.busy && !Google.connected && Google.configured
                onClicked: Google.run({ cmd: "connect" })
            }
        }
        Row {
            visible: Google.connected
            label: "Remove iRice's copy"
            detail: "Deletes the synced data on this computer without contacting Google (for when you're offline)"
            SettingsButton { text: "Remove local data"; enabled: !Google.busy; onClicked: Google.run({ cmd: "forget" }) }
        }
    }

    Connections {
        target: Google
        function onMutationComplete() {
            if (Google.operation === "eventAdd")
                eventText.text = "";
        }
    }
}
