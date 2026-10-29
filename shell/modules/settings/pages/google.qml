import QtQuick
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22
    property bool advanced: false
    property string selectedAccount: Google.account.accountId || Google.account.goaAccounts?.[0]?.id || ""
    // What the helper is doing, in words (for the footer while it works)
    readonly property string working: ({
            status: "Checking your account…", sync: "Syncing Calendar and Tasks…", connectDesktop: "Connecting your account…",
            connect: "Waiting for you to sign in to Google in your browser…", configure: "Importing the client…",
            disconnect: "Disconnecting…", forget: "Removing local account data…", eventAdd: "Adding the event…",
            taskAdd: "Adding the task…", taskUpdate: "Saving the task…", taskDelete: "Deleting the task…"
        })[Google.operation] ?? "Working…"
    Group {
        width: parent.width
        title: "Google account"
        footer: Google.lastError || (Google.busy ? page.working : Google.connected ? "Connected. Calendar and Tasks refresh every 15 minutes. Last sync: " + (Google.account.syncedAt ? new Date(Google.account.syncedAt).toLocaleString() : "not yet") : "Sign in once through desktop Online Accounts, then select your Google account here. No Cloud project or client file is needed with this method.")
        Row {
            label: "Sign in with Google"
            detail: "Open Online Accounts → Google. Enable Calendars after signing in."
            SettingsButton {
                text: "Open Online Accounts"
                onClicked: {
                    const app = Google.account.goaSettings;
                    if (!app) {
                        Google.lastError = "Install gnome-online-accounts, python-gobject and gnome-control-center (or gnome-online-accounts-gtk), then click Refresh accounts.";
                        return;
                    }
                    Launch.run(app.endsWith("gnome-control-center") ? ["env", "XDG_CURRENT_DESKTOP=GNOME", app, "online-accounts"] : [app]);
                }
            }
        }
        Row {
            label: "Google account"
            Dropdown {
                width: 260
                options: (Google.account.goaAccounts ?? []).map(a => ({ value: a.id, label: a.name }))
                value: page.selectedAccount
                onChosen: v => page.selectedAccount = v
            }
        }
        Row {
            label: "Connect desktop account"
            Flow {
                spacing: 8
                SettingsButton { text: "Refresh accounts"; enabled: !Google.busy; onClicked: Google.run({ cmd: "status" }) }
                SettingsButton { text: "Connect account"; enabled: !Google.busy && !Google.connected && page.selectedAccount !== ""; onClicked: Google.run({ cmd: "connectDesktop", account: page.selectedAccount }) }
            }
        }
        Row {
            label: "Sync & disconnect"
            Flow {
                spacing: 8
                SettingsButton { text: "Sync now"; enabled: !Google.busy && Google.connected; onClicked: Google.run({ cmd: "sync" }) }
                SettingsButton { text: "Disconnect account"; enabled: !Google.busy && Google.connected; onClicked: Google.run({ cmd: "disconnect" }) }
            }
        }
        Row {
            label: "Remove local account data"
            detail: Google.account.backend === "goa" ? "Disconnects iRice only; other apps keep their account access." : "For offline removal. Revoke access later in your Google account."
            SettingsButton { text: "Forget locally"; enabled: !Google.busy && Google.connected; onClicked: Google.run({ cmd: "forget" }) }
        }
    }
    Group {
        width: parent.width
        title: "Advanced connection"
        footer: "Optional: use your own OAuth client instead of desktop Online Accounts."
        Row {
            label: "Use a custom OAuth client"
            Switch { checked: page.advanced; onToggled: page.advanced = !checked }
        }
        Row {
            visible: page.advanced
            label: "Desktop client JSON"
            TextField { id: client; width: 260; placeholder: "~/Downloads/client_secret_….json"; commitOnBlur: false }
        }
        Row {
            visible: page.advanced
            label: "Custom connection"
            Flow {
                spacing: 8
                SettingsButton { text: "Import client"; enabled: !Google.busy && !Google.connected && client.text.trim() !== ""; onClicked: Google.run({ cmd: "configure", path: client.text.trim() }) }
                SettingsButton { text: "Connect account"; enabled: !Google.busy && !Google.connected && Google.configured; onClicked: Google.run({ cmd: "connect" }) }
            }
        }
    }

    Group {
        width: parent.width
        title: "Calendar"
        footer: "Events from visible Google calendars appear in Calendar, Spotlight and Today, covering one year before and after each sync. Google handles recurring events and timezones. Edit existing Google events in Google Calendar."
        Row {
            label: "Add to primary calendar"
            detail: "For example: Lunch tomorrow at 1pm"
            TextField { id: eventText; width: 260; commitOnBlur: false; placeholder: "Event and time" }
        }
        Row {
            label: ""
            Flow {
                spacing: 8
                SettingsButton { text: "Add event"; enabled: Google.connected && !Google.busy && eventText.text.trim() !== ""; onClicked: Google.run({ cmd: "eventAdd", text: eventText.text }) }
                SettingsButton { text: "Open Calendar"; onClicked: Qt.openUrlExternally("https://calendar.google.com/") }
            }
        }
    }
    Group {
        width: parent.width
        title: "Tasks"
        footer: "Google tasks appear alongside local reminders. Tick, rename or delete them in Notification Center; changes show at once and reach Google in the background, and deleting can be undone for a few seconds. Google supports due dates, but not due times. If a task is changed here and on another device at the same time, the later change wins."
        Row {
            label: "New tasks go to"
            Dropdown {
                width: 260
                options: [{ value: "", label: "Local — this computer" }].concat(Google.taskLists.map(l => ({ value: l.id, label: l.title })))
                value: Config.options.google.taskListId
                onChosen: v => Config.options.google.taskListId = v
            }
        }
        Row {
            label: "Manage task lists"
            SettingsButton { text: "Open Tasks"; onClicked: Qt.openUrlExternally("https://tasks.google.com/") }
        }
    }
    Group {
        width: parent.width
        title: "Keep & notes"
        footer: "Google Keep opens in your browser. Google’s official Keep API is for enterprise administration, so personal Keep notes do not sync into iRice. Your local Markdown notes remain available through Quick Note."
        Row {
            label: "Google Keep"
            detail: "Notes, lists and reminders in your Google account"
            SettingsButton { text: "Open Keep"; onClicked: Google.openKeep() }
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
