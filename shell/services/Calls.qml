pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Notifications
import qs.common

// A call or meeting in progress, for the island's live activity: an app made
// for calls is using the microphone (Discord, Zoom, Teams, Slack...), or a
// browser is, with a meeting open in one of its windows (Google Meet, Zoom,
// Teams, Jitsi...). Mute is the system microphone; Leave closes the call's
// own window where the app has one (the app then asks or hangs up); a call in
// a browser tab offers Open instead, so other tabs are never closed.
// Also the ringing: an incoming call (Notifications.isCall) becomes a call
// card in the island (IslandCall), see "Incoming calls" below.
Singleton {
    id: root

    readonly property bool on: Island.enabled && Island.wants("calls")

    readonly property var callApps: [
        { re: /discord|vesktop|webcord/i, name: "Discord", window: /discord|vesktop/i },
        { re: /^zoom|zoom\.real|zoomwebview/i, name: "Zoom", window: /^zoom$/i, callTitle: /^Zoom Meeting|^Meeting$/i },
        { re: /teams/i, name: "Teams", window: /teams/i },
        { re: /slack/i, name: "Slack", window: /slack/i, callTitle: /huddle/i },
        { re: /skype/i, name: "Skype", window: /skype/i },
        { re: /signal/i, name: "Signal", window: /signal/i },
        { re: /telegram/i, name: "Telegram", window: /telegram/i },
        { re: /element/i, name: "Element", window: /element/i },
        { re: /whatsapp|zapzap/i, name: "WhatsApp", window: /whatsapp|zapzap/i },
        { re: /webex/i, name: "Webex", window: /webex/i }
    ]
    readonly property var browsers: /firefox|librewolf|zen|floorp|waterfox|chrom|brave|vivaldi|edge|opera|epiphany|qutebrowser/i
    readonly property var meetings: [
        { re: /Google Meet|^Meet [-–]|meet\.google/i, name: "Google Meet" },
        { re: /Zoom/i, name: "Zoom" },
        { re: /Microsoft Teams|\| Teams/i, name: "Teams" },
        { re: /Jitsi/i, name: "Jitsi Meet" },
        { re: /Discord/i, name: "Discord" },
        { re: /Whereby/i, name: "Whereby" },
        { re: /Webex/i, name: "Webex" },
        { re: /Slack/i, name: "Slack" },
        { re: /WhatsApp/i, name: "WhatsApp" }
    ]

    // { name, window (HyprlandToplevel or null), leave (true: closing its
    // window ends it), appIcon (a call answered in the island: "Call" with
    // the app's icon) }
    readonly property var call: {
        if (!on)
            return null;
        const wins = Hyprland.toplevels.values;
        const ans = answered;
        for (const s of Privacy.micStreams) {
            const id = (s.bin + " " + s.name).trim();
            const app = callApps.find(a => a.re.test(id));
            if (app) {
                const own = wins.filter(t => app.window.test(t.lastIpcObject?.class ?? ""));
                const callWin = app.callTitle ? own.find(t => app.callTitle.test(t.title ?? "")) : null;
                const mine = ans && ans.app === app.name;
                return { name: mine ? "Call" : app.name, window: callWin ?? own[0] ?? null, leave: !!callWin, appIcon: mine ? ans.appIcon : "" };
            }
            if (browsers.test(id)) {
                for (const t of wins) {
                    if (!browsers.test(t.lastIpcObject?.class ?? ""))
                        continue;
                    const m = meetings.find(x => x.re.test(t.title ?? ""));
                    if (m)
                        return { name: m.name, window: t, leave: false };
                }
            }
        }
        // Answered in the island, and the app opened a window for the call
        // (titled like a call, or a second window next to the ones it had:
        // a first one is just the app coming up): live while that window is
        // open; closing it ends the call
        if (ans) {
            const own = wins.filter(t => root.windowOf(ans, t));
            const callWin = own.find(t => /\b(call|calling|meeting|huddle)\b/i.test(t.title ?? "") || (ans.before.length > 0 && !ans.before.includes(t.address)));
            if (callWin)
                return { name: "Call", window: callWin, leave: true, appIcon: ans.appIcon };
        }
        return null;
    }
    readonly property bool active: call !== null
    property real started: 0
    onActiveChanged: {
        // (an answered call counts from the moment it was answered)
        started = !active ? 0 : answered && Date.now() - answered.at < 60000 ? answered.at : Date.now();
        if (!active && answered && answeredWait.seen)
            answered = null; // that call is over
        answeredWait.seen = active;
    }

    function open() {
        const t = call?.window;
        if (!t)
            return;
        const id = t.workspace?.id ?? 0;
        if (id > 0 && id !== Hyprland.focusedMonitor?.activeWorkspace?.id)
            Hyprland.dispatch("hl.dsp.focus({ workspace = " + id + " })");
        Qt.callLater(() => t.wayland?.activate());
    }
    function leave() {
        const t = call?.window;
        if (!t || !call.leave)
            return open();
        Hyprland.dispatch("hl.dsp.window.close({ window = \"address:0x" + t.address.replace(/^0x/, "") + "\" })");
    }

    // ── Incoming calls ──────────────────────────────────────────────────
    // A ringing call opens the island as a call card (IslandCall): the
    // caller's photo, name and app, Decline and Answer. It rings (the
    // ringtone every 2.6 s, for about 30 s) and stays until it's answered,
    // declined (Esc too), or the app takes it back (the caller hung up or it
    // was answered elsewhere); unanswered, it goes into the island's history
    // as a missed call. Answer uses the notification's own Answer/Accept,
    // else its default action, else opens the app; Decline its
    // Decline/Reject, else dismisses it. An app that can't take calls here
    // (WhatsApp: "check your phone") gets Open instead of Answer.
    // With the island (or its Calls alert) off, calls stay a banner
    // (Notifications.qml).
    property var ringing: null // { n, uid, name, app, appIcon, photo, initials, openOnly, at, ... }
    readonly property bool isRinging: ringing !== null
    readonly property var answerRe: /\b(answer|accept|pick ?up|join)\b/i
    readonly property var declineRe: /\b(decline|reject|dismiss|ignore|hang ?up|deny|busy)\b/i
    readonly property var phoneOnlyRe: /\b(check|answer (it )?on|use) your (phone|mobile)\b|\bon your phone\b/i

    function plain(x) {
        return String(x ?? "").replace(/<[^>]*>/g, "").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").trim();
    }
    function actionFor(n, re) {
        return (n?.actions ?? []).find(a => a.identifier !== "inline-reply" && a.identifier !== "default" && (re.test(a.identifier ?? "") || re.test(a.text ?? ""))) ?? null;
    }
    // Who's calling and from which app
    function describe(n) {
        const entry = Apps.entryFor(n.desktopEntry || n.appName || "");
        const known = callApps.find(a => a.re.test((n.desktopEntry ?? "") + " " + (n.appName ?? "")));
        const app = known?.name ?? (n.appName || entry?.name || "Call");
        const summary = plain(n.summary), body = plain(n.body);
        // ("Incoming voice call" / "Roshan": the name is the body)
        const name = /\bcall\b|\bcalling\b/i.test(summary) && body !== "" && body.length <= 40 && !/\bcall/i.test(body) ? body : summary || app;
        const words = name.split(/\s+/).filter(w => w.length > 0 && (w[0].toUpperCase() !== w[0].toLowerCase() || /^[0-9]/.test(w)));
        const keys = [n.desktopEntry, n.appName, entry?.id].map(k => String(k ?? "").toLowerCase().replace(/\.desktop$/, "").replace(/[^a-z0-9]+/g, "")).filter(k => k.length >= 3);
        return {
            name: name,
            app: app,
            appIcon: n.appIcon ? IconLookup.safe(n.appIcon, "dialog-information") : Apps.iconFor(n.desktopEntry || n.appName || ""),
            photo: n.image ? IconLookup.safe(n.image, []) : "",
            initials: words.slice(0, 2).map(w => w[0].toUpperCase()).join(""),
            openOnly: phoneOnlyRe.test(body + " " + summary),
            canAnswer: !!actionFor(n, answerRe),
            winRe: known?.window ?? null,
            keys: keys,
            entryId: entry?.id ?? ""
        };
    }
    // Does this Hyprland window belong to the call's app?
    function windowOf(r, t) {
        const cls = String(t.lastIpcObject?.class ?? "").toLowerCase();
        if (cls === "")
            return false;
        if (r.winRe)
            return r.winRe.test(cls);
        const flat = cls.replace(/[^a-z0-9]+/g, "");
        return r.keys.some(k => flat.includes(k) || (flat.length >= 4 && k.includes(flat)));
    }
    // Shows a ringing call in the island; false when the island can't (off,
    // or the shell just started), so it stays a banner
    function ring(n) {
        if (!Island.enabled || !Island.armed || !Island.wants("calls") || !n)
            return false;
        // Over a fullscreen window (a video, a game) the menu bar lifts its
        // island above it for the card (in every "In full screen" mode). A
        // display without a menu bar has no island: the call is a banner there.
        const shown = Config.options.menubar.screens;
        if (Island.fullscreenNow() && shown.length > 0 && !shown.includes(Hyprland.focusedMonitor?.name ?? ""))
            return false;
        const info = describe(n);
        const card = { kind: "ringing", sticky: true, nid: n.id, title: info.name, subtitle: info.app, call: info };
        // The app changed it (same id): the card follows
        if (ringing && ringing.n.id === n.id) {
            ringing = Object.assign({}, ringing, info, { n: n });
            Island.update(ringing.uid, card);
            return true;
        }
        if (ringing)
            missed(); // a second call while one rings: the first one goes by
        const uid = Island.post(card, Island.untilOver);
        if (!uid)
            return false;
        ringing = Object.assign({ n: n, uid: uid, at: Date.now() }, info);
        rings = 0;
        const h = n.hints ?? {};
        ringFile = h["suppress-sound"] ? "" : String(h["sound-file"] ?? "").startsWith("/") ? String(h["sound-file"]) : Sounds.allowed("ringtone") ? Sounds.file("ringtone") : "";
        chime.restart();
        giveUp.restart();
        return true;
    }
    // Ends the ringing (the card leaves, the ringtone stops)
    function stopRinging() {
        const r = ringing;
        ringing = null;
        chime.stop();
        giveUp.stop();
        ringPlayer.running = false;
        if (r)
            Island.remove(r.uid);
        return r;
    }
    function answer() {
        const r = stopRinging();
        if (!r)
            return;
        const n = r.n;
        if (!r.openOnly) {
            // Its call window (or microphone) shows the live activity
            answered = { app: r.app, appIcon: r.appIcon, winRe: r.winRe, keys: r.keys, at: Date.now(), before: Hyprland.toplevels.values.filter(t => windowOf(r, t)).map(t => t.address) };
            answeredWait.seen = false;
            answeredWait.restart();
        }
        const a = r.openOnly ? null : actionFor(n, answerRe);
        const def = (n?.actions ?? []).find(x => x.identifier === "default") ?? null;
        try {
            if (a)
                a.invoke();
            else if (def)
                def.invoke();
            else
                openApp(r);
        } catch (e) {
            openApp(r); // (the app closed it meanwhile)
        }
    }
    function decline() {
        const r = stopRinging();
        if (!r)
            return;
        const a = actionFor(r.n, declineRe);
        try {
            if (a)
                a.invoke();
            else
                r.n.dismiss();
        } catch (e) {}
    }
    // Unanswered (the caller hung up, or nothing happened for 2 minutes):
    // a line in the island's history (Recent, Notification Center)
    function missed() {
        const r = stopRinging();
        if (!r)
            return;
        Island.record({ kind: "missedCall", icon: "phone-x", title: "Missed call", subtitle: r.name + (r.name !== r.app ? " · " + r.app : ""), color: Theme.accents.red, action: "Open " + r.app, onAction: () => root.openApp(r) });
    }
    // Brings up the app's window, or starts it
    function openApp(r) {
        const h = Hyprland.toplevels.values.filter(t => windowOf(r, t));
        if (h.length > 0)
            return Apps.show(h[h.length - 1]);
        const entry = r.entryId ? Apps.byId(r.entryId) : null;
        if (entry)
            Apps.launch(entry);
    }
    // The notification went away while ringing: the app took it back (the
    // caller hung up) is a missed call; you dismissing it (Notification
    // Center) is a decline
    Connections {
        target: root.ringing?.n ?? null
        function onClosed(reason) {
            if (reason === NotificationCloseReason.Dismissed)
                root.stopRinging();
            else
                root.missed();
        }
    }
    // A safety net: an app that never takes its call notification back
    Timer {
        id: giveUp
        interval: 120000
        onTriggered: root.missed()
    }
    // The ringtone: every 2.6 s, quiet after about 30 s (the card stays)
    property int rings: 0
    property string ringFile: ""
    Timer {
        id: chime
        interval: 2600
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            if (!root.ringing || root.ringFile === "" || ++root.rings > 12) {
                chime.stop();
                return;
            }
            if (!ringPlayer.running)
                ringPlayer.running = true;
        }
    }
    Process {
        id: ringPlayer
        command: Sounds.command(root.ringFile)
    }

    // A call answered in the island, until its window (or microphone) shows
    // up and then goes; given up after 20 s with neither
    property var answered: null // { app, appIcon, winRe, keys, at, before: [window addresses] }
    Timer {
        id: answeredWait
        property bool seen: false
        interval: 20000
        onTriggered: if (!root.active) root.answered = null
    }

    // qs ipc call calls answer | decline | state  (as the card's buttons)
    IpcHandler {
        target: "calls"
        function answer(): void { root.answer(); }
        function decline(): void { root.decline(); }
        function state(): string {
            return JSON.stringify({ ringing: root.ringing ? { name: root.ringing.name, app: root.ringing.app, openOnly: root.ringing.openOnly, canAnswer: root.ringing.canAnswer, rings: root.rings } : null, answered: root.answered ? root.answered.app : null, call: root.call ? { name: root.call.name, leave: root.call.leave } : null });
        }
    }
}
