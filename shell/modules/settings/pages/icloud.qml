import QtQuick
import QtQuick as QQ
import qs.common
import qs.modules.settings
import qs.services

// Settings → iCloud: your Apple ID and an app-specific password (Apple's way
// for apps that aren't Apple's), then iCloud calendars in Calendar and Today,
// and iCloud contacts in Spotlight (services/CloudAccounts)
Column {
    id: page
    readonly property bool signingHere: CloudAccounts.signingIn && CloudAccounts.signingInto === "icloud"
    width: parent?.width ?? 0
    spacing: 24
    readonly property var icloud: CloudAccounts.accounts.filter(a => a.provider === "icloud")

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
        title: "iCloud account"
        footer: CloudAccounts.lastError !== "" && !CloudAccounts.busy && CloudAccounts.errorFrom !== "microsoft" ? CloudAccounts.lastError
            : page.signingHere ? "Checking your Apple ID…"
            : page.icloud.length > 0 ? "Calendars sync when you sign in, unlock or open Notification Center, and every 15 minutes while you use the computer. Your app-specific password is kept in your keyring."
            : "iCloud needs an app-specific password: Apple's way to let an app outside Apple's in, without your real password. Making one needs two-factor authentication on your Apple ID."

        Repeater {
            model: page.icloud
            Row {
                id: acct
                required property var modelData
                label: modelData.identity
                detail: CloudAccounts.busy ? "Syncing…" : "Last synced " + page.ago(modelData.syncedAt) + " · " + CloudAccounts.contacts.filter(c => c.account === acct.modelData.id).length + " contacts"
                QQ.Row {
                    spacing: 8
                    SettingsButton { text: "Sync now"; enabled: !CloudAccounts.busy; onClicked: CloudAccounts.sync() }
                    SettingsButton { text: "Sign out"; danger: true; enabled: !CloudAccounts.busy; onClicked: CloudAccounts.disconnect(acct.modelData.id) }
                }
            }
        }

        // A keyring keeps the password (the Secret Service)
        Row {
            visible: page.icloud.length === 0 && Needs.checked && (!Needs.secretTool || !Needs.keyring)
            label: "A keyring is needed first"
            detail: "Your app-specific password is kept in a keyring, never in a file. Install GNOME Keyring (KDE's KWallet works too, with its Secret Service on). It asks for your password once."
            SettingsButton {
                text: Needs.installing ? "Installing…" : "Install"
                icon: "download-simple"
                busy: Needs.installing
                enabled: !Updates.running
                onClicked: Needs.install(["gnome-keyring", "libsecret"])
            }
        }
        // Signing in
        Row {
            visible: page.icloud.length === 0
            label: "1. Make an app-specific password"
            detail: "At account.apple.com: Sign-In and Security → App-Specific Passwords → add one named iRice. It looks like abcd-efgh-ijkl-mnop."
            SettingsButton { text: "Open account.apple.com"; icon: "arrow-square-out"; onClicked: Qt.openUrlExternally("https://account.apple.com/account/manage") }
        }
        Row {
            visible: page.icloud.length === 0
            label: "2. Apple ID"
            TextField { id: appleId; width: 260; placeholder: "you@icloud.com"; commitOnBlur: false }
        }
        Row {
            visible: page.icloud.length === 0
            label: "3. App-specific password"
            TextField { id: password; width: 260; placeholder: "abcd-efgh-ijkl-mnop"; secret: true; commitOnBlur: false }
        }
        Row {
            visible: page.icloud.length === 0
            label: "Sign in to iCloud"
            detail: "Calendars and contacts. Your real Apple ID password is never asked for."
            SettingsButton {
                text: page.signingHere ? "Signing in…" : "Sign in"
                primary: true
                busy: page.signingHere
                enabled: !CloudAccounts.busy && appleId.text.trim() !== "" && password.text.trim() !== ""
                onClicked: {
                    CloudAccounts.connectICloud(appleId.text.trim(), password.text);
                    password.text = ""; // (not kept here once it's sent)
                }
            }
        }
    }

    Group {
        width: parent.width
        visible: page.icloud.length > 0
        title: "Calendars"
        footer: CloudAccounts.calendars.length === 0 ? "No calendars yet. They appear after the first sync." : "Shown calendars appear in Calendar, Today and Spotlight. Add an event from Notification Center's calendar and pick an iCloud calendar for it. Repeating events show here but are changed on your iPhone or at icloud.com."
        Repeater {
            model: CloudAccounts.calendars.filter(c => CloudAccounts.account(c.account)?.provider === "icloud")
            Row {
                id: cal
                required property var modelData
                readonly property string ref: modelData.account + "|" + modelData.id
                label: modelData.title
                detail: modelData.writable ? "" : "View only"
                Switch {
                    checked: CloudAccounts.shown(cal.modelData)
                    onToggled: {
                        const hidden = (Config.options.cloud.hiddenCalendars ?? []).filter(x => x !== cal.ref);
                        Config.options.cloud.hiddenCalendars = checked ? hidden.concat([cal.ref]) : hidden;
                    }
                }
            }
        }
    }
}
