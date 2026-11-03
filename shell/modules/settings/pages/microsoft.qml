import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

// Settings → Microsoft 365: an Outlook or work/school account through GNOME
// Online Accounts (its Microsoft sign-in in the browser), then Outlook
// calendars in Calendar and Today and Microsoft To Do in To-do
// (services/CloudAccounts), and OneDrive as a folder (services/CloudDrives)
Column {
    id: page
    readonly property bool signingHere: CloudAccounts.signingIn && CloudAccounts.signingInto === "microsoft"
    width: parent?.width ?? 0
    spacing: 24
    readonly property var microsoft: CloudAccounts.accounts.filter(a => a.provider === "microsoft")

    property real now: Date.now()
    Timer { interval: 30000; repeat: true; running: page.visible; onTriggered: page.now = Date.now() }
    function ago(t) {
        if (!t)
            return "not yet";
        const m = Math.round((now - t) / 60000);
        return m < 1 ? "just now" : m < 60 ? m + " min ago" : m < 1440 ? Math.round(m / 60) + " h ago" : new Date(t).toLocaleDateString();
    }

    Group {
        width: parent.width
        title: "Microsoft account"
        footer: CloudAccounts.lastError !== "" && !CloudAccounts.busy && CloudAccounts.errorFrom !== "icloud" ? CloudAccounts.lastError
            : page.signingHere ? "Sign in to Microsoft in your browser, then come back here…"
            : page.microsoft.length > 0 ? "Outlook calendars and To Do sync when you sign in, unlock or open Notification Center, and every 15 minutes while you use the computer."
            : "Your password stays with Microsoft: iRice uses the account through GNOME Online Accounts and never sees it. Work or school accounts may need their admin's approval."
        Repeater {
            model: page.microsoft
            Row {
                id: acct
                required property var modelData
                label: modelData.identity || "Microsoft account"
                detail: CloudAccounts.busy ? "Syncing…" : "Last synced " + page.ago(modelData.syncedAt)
                QQ.Row {
                    spacing: 8
                    SettingsButton { text: "Sync now"; enabled: !CloudAccounts.busy; onClicked: CloudAccounts.sync() }
                    SettingsButton { text: "Disconnect"; danger: true; enabled: !CloudAccounts.busy; onClicked: CloudAccounts.disconnect(acct.modelData.id) }
                }
            }
        }
        Row {
            visible: page.microsoft.length === 0 && Needs.checked && (!Needs.goa || !Needs.goaMicrosoft)
            label: !Needs.goa ? "GNOME Online Accounts is needed" : "GNOME Online Accounts is too old"
            detail: "Microsoft sign-in goes through GNOME Online Accounts (3.50 or newer), so iRice never sees your password. It asks for your password once."
            SettingsButton {
                text: Needs.installing ? "Installing…" : !Needs.goa ? "Install" : "Update"
                icon: "download-simple"
                busy: Needs.installing
                enabled: !Updates.running
                onClicked: Needs.install(["gnome-online-accounts"])
            }
        }
        Row {
            visible: page.microsoft.length === 0 && Needs.checked && Needs.goa && Needs.goaMicrosoft && !Needs.goaSignin
            label: Needs.building ? "Building iRice's sign-in helper…" : "iRice's sign-in helper isn't built yet"
            detail: Needs.buildError !== "" ? Needs.buildError : "It's built on this computer, with iRice's other native helpers (a minute or two)."
            labelColor: Needs.buildError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton { text: Needs.building ? "Building…" : "Build It"; busy: Needs.building; onClicked: Needs.buildNative() }
        }
        Row {
            visible: page.microsoft.length === 0 && (!Needs.checked || (Needs.goa && Needs.goaMicrosoft && Needs.goaSignin))
            label: "Microsoft 365 account"
            detail: "Outlook calendars and Microsoft To Do, kept in sync. Personal (Outlook.com) and work or school accounts."
            SettingsButton {
                text: page.signingHere ? "Waiting for the browser…" : "Sign in with Microsoft"
                primary: true
                busy: page.signingHere
                enabled: !CloudAccounts.busy
                onClicked: CloudAccounts.connectMicrosoft()
            }
        }
    }

    Group {
        width: parent.width
        visible: page.microsoft.length > 0
        title: "Calendars and To Do"
        footer: "Repeating events are changed in Outlook. Microsoft To Do tasks appear in To-do with your other reminders; deleting can be undone for 8 seconds."
        // (which calendars show and where new to-dos go: set on the Calendar
        // page, each setting's one home)
        SubpageLink {
            icon: "calendar-blank"
            label: "Shown calendars"
            detail: "Choose them on the Calendar page"
            page: "calendar"
            target: "From your accounts"
        }
        SubpageLink {
            icon: "list-checks"
            label: "Where new to-dos go"
            detail: "Choose a list on the Calendar page"
            page: "calendar"
            target: "To-dos"
        }
    }

    // OneDrive as a folder (rclone)
    DriveGroup { width: parent.width; drive: "onedrive" }
}
