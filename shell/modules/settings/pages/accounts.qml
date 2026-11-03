import QtQuick
import qs.common
import qs.modules.settings

// Accounts: Google, iCloud and Microsoft 365 on one page, each its own part
// (their pages as they were: sign in, calendars, contacts, drives)
Column {
    width: parent?.width ?? 0
    spacing: 22

    PagePart {
        title: "Google"
        source: Qt.resolvedUrl("google.qml")
    }
    PagePart {
        title: "iCloud"
        source: Qt.resolvedUrl("icloud.qml")
    }
    PagePart {
        title: "Microsoft 365"
        source: Qt.resolvedUrl("microsoft.qml")
    }
}
