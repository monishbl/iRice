pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Notifications
import qs.common

// Notification daemon. Everything received is kept for Notification Center
// until dismissed; new ones also show as banners unless Do Not Disturb is on
// (critical ones always show).
Singleton {
    id: root

    readonly property var list: server.trackedNotifications.values
    property var popups: [] // banners showing, newest first
    // The same banners as a stable model ({ nid }), so adding one doesn't
    // rebuild (and re-animate) the others; byId maps nid -> notification
    property alias popupModel: popupList
    ListModel { id: popupList }
    property var byId: ({})

    function toIsland(n) {
        const words = (n.summary || n.appName || "").split(/\s+/).filter(w => w.length > 0 && w[0].toUpperCase() !== w[0].toLowerCase() || /^[0-9]/.test(w));
        const def = (n.actions ?? []).find(a => a.identifier === "default");
        Island.post({
            kind: "notification",
            card: {
                name: n.summary || n.appName,
                time: "now",
                text: (n.body ?? "").replace(/<[^>]*>/g, "").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">"),
                initials: words.slice(0, 2).map(w => w[0].toUpperCase()).join(""),
                avatar: n.image || "",
                appIcon: n.appIcon || Apps.iconFor(n.desktopEntry || n.appName || "")
            },
            onAction: () => def ? def.invoke() : Panels.show("notificationCenter")
        }, 5000);
    }

    function addPopup(n) {
        const b = Object.assign({}, byId);
        b[n.id] = n;
        byId = b;
        // An update to a banner that's showing (same id) changes it in place
        if (popups.some(p => p.id === n.id)) {
            popups = popups.map(p => p.id === n.id ? n : p);
            return;
        }
        popups = [n].concat(popups);
        popupList.insert(0, { nid: n.id });
    }
    property var received: ({}) // notification id -> Date
    property int unread: 0 // arrived since Notification Center was last opened

    Connections {
        target: Panels
        function onOpenChanged() {
            if (Panels.open === "notificationCenter")
                root.unread = 0;
        }
    }

    function dismissPopup(n) {
        if (!n)
            return;
        popups = popups.filter(p => p !== n);
        for (let i = 0; i < popupList.count; i++)
            if (popupList.get(i).nid === n.id) {
                popupList.remove(i);
                break;
            }
    }

    function clearAll() {
        for (const n of list.slice())
            n.dismiss();
        popups = [];
        popupList.clear();
        unread = 0;
    }

    function iconFor(n) {
        if (n.image)
            return n.image;
        const icon = n.appIcon || Apps.entryFor(n.desktopEntry)?.icon || "";
        if (icon.startsWith("/") || icon.startsWith("file:"))
            return icon;
        return icon ? IconLookup.path(icon, "dialog-information") : IconLookup.path("dialog-information");
    }

    function timeLabel(n, now) {
        const t = received[n.id];
        if (!t)
            return "";
        const mins = Math.floor((now - t) / 60000);
        if (mins < 1)
            return "now";
        if (mins < 60)
            return mins + "m ago";
        return Qt.formatTime(t, "h:mm AP");
    }

    NotificationServer {
        id: server
        keepOnReload: true
        persistenceSupported: true
        bodySupported: true
        bodyMarkupSupported: true
        bodyHyperlinksSupported: true
        actionsSupported: true
        actionIconsSupported: true
        imageSupported: true
        inlineReplySupported: true

        onNotification: n => {
            n.tracked = true;
            if (Panels.open !== "notificationCenter")
                root.unread++;
            const r = Object.assign({}, root.received);
            r[n.id] = new Date();
            root.received = r;
            if (!Focus.allows(n))
                return;
            // Optionally as an iOS-style card in the dynamic island instead of a banner
            if (Island.enabled && Island.wants("notifications") && n.urgency !== NotificationUrgency.Critical)
                root.toIsland(n);
            else
                root.addPopup(n);
        }
    }
}
