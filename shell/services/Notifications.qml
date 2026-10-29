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

    // ── Inline replies ──────────────────────────────────────────────────
    // Messaging apps (Discord, Telegram, Signal, KDE Connect…) let you answer
    // from the notification. The banner and Notification Center show the
    // composer in the card; the island has no keyboard of its own, so its
    // Reply opens Notification Center with that card's composer open.
    property var drafts: ({}) // notification id -> text being written (outlives a card moving from banner to the list)
    property int replyTarget: -1 // a notification whose composer should open (from the island)
    property int replying: 0 // composers open in banners (their window takes the keyboard meanwhile)
    function canReply(n) {
        return !!n && n.hasInlineReply === true;
    }
    function draft(n) {
        return n ? drafts[n.id] ?? "" : "";
    }
    function setDraft(n, text) {
        if (!n)
            return;
        const d = Object.assign({}, drafts);
        if (text === "")
            delete d[n.id];
        else
            d[n.id] = text;
        drafts = d;
    }
    function startReply(n) {
        if (!canReply(n))
            return;
        replyTarget = n.id;
        Panels.show("notificationCenter");
    }
    // The newest notification you can answer (the quick reply shortcut);
    // false when there's none
    function replyLatest() {
        const n = list.slice().reverse().find(x => canReply(x)) ?? null;
        if (n)
            startReply(n);
        return n !== null;
    }
    // Sends it; "" when sent, else what went wrong. The app gets the text;
    // a notification that isn't resident is then done with.
    function reply(n, text) {
        const t = text.trim();
        if (t === "")
            return "Write a reply first.";
        try {
            n.sendInlineReply(t);
        } catch (e) {
            return (n?.appName || "The app") + " closed this notification, so the reply couldn't be sent. Open the app to answer.";
        }
        setDraft(n, "");
        if (!n.resident)
            Qt.callLater(() => sentLater.schedule(n));
        return "";
    }
    // Dismissed a moment after sending, so "Sent" shows first
    QtObject {
        id: sentLater
        property var pending: []
        function schedule(n) {
            pending = pending.concat([n]);
            sentTimer.restart();
        }
    }
    Timer {
        id: sentTimer
        interval: 1200
        onTriggered: {
            for (const n of sentLater.pending)
                n?.dismiss?.();
            sentLater.pending = [];
        }
    }

    function toIsland(n) {
        const words = (n.summary || n.appName || "").split(/\s+/).filter(w => w.length > 0 && w[0].toUpperCase() !== w[0].toLowerCase() || /^[0-9]/.test(w));
        const def = (n.actions ?? []).find(a => a.identifier === "default");
        const b = Object.assign({}, byId);
        b[n.id] = n;
        byId = b;
        Island.post({
            kind: "notification",
            nid: n.id,
            card: {
                name: n.summary || n.appName,
                time: "now",
                text: (n.body ?? "").replace(/<[^>]*>/g, "").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">"),
                initials: words.slice(0, 2).map(w => w[0].toUpperCase()).join(""),
                avatar: n.image ? IconLookup.safe(n.image, []) : "",
                appIcon: n.appIcon ? IconLookup.safe(n.appIcon, "dialog-information") : Apps.iconFor(n.desktopEntry || n.appName || ""),
                buttons: (canReply(n) ? [{ text: "Reply", invoke: () => root.startReply(n) }] : []).concat((n.actions ?? []).filter(a => a.identifier !== "default" && a.identifier !== "inline-reply" && a.text).map(a => ({ text: a.text, invoke: () => a.invoke() }))).slice(0, 2)
            },
            onAction: () => def ? def.invoke() : Panels.show("notificationCenter")
        }, 5000);
    }

    // A notification the island already shows: from the app behind a live
    // activity (the music player), or naming the same thing as an island
    // alert from the last 10 s (the same Bluetooth device, drive, download,
    // timer). Anything with buttons of its own (Reply, Accept) still shows.
    readonly property var islandKinds: ["music", "device", "usb", "display", "charging", "battery", "audio", "download", "transfer", "timer", "stopwatch", "recording", "trash"]
    function norm(x) {
        return String(x ?? "").toLowerCase().replace(/<[^>]*>/g, " ").replace(/[^a-z0-9\u00c0-\uffff]+/g, " ").trim();
    }
    function mentions(a, b) {
        a = norm(a);
        b = norm(b);
        return a.length >= 3 && b.length >= 3 && (a.includes(b) || b.includes(a));
    }
    function actionable(n) {
        return canReply(n) || (n.actions ?? []).some(a => a.identifier !== "default");
    }
    // From the app that's playing, while the music activity is live (its
    // controls are in the island)
    function fromPlayer(n) {
        if (!Island.activities.some(a => a.kind === "music"))
            return false;
        const app = norm(n.appName) + " " + norm(n.desktopEntry);
        const p = Island.player;
        const who = [p?.identity, p?.desktopEntry].map(norm).filter(x => x.length >= 3);
        return who.some(w => app.includes(w) || w.includes(norm(n.appName)));
    }
    function duplicate(n) {
        if (!Island.enabled)
            return false;
        if (fromPlayer(n))
            return true;
        // About something the island just showed (its title names the
        // thing: the device, drive, file or timer; subtitles like
        // "Connected" are too common to go by)
        const now = Date.now();
        return Island.recent.some(e => now - e.at <= 10000 && root.about(e, n));
    }
    function about(e, n) {
        return islandKinds.includes(e.kind) && (mentions(e.title, n.summary) || mentions(e.title, n.body));
    }
    // A notification's own buttons on the island alert about the same
    // thing, while that alert shows: the free places beside its title, the
    // rest on rows under it, and a little longer on screen to use them
    function attachToAlert(n) {
        if (!Island.enabled)
            return false;
        const now = Date.now();
        const e = Island.recent.slice().reverse().find(x => x.uid && now - x.at <= 10000 && about(x, n) && Island.byUid[x.uid]);
        return e ? attach(e.uid, n) : false;
    }
    function attach(uid, n) {
        const ev = Island.byUid[uid];
        const acts = (canReply(n) ? [{ text: "Reply", invoke: () => root.startReply(n) }] : []).concat((n.actions ?? []).filter(a => a.identifier !== "default" && a.identifier !== "inline-reply" && a.text));
        if (!ev || acts.length === 0)
            return false;
        const patch = {};
        let i = 0;
        if (!ev.action && acts[i]) {
            const a = acts[i++];
            patch.action = a.text;
            patch.onAction = () => a.invoke();
        }
        if (!ev.action2 && acts[i]) {
            const a = acts[i++];
            patch.action2 = a.text;
            patch.onAction2 = () => a.invoke();
        }
        // The rest on rows under it
        if (i < acts.length)
            patch.more = (ev.more ?? []).concat(acts.slice(i).map(a => ({ text: a.text, act: () => a.invoke() })));
        patch.expires = Math.max(ev.expires ?? 0, Date.now() + 8000);
        Island.update(uid, patch);
        return true;
    }

    // An island alert that came just after a notification about the same
    // thing (Bluetooth and USB alerts wait a moment for names to settle):
    // the notification leaves the screen (it stays in Notification Center,
    // and the bell), its buttons moving onto the alert; one whose buttons
    // don't fit stays where it is
    function islandSaid(e) {
        const now = Date.now();
        const quietNow = n => n && now - (received[n.id]?.getTime() ?? 0) < 4000 && about(e, n) && (!actionable(n) || attach(e.uid, n));
        for (const n of popups.slice())
            if (quietNow(n))
                dismissPopup(n);
        for (let i = Island.alerts.count - 1; i >= 0; i--) {
            const ev = Island.byUid[Island.alerts.get(i).uid];
            if (ev?.kind === "notification" && ev.nid !== undefined && quietNow(byId[ev.nid]))
                Island.remove(ev.uid);
        }
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

    function forget(id) {
        if (drafts[id] !== undefined) {
            const d = Object.assign({}, drafts);
            delete d[id];
            drafts = d;
        }
        if (byId[id] === undefined && received[id] === undefined)
            return;
        const b = Object.assign({}, byId), r = Object.assign({}, received);
        delete b[id];
        delete r[id];
        byId = b;
        received = r;
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
        if (!n)
            return "";
        // (theme names and image://icon URLs checked: a missing one would draw
        // Qt's pink and black grid)
        if (n.image)
            return IconLookup.safe(n.image, "dialog-information");
        const icon = n.appIcon || Apps.entryFor(n.desktopEntry)?.icon || "";
        if (icon.startsWith("/") || icon.startsWith("file:"))
            return icon;
        return icon ? IconLookup.safe(icon, "dialog-information") : IconLookup.path("dialog-information");
    }

    function timeLabel(n, now) {
        const t = n ? received[n.id] : undefined;
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
        inlineReplySupported: true // the composer: NotificationCard (banners, Notification Center), Reply in the island

        onNotification: n => {
            n.tracked = true;
            // Forget it in the lookup maps once it's closed for good (they'd
            // otherwise grow forever); later, so cards animating out still have it
            n.closed.connect(() => Qt.callLater(() => root.forget(n.id)));
            // keepOnReload: after a shell reload the server hands back the
            // notifications it kept; they were shown already
            if (n.lastGeneration)
                return;
            if (Panels.open !== "notificationCenter")
                root.unread++;
            const r = Object.assign({}, root.received);
            r[n.id] = new Date();
            root.received = r;
            if (!Focus.allows(n))
                return;
            // Already on show in the island (a live activity, or an alert a
            // moment ago): only to Notification Center and the bell, quietly.
            // One with buttons of its own (Reply, Accept) puts them on that
            // island alert when it's still showing; otherwise it shows as
            // usual, in the island or as a banner, as set.
            if (root.actionable(n) ? root.attachToAlert(n) : root.duplicate(n))
                return;
            Sounds.notify(n);
            // Optionally as an iOS-style card in the dynamic island instead of a banner
            if (Island.enabled && Island.armed && Island.wants("notifications") && n.urgency !== NotificationUrgency.Critical)
                root.toIsland(n);
            else
                root.addPopup(n);
        }
    }
}
