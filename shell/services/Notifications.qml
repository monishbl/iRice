pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Notifications
import qs.common
import "../common/phosphor.js" as Phosphor
import "../modules/notifications/NotificationGroups.js" as NotificationGroups

// Notification daemon. Everything received is kept for Notification Center
// until dismissed (the newest 200); new ones also show as banners unless Do Not Disturb is on
// (critical ones always show).
// With "notifications" in rust.surfaces, tessera-ui serves them instead (its
// server, its banners: native/tessera-ui/src/surfaces/notifications) and this
// mirrors its list for Notification Center, the lock screen, the bell and the
// island ("tessera-ui's list" below); this shell's server comes back when
// tessera-ui stops serving.
Singleton {
    id: root

    // Oldest first: this shell's server's, then tessera-ui's (while it serves,
    // and what it left when it stopped, until dismissed)
    readonly property var list: {
        const own = serverLoader.item?.trackedNotifications.values ?? [];
        return mirror.length === 0 ? own : Array.from(own).concat(mirror);
    }
    property var popups: [] // banners showing, newest first
    // The same banners as a stable model ({ nid }), so adding one doesn't
    // rebuild (and re-animate) the others; byId maps nid -> notification
    property alias popupModel: popupList
    ListModel { id: popupList }
    property var byId: ({})
    readonly property int keep: 200

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

    // ── Incoming calls ──────────────────────────────────────────────────
    // A ringing call (Discord, WhatsApp, any app): category "call" (or an
    // x-…call one), or words like "Incoming call" / "is calling". With the
    // island on it becomes the island's call card (Calls.ring: Answer,
    // Decline, a ringtone); otherwise a banner. Either way it stays until
    // it's answered, declined, or the app takes it back (closes it).
    function isCall(n) {
        if (!n)
            return false;
        const cat = String(n.hints?.category ?? "").toLowerCase();
        if (/missed|ended|declined/.test(cat))
            return false;
        if (/(^|[.\-])call($|[.\-])/.test(cat))
            return true;
        const text = norm(n.summary) + " " + norm(n.body);
        // (Discord says "<name> started a call.")
        return /\bincoming (voice |video |audio |group )?call\b|\bis calling\b|\bcalling you\b|\b(voice|video) call from\b|\bwants to (video )?call\b|\bstarted a (voice |video )?call\b/.test(text) && !/\bmissed\b|\bended\b/.test(text);
    }

    // A notification body as markup a card can show safely. The spec's
    // subset stays (bold, italic, underline, line breaks, links to web and
    // mail); everything else goes, above all <img>, which Qt would fetch
    // from the network the moment the card shows (a read receipt for
    // whoever wrote the message).
    function safeMarkup(body) {
        return String(body ?? "").replace(/<(\/?)([a-zA-Z][a-zA-Z0-9]*)\b([^<>]*)>/g, (tag, close, name, rest) => {
            const n = name.toLowerCase();
            if (n === "b" || n === "i" || n === "u")
                return "<" + close + n + ">";
            if (n === "br")
                return close ? "" : "<br>";
            if (n === "a") {
                if (close)
                    return "</a>";
                const m = /\bhref\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(rest);
                const href = m ? (m[1] ?? m[2] ?? "") : "";
                return Launch.linkAllowed(href) ? "<a href=\"" + href.replace(/"/g, "%22") + "\">" : "<a>";
            }
            return "";
        });
    }
    // A notification body as plain text (markup and entities gone)
    function plainBody(body) {
        return String(body ?? "").replace(/<[^>]*>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">")
            .replace(/&quot;/g, "\"").replace(/&apos;|&#39;/g, "'").replace(/&amp;/g, "&");
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
                text: plainBody(n.body), // (shown as plain text)
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
        if (rustServes && mirror.length > 0)
            RustUi.send({ to: "notifications", do: "clear" });
        for (const n of list.slice())
            if (!n.mirrored || !rustServes)
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
        return Qt.formatTime(t, TimeFormat.hm);
    }

    // ── Who serves: this shell, or tessera-ui ───────────────────────────
    // Quickshell's server, once made, holds org.freedesktop.Notifications for
    // as long as this shell runs (it can't let the name go). So while
    // tessera-ui is to serve them (rust.surfaces), it isn't made: tessera-ui
    // takes the name, then says it draws "notifications". It's made (and
    // stays) when tessera-ui isn't wanted for them, or hasn't said so a few
    // seconds after the start (or after it stopped: a restart with a new list
    // takes a moment). Made while tessera-ui still holds the name, it takes
    // the name the moment tessera-ui lets go (Quickshell watches for that);
    // tessera-ui, queued behind it, then never draws them. The banners are
    // always drawn by whoever holds the name.
    readonly property bool rustWanted: Config.ready && (Config.options.rust.surfaces ?? []).includes("notifications")
    readonly property bool rustServes: RustUi.drawing("notifications")
    property bool ownServer: false
    onRustWantedChanged: considerServer()
    onRustServesChanged: {
        considerServer();
        if (rustServes) {
            mirrorPrimed = false;
            RustUi.send({ to: "notifications", do: "hello" });
            tellRust();
        }
    }
    Connections {
        target: Config
        function onReadyChanged() { root.considerServer(); }
    }
    Component.onCompleted: considerServer()
    function considerServer() {
        if (ownServer || !Config.ready)
            return;
        if (rustServes) {
            serverWait.stop();
            return;
        }
        if (!serverWait.running) {
            serverWait.interval = rustWanted ? 6000 : 1;
            serverWait.start();
        }
    }
    Timer {
        id: serverWait
        onTriggered: if (!root.rustServes) root.ownServer = true
    }

    // ── tessera-ui's list, while it serves ──────────────────────────────
    // Its lines (RustUi.message): {"kind":"notifications","list":[…]}, each
    // with Quickshell's names and where it went as it arrived (route: banner
    // | island | call | quiet). Each becomes an object shaped like
    // Quickshell's Notification (a closed signal, actions with invoke(),
    // dismiss(), sendInlineReply()) whose acts go back as lines on its stdin.
    property var mirror: []
    property var mirrorById: ({})
    property bool mirrorPrimed: false // (the first list after hello is old news: no sounds)
    property var dismissedHere: ({}) // ids dismissed from this shell (closed as Dismissed)
    Connections {
        target: RustUi
        function onMessage(e) {
            if (e.kind === "notifications")
                root.fromRust(e.list ?? []);
            else if (e.kind === "nc")
                root.ncAct(e);
        }
    }
    function toRust(id, o) {
        RustUi.send(Object.assign({ to: "notifications", id: id }, o));
    }
    // Its banners hide under Notification Center and Control Center, and move
    // down while the OSD shows, as this shell's own do
    readonly property var rustShell: ({ covered: Panels.open === "notificationCenter" || Panels.open === "controlCenter", osd: Panels.osdVisible })
    onRustShellChanged: tellRust()
    function tellRust() {
        if (rustServes)
            RustUi.send(Object.assign({ to: "notifications", do: "shell" }, rustShell));
    }
    Component {
        id: mirrorNote
        QtObject {
            readonly property bool mirrored: true
            property int id
            property string appName
            property string appIcon
            property string desktopEntry
            property string summary
            property string body
            property string image
            property int urgency: 1
            property real expireTimeout: -1
            property bool resident
            // (no `transient`: a reserved word in QML, and nothing here reads it)
            property bool hasInlineReply
            property string inlineReplyPlaceholder
            property var hints: ({})
            property var actions: []
            property string route
            property bool tracked: true
            readonly property bool lastGeneration: false
            signal closed(int reason)
            function dismiss() {
                const d = Object.assign({}, root.dismissedHere);
                d[id] = true;
                root.dismissedHere = d;
                if (root.rustServes)
                    root.toRust(id, { do: "dismiss" });
                else
                    root.dropMirrored(id, NotificationCloseReason.Dismissed);
            }
            function expire() { dismiss(); }
            function sendInlineReply(text) {
                if (!root.rustServes)
                    throw new Error("gone");
                root.toRust(id, { do: "reply", text: text });
            }
        }
    }
    function fill(o, x) {
        const id = x.id;
        o.appName = x.appName ?? "";
        o.appIcon = x.appIcon ?? "";
        o.desktopEntry = x.desktopEntry ?? "";
        o.summary = x.summary ?? "";
        o.body = x.body ?? "";
        o.image = x.image ?? "";
        o.urgency = x.urgency ?? 1;
        o.expireTimeout = x.expireTimeout ?? -1;
        o.resident = !!x.resident;
        o.hasInlineReply = !!x.hasInlineReply;
        o.inlineReplyPlaceholder = x.inlineReplyPlaceholder ?? "";
        o.route = x.route ?? "";
        const h = { "desktop-entry": o.desktopEntry };
        if (x.category)
            h.category = x.category;
        if ((x.value ?? -1) >= 0)
            h.value = x.value;
        if (x.suppressSound)
            h["suppress-sound"] = true;
        if (x.soundFile)
            h["sound-file"] = x.soundFile;
        o.hints = h;
        o.actions = (x.actions ?? []).map(a => ({ identifier: a.id, text: a.text, invoke: () => root.toRust(id, { do: "invoke", action: a.id }) }));
    }
    function fromRust(items) {
        const now = Date.now();
        const byIdNow = {}, out = [], fresh = [];
        const r = Object.assign({}, received);
        for (const x of items) {
            let o = mirrorById[x.id];
            if (!o) {
                o = mirrorNote.createObject(root, { id: x.id });
                fresh.push(o);
            }
            fill(o, x);
            byIdNow[x.id] = o;
            out.push(o);
            r[x.id] = new Date(x.at || now);
        }
        const gone = Object.keys(mirrorById).filter(id => !byIdNow[id]).map(id => mirrorById[id]);
        mirrorById = byIdNow;
        mirror = out;
        received = r;
        const b = Object.assign({}, byId);
        for (const o of out)
            b[o.id] = o;
        byId = b;
        for (const o of gone)
            closeMirrored(o, dismissedHere[o.id] ? NotificationCloseReason.Dismissed : NotificationCloseReason.CloseRequested);
        const primed = mirrorPrimed;
        mirrorPrimed = true;
        if (primed)
            for (const o of fresh)
                arrivedFromRust(o);
    }
    // What this shell does for a new one tessera-ui received: the bell, and
    // its sound, island card or call card (the banner is tessera-ui's)
    function arrivedFromRust(n) {
        if (Panels.open !== "notificationCenter")
            unread++;
        if (n.route === "banner") {
            Sounds.notify(n);
        } else if (n.route === "island") {
            Sounds.notify(n);
            if (Island.enabled && Island.armed)
                toIsland(n);
            else
                toRust(n.id, { do: "banner" });
        } else if (n.route === "call") {
            if (!Calls.ring(n)) {
                Sounds.notify(n);
                toRust(n.id, { do: "banner" });
            }
        }
    }
    function closeMirrored(o, reason) {
        o.closed(reason);
        const d = Object.assign({}, dismissedHere);
        delete d[o.id];
        dismissedHere = d;
        Qt.callLater(() => {
            root.forget(o.id);
            o.destroy();
        });
    }
    // One left from tessera-ui's last run, dismissed after it stopped
    function dropMirrored(id, reason) {
        const o = mirrorById[id];
        if (!o)
            return;
        const m = Object.assign({}, mirrorById);
        delete m[id];
        mirrorById = m;
        mirror = mirror.filter(x => x !== o);
        closeMirrored(o, reason);
    }

    // ── Notification Center drawn by tessera-ui ─────────────────────────
    // While it serves them it draws Notification Center too
    // (native/tessera-ui/src/surfaces/notifications/panel.rs): Panels still
    // opens and closes it (the clock, the hot corner, IPC); each is passed on
    // as a command line, with the widgets' data (Calendar, Todos, Timers,
    // the island's alerts) while it's open. It asks for what only this shell
    // does as {"kind":"nc","act":…} lines (ncAct below).
    readonly property bool rustCenter: rustServes && Panels.open === "notificationCenter"
    readonly property string rustCenterScreen: rustCenter ? (Panels.screen?.name ?? "") : ""
    onRustCenterChanged: forwardCenter()
    onRustCenterScreenChanged: forwardCenter()
    function forwardCenter() {
        if (rustCenter) {
            RustUi.send({ cmd: "open", surface: "notifications", screen: rustCenterScreen });
            if (rustWidgets)
                RustUi.send(rustWidgets);
            sendCalendar();
        } else {
            if (rustServes)
                RustUi.send({ cmd: "close", surface: "notifications" });
            ncFine(false);
        }
    }
    property var ncUndo: null // a to-do just deleted, offered back for a few seconds
    Timer {
        id: ncUndoTimer
        interval: 8000
        onTriggered: root.ncUndo = null
    }
    property bool ncFineOn: false // (the stopwatch's hundredths, while it's shown)
    function ncFine(on) {
        if (on === ncFineOn)
            return;
        ncFineOn = on;
        Timers.swFine = Math.max(0, Timers.swFine + (on ? 1 : -1));
    }
    readonly property var rustWidgets: !rustCenter ? null : ({
            to: "notifications",
            do: "widgets",
            tab: Panels.ncTab,
            focus: Focus.on ? (Focus.mode?.name ?? "Focus") + " is on" : "",
            alerts: Island.history.map(a => ({
                        hid: a.hid,
                        kind: NotificationGroups.kindOf(a),
                        glyph: Phosphor.glyphs[a.icon ?? "bell"] ?? Phosphor.glyphs["bell"],
                        color: String(a.color ?? Theme.accent),
                        picture: a.picture ?? "",
                        title: a.title ?? "",
                        detail: [a.subtitle ?? "", a.battery !== undefined ? "Battery " + Math.round(a.battery * 100) + "%" : "", (a.times ?? 1) > 1 ? a.times + " times" : ""].filter(x => !!x).join(" · "),
                        at: a.at ?? 0,
                        act: Island.historyActs[a.hid]?.text ?? ""
                    })),
            todos: Todos.list.map(t => ({
                        key: t.key,
                        text: t.text ?? "",
                        done: !!t.done,
                        due: [Todos.dueLabel(t), t.google ? "Google · " + t.listName : ""].filter(Boolean).join(" · "),
                        overdue: !!Todos.overdue(t)
                    })),
            undo: ncUndo?.text ?? "",
            timer: { remaining: Timers.format(Timers.remaining), phase: Timers.phase, progress: Timers.progress, running: Timers.running, completed: Timers.completed },
            sw: { elapsed: Timers.formatMs(Timers.swElapsed), running: Timers.swRunning, laps: Timers.laps.map((l, i) => ({ name: "Lap " + (Timers.laps.length - i), time: Timers.formatMs(l) })) }
        })
    onRustWidgetsChanged: if (rustWidgets) RustUi.send(rustWidgets)
    // The calendar's days it shows ({from, to, day}: "y-mm-dd", `to` the day
    // after): CalendarWidget.qml's marks, everyDay and items
    property var ncCal: null
    function sendCalendar() {
        if (!rustCenter || !ncCal)
            return;
        void Calendar.local, Calendar.remote;
        const p = s => {
            const [y, m, d] = s.split("-").map(Number);
            return new Date(y, m - 1, d);
        };
        const key = d => d.getFullYear() + "-" + String(d.getMonth() + 1).padStart(2, "0") + "-" + String(d.getDate()).padStart(2, "0");
        const first = p(ncCal.from), to = p(ncCal.to), day = p(ncCal.day);
        const list = Calendar.between(first, to);
        const daily = Calendar.dailyIn(list, first, to);
        const marks = {};
        const mark = k => marks[k] = marks[k] ?? { colors: [], todos: 0 };
        for (const e of list) {
            if (daily[Calendar.seriesOf(e)])
                continue;
            const d = new Date(e.start.getFullYear(), e.start.getMonth(), e.start.getDate());
            let end = e.allDay ? new Date(e.end.getTime() - 1) : e.end;
            end = new Date(end.getFullYear(), end.getMonth(), end.getDate());
            for (; d <= end; d.setDate(d.getDate() + 1)) {
                const c = String(e.color || Theme.accent);
                if (!mark(key(d)).colors.includes(c))
                    mark(key(d)).colors.push(c);
            }
        }
        for (const t of Todos.list)
            if (t.due && !t.done)
                mark(key(new Date(t.due))).todos++;
        const event = e => ({ kind: "event", title: e.title ?? "", sub: Calendar.timeText(e) + (e.location ? " · " + e.location : "") + (e.calendar ? " · " + e.calendar : ""), color: String(e.color || Theme.accent), key: String(e.id ?? "") });
        const todo = t => ({ kind: "todo", title: t.text ?? "", sub: "To-do" + (t.allDay ? "" : " · " + Qt.formatTime(new Date(t.due), TimeFormat.hm)), done: !!t.done, key: t.key });
        // (the week: a few of what's on each day, then "+N more")
        if ((to - first) / 86400000 <= 7.5)
            for (const d = new Date(first); d < to; d.setDate(d.getDate() + 1)) {
                const evs = Calendar.eventsOn(d), tds = Todos.on(d).filter(t => !t.done);
                const m = mark(key(d));
                m.lines = evs.slice(0, 4).map(event).concat(tds.slice(0, 3).map(todo));
                m.more = evs.length + tds.length - m.lines.length;
            }
        RustUi.send({
            to: "notifications",
            do: "calendar",
            marks: marks,
            everyDay: Object.values(daily).map(d => d.title).join(", "),
            items: Calendar.eventsOn(day).map(event).concat(Todos.on(day).map(todo))
        });
    }
    Connections {
        target: Calendar
        function onLocalChanged() { root.sendCalendar(); }
        function onRemoteChanged() { root.sendCalendar(); }
    }
    Connections {
        target: Todos
        function onListChanged() { root.sendCalendar(); }
    }
    // What Notification Center asks of this shell
    function ncAct(e) {
        switch (e.act) {
        case "close":
            if (Panels.open === "notificationCenter")
                Panels.close();
            break;
        case "manage":
            Panels.openSettings("notifications");
            break;
        case "tab":
            Panels.ncTab = e.tab ?? 0;
            break;
        case "calendar":
            ncCal = { from: e.from, to: e.to, day: e.day };
            sendCalendar();
            break;
        case "todo.add":
            if ((e.text ?? "").trim() !== "")
                Todos.add(e.text);
            break;
        case "todo.toggle": {
            const t = Todos.find(e.key);
            if (t)
                Todos.update(t, { done: !t.done });
            break;
        }
        case "todo.rename":
            if ((e.text ?? "").trim() !== "")
                Todos.update(e.key, { text: e.text.trim() });
            break;
        case "todo.remove":
            ncUndo = Todos.remove(e.key);
            ncUndoTimer.restart();
            break;
        case "todo.restore":
            Todos.restore(ncUndo);
            ncUndo = null;
            break;
        case "timer.toggle":
            Timers.toggle();
            break;
        case "timer.skip":
            Timers.skip();
            break;
        case "timer.reset":
            Timers.reset();
            break;
        case "sw.toggle":
            Timers.swToggle();
            break;
        case "sw.lap":
            Timers.swLap();
            break;
        case "sw.reset":
            Timers.swReset();
            break;
        case "sw.fine":
            ncFine(!!e.on && rustCenter);
            break;
        case "island.forget":
            Island.forget(e.hid);
            break;
        case "island.clear":
            Island.clearHistory();
            break;
        case "alert.act":
            Island.historyActs[e.hid]?.act?.();
            break;
        }
    }

    LazyLoader {
        id: serverLoader
        active: root.ownServer

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
                // At most `keep` stay in Notification Center: the oldest go (a
                // chatty app's, with their pictures, piled up until dismissed)
                const all = server.trackedNotifications.values;
                for (let i = 0; i < all.length - root.keep; i++)
                    all[i].dismiss();
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
                if (root.isCall(n)) {
                    if (Calls.ring(n))
                        return;
                    Sounds.notify(n);
                    root.addPopup(n);
                    return;
                }
                // The app turned its ringing call into something else (same id:
                // "Missed call", "Call ended"): the call card goes, as missed
                if (Calls.ringing && Calls.ringing.n.id === n.id)
                    Calls.missed();
                // Already on show in the island (a live activity, or an alert a
                // moment ago): only to Notification Center and the bell, quietly.
                // One with buttons of its own (Reply, Accept) puts them on that
                // island alert when it's still showing; otherwise it shows as
                // usual, in the island or as a banner, as set.
                if (root.actionable(n) ? root.attachToAlert(n) : root.duplicate(n))
                    return;
                // A fullscreen app with "Only important" (Settings → Menu Bar →
                // Island): only critical ones show; the rest wait in Notification
                // Center, quietly
                if (Island.fsQuiet() && n.urgency !== NotificationUrgency.Critical)
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
}
