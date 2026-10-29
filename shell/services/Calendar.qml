pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Calendar events and reminders. Subscriptions are iCalendar (.ics) links:
// Google Calendar's "secret address in iCal format", iCloud public calendars,
// Outlook's published ICS, Nextcloud/CalDAV export links, or local .ics files.
// Local events and reminders are made in the Notification Center or Spotlight
// ("remind me to … at 5pm"). Alerts arrive as notifications.
//
// Times with a TZID are read as local time; recurrence covers the usual
// RRULEs (DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL, BYDAY,
// BYMONTHDAY) plus EXDATE and moved instances.
Singleton {
    id: root

    // Subscriptions: [{ name, url, color, enabled }] in ~/.config/irice/calendars.json
    property var sources: []
    // Local events: [{ id, title, start, end, allDay, alert (minutes or -1), notes }] (ms timestamps)
    property var local: []
    // Parsed subscription events by source url: [{ uid, title, start, end, allDay, location, alert, rrule… }]
    property var remote: ({})
    property string lastError: ""
    property int defaultAlert: 10 // minutes before timed events without their own alert

    readonly property string configPath: Config.dir + "/calendars.json"
    readonly property string statePath: UiState.dir + "/events.json"
    readonly property string cacheDir: Wallpaper.cacheDir + "/calendars"

    // ---------- Queries ----------

    // Expanded instances overlapping [from, to): { key, title, start: Date, end: Date, allDay, location, color, calendar, local, id }
    function between(from, to) {
        const out = [];
        for (const e of local) {
            if (e.start < to.getTime() && ((e.end || e.start) > from.getTime() || ((!e.end || e.end === e.start) && e.start >= from.getTime())))
                out.push({ key: "local:" + e.id, id: e.id, title: e.title, start: new Date(e.start), end: new Date(e.end || e.start), allDay: !!e.allDay, location: "", color: "", calendar: "Reminders", local: true, alert: e.alert ?? defaultAlert, notes: e.notes ?? "" });
        }
        for (const src of sources) {
            if (src.enabled === false)
                continue;
            for (const ev of remote[src.url] ?? [])
                for (const inst of expand(ev, from, to))
                    out.push(Object.assign({ key: src.url + "|" + ev.uid + "|" + inst.start.getTime(), title: ev.title, allDay: ev.allDay, location: ev.location, color: src.color ?? "", calendar: src.name, local: false, alert: ev.alert ?? (ev.allDay ? -1 : defaultAlert), notes: "" }, inst));
        }
        for (const e of Google.events) {
            const date = x => x.dateTime ? new Date(x.dateTime) : new Date(Number(x.date.slice(0, 4)), Number(x.date.slice(5, 7)) - 1, Number(x.date.slice(8, 10)));
            const start = date(e.start), end = date(e.end);
            if (start < to && end > from)
                out.push({ key: "google:" + e.calendarId + ":" + e.id, id: e.id, title: e.title,
                    start: start, end: end, allDay: !!e.start.date, location: e.location, color: e.color,
                    calendar: e.calendar, local: false, alert: -1, notes: e.notes });
        }
        return out.sort((a, b) => (b.allDay - a.allDay) || (a.start - b.start));
    }

    function eventsOn(day) {
        const from = new Date(day.getFullYear(), day.getMonth(), day.getDate());
        const to = new Date(from.getFullYear(), from.getMonth(), from.getDate() + 1);
        return between(from, to);
    }

    // Days of a month view that have events, as "y-m-d" keys
    function busyDays(first, last) {
        const set = {};
        const dayOf = t => new Date(t.getFullYear(), t.getMonth(), t.getDate());
        for (const e of between(first, new Date(last.getFullYear(), last.getMonth(), last.getDate() + 1))) {
            const d = dayOf(e.start);
            let end = dayOf(e.allDay ? new Date(e.end.getTime() - 1) : e.end);
            if (end < d)
                end = d;
            for (; d <= end; d.setDate(d.getDate() + 1))
                set[d.getFullYear() + "-" + d.getMonth() + "-" + d.getDate()] = true;
        }
        return set;
    }

    function upcoming(count) {
        const now = new Date();
        return between(now, new Date(now.getTime() + 14 * 86400000)).filter(e => !e.allDay ? e.end > now : true).slice(0, count);
    }

    function search(q, limit) {
        const now = new Date();
        const ql = q.toLowerCase();
        return between(new Date(now.getTime() - 7 * 86400000), new Date(now.getTime() + 90 * 86400000)).filter(e => e.title.toLowerCase().includes(ql) || (e.location ?? "").toLowerCase().includes(ql)).slice(0, limit);
    }

    function timeText(e) {
        if (e.allDay)
            return "All day";
        return Qt.formatTime(e.start, "h:mm AP") + (e.end > e.start ? " – " + Qt.formatTime(e.end, "h:mm AP") : "");
    }

    // ---------- Local events ----------

    function add(title, start, end, allDay, alert, notes) {
        const e = { id: Date.now().toString(36) + Math.random().toString(36).slice(2, 6), title: title, start: start.getTime(), end: (end ?? start).getTime(), allDay: !!allDay, alert: alert ?? defaultAlert, notes: notes ?? "" };
        local = local.concat([e]);
        saveLocal();
        return e;
    }
    // Change one of your events (subscribed calendars are read-only)
    function update(id, patch) {
        local = local.map(e => e.id === id ? Object.assign({}, e, patch) : e);
        saveLocal();
    }
    function remove(id) {
        const e = local.find(x => x.id === id);
        local = local.filter(x => x.id !== id);
        saveLocal();
        return e; // for Undo
    }
    function restoreEvent(e) {
        if (e && !local.some(x => x.id === e.id)) {
            local = local.concat([e]);
            saveLocal();
        }
    }

    // "remind me to call mum at 5pm", "dentist tomorrow at 9:30", "stretch in 20 minutes"
    // Returns { title, start: Date } or null.
    function parseQuick(text) {
        let t = text.trim().replace(/^remind me (to )?/i, "").replace(/^add (an? )?(event|reminder) /i, "");
        const now = new Date();
        let start = null;
        let m = t.match(/\s+in\s+(\d+)\s*(min|mins|minutes?|h|hrs?|hours?)\s*$/i);
        if (m) {
            const n = Number(m[1]);
            start = new Date(now.getTime() + n * (/^h/i.test(m[2]) ? 3600000 : 60000));
            t = t.slice(0, m.index);
        } else {
            m = t.match(/\s+(?:(today|tonight|tomorrow|on\s+(mon|tue|wed|thu|fri|sat|sun)\w*)\s+)?at\s+(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?\s*$/i);
            if (!m)
                return null;
            let h = Number(m[3]);
            const min = Number(m[4] ?? 0);
            if (m[5]) {
                const pm = m[5].toLowerCase() === "pm";
                h = (h % 12) + (pm ? 12 : 0);
            } else if (m[1]?.toLowerCase() === "tonight" && h < 12) {
                h += 12;
            }
            if (h > 23 || min > 59)
                return null;
            start = new Date(now.getFullYear(), now.getMonth(), now.getDate(), h, min);
            const day = (m[1] ?? "").toLowerCase();
            if (day === "tomorrow")
                start.setDate(start.getDate() + 1);
            else if (m[2]) {
                const want = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"].indexOf(m[2].toLowerCase());
                let diff = (want - start.getDay() + 7) % 7;
                if (diff === 0 && start <= now)
                    diff = 7;
                start.setDate(start.getDate() + diff);
            } else if (!day && start <= now) {
                start.setDate(start.getDate() + 1); // "at 9" after 9 means tomorrow
            }
            t = t.slice(0, m.index);
        }
        t = t.trim();
        if (t === "")
            return null;
        return { title: t.charAt(0).toUpperCase() + t.slice(1), start: start };
    }

    // ---------- Reminders ----------

    property var alerted: ({}) // key -> true, for this session
    function checkAlerts() {
        const now = new Date();
        const soon = new Date(now.getTime() + 24 * 3600000);
        for (const e of between(new Date(now.getTime() - 3600000), soon)) {
            if (e.alert < 0 || alerted[e.key])
                continue;
            const at = e.start.getTime() - e.alert * 60000;
            // Fire once, within ten minutes of the alert time (not for alerts long past)
            if (now.getTime() >= at && now.getTime() - at < 600000) {
                const a = Object.assign({}, alerted);
                a[e.key] = true;
                alerted = a;
                const when = e.alert === 0 || e.local && e.start <= now ? "Now" : e.allDay ? "Today" : "At " + Qt.formatTime(e.start, "h:mm AP") + (e.alert > 0 ? " · in " + e.alert + " min" : "");
                Quickshell.execDetached(["notify-send", "-a", "Calendar", "-i", "x-office-calendar", "-u", "normal", e.title, when + (e.location ? "\n" + e.location : "")]);
            }
        }
    }
    Timer {
        running: true
        repeat: true
        interval: 30000
        triggeredOnStart: true
        onTriggered: root.checkAlerts()
    }

    // ---------- iCalendar ----------

    // "20261026T090000Z" | "20261026T090000" | "20261026" -> { date, allDay }
    function icsDate(value, params) {
        const v = value.trim();
        const y = Number(v.slice(0, 4)), mo = Number(v.slice(4, 6)) - 1, d = Number(v.slice(6, 8));
        if (v.length === 8 || /VALUE=DATE(?!-)/.test(params))
            return { date: new Date(y, mo, d), allDay: true };
        const h = Number(v.slice(9, 11)), mi = Number(v.slice(11, 13)), s = Number(v.slice(13, 15));
        if (v.endsWith("Z"))
            return { date: new Date(Date.UTC(y, mo, d, h, mi, s)), allDay: false };
        return { date: new Date(y, mo, d, h, mi, s), allDay: false };
    }
    // "-PT15M", "PT1H", "-P1D", "P1DT2H" -> minutes (negative = before)
    function duration(v) {
        const m = v.match(/^([+-])?P(?:(\d+)W)?(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?$/);
        if (!m)
            return null;
        const mins = (Number(m[2] ?? 0) * 7 * 1440) + Number(m[3] ?? 0) * 1440 + Number(m[4] ?? 0) * 60 + Number(m[5] ?? 0);
        return m[1] === "-" ? -mins : mins;
    }

    function parseIcs(text) {
        const lines = text.replace(/\r\n[ \t]/g, "").replace(/\n[ \t]/g, "").split(/\r?\n/);
        const events = [];
        const overrides = []; // moved instances: { uid, recurrence, ev }
        let ev = null, inAlarm = false;
        for (const line of lines) {
            if (line === "BEGIN:VEVENT") {
                ev = { uid: "", title: "(No title)", start: null, end: null, allDay: false, location: "", rrule: null, exdates: [], alert: null, durationMin: null, recurrence: null };
                continue;
            }
            if (!ev)
                continue;
            if (line === "BEGIN:VALARM") {
                inAlarm = true;
                continue;
            }
            if (line === "END:VALARM") {
                inAlarm = false;
                continue;
            }
            if (line === "END:VEVENT") {
                if (ev.start) {
                    if (!ev.end)
                        ev.end = ev.durationMin !== null ? new Date(ev.start.getTime() + ev.durationMin * 60000) : ev.allDay ? new Date(ev.start.getTime() + 86400000) : ev.start;
                    if (ev.recurrence)
                        overrides.push(ev);
                    else
                        events.push(ev);
                }
                ev = null;
                continue;
            }
            const colon = line.indexOf(":");
            if (colon < 0)
                continue;
            const head = line.slice(0, colon), value = line.slice(colon + 1);
            const name = head.split(";")[0].toUpperCase(), params = head.slice(name.length);
            if (inAlarm) {
                if (name === "TRIGGER" && !/RELATED=END/.test(params)) {
                    const mins = duration(value);
                    if (mins !== null && mins <= 0)
                        ev.alert = ev.alert === null ? -mins : Math.max(ev.alert, -mins);
                }
                continue;
            }
            const unescape = s => s.replace(/\\n/gi, "\n").replace(/\\([,;\\])/g, "$1");
            switch (name) {
            case "UID": ev.uid = value; break;
            case "SUMMARY": ev.title = unescape(value); break;
            case "LOCATION": ev.location = unescape(value); break;
            case "DTSTART": { const r = icsDate(value, params); ev.start = r.date; ev.allDay = r.allDay; break; }
            case "DTEND": ev.end = icsDate(value, params).date; break;
            case "DURATION": ev.durationMin = duration(value); break;
            case "RRULE": ev.rrule = value.split(";").reduce((o, p) => { const [k, v] = p.split("="); o[k] = v; return o; }, {}); break;
            case "EXDATE": for (const v of value.split(",")) ev.exdates.push(icsDate(v, params).date.getTime()); break;
            case "RECURRENCE-ID": ev.recurrence = icsDate(value, params).date.getTime(); break;
            case "STATUS": if (value === "CANCELLED") ev.cancelled = true; break;
            }
        }
        // Moved instances replace the original occurrence
        for (const o of overrides) {
            const master = events.find(e => e.uid === o.uid);
            if (master)
                master.exdates.push(o.recurrence);
            if (!o.cancelled)
                events.push(Object.assign({}, o, { recurrence: null, rrule: null }));
        }
        return events.filter(e => !e.cancelled);
    }

    // Occurrences of one event overlapping [from, to)
    function expand(ev, from, to) {
        if (![ev.start, ev.end, from, to].every(d => d && Number.isFinite(d.getTime())) || to <= from || ev.end < ev.start) return [];
        const len = ev.end - ev.start;
        if (!ev.rrule)
            return ev.start < to && ev.end > from || (len === 0 && ev.start >= from && ev.start < to) ? [{ start: ev.start, end: ev.end }] : [];
        const r = ev.rrule;
        const freq = r.FREQ, interval = Number(r.INTERVAL ?? 1);
        if (!Number.isSafeInteger(interval) || interval < 1) return [];
        const count = r.COUNT ? Number(r.COUNT) : Infinity;
        const until = r.UNTIL ? icsDate(r.UNTIL, "").date.getTime() + (r.UNTIL.length === 8 ? 86399999 : 0) : Infinity;
        const days = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];
        const byday = r.BYDAY ? r.BYDAY.split(",").map(s => ({ n: s.length > 2 ? Number(s.slice(0, -2)) : 0, d: days.indexOf(s.slice(-2)) })) : null;
        const bymonthday = r.BYMONTHDAY ? r.BYMONTHDAY.split(",").map(Number) : null;
        const out = [];
        let n = 0;
        const s0 = ev.start;
        const push = d => {
            if (d < s0 || d.getTime() > until || n >= count)
                return false;
            n++;
            if (!(ev.exdates ?? []).includes(d.getTime()) && d.getTime() < to.getTime() && (d.getTime() + len > from.getTime() || (len === 0 && d >= from)))
                out.push({ start: d, end: new Date(d.getTime() + len) });
            return true;
        };
        const at = (y, m, d) => new Date(y, m, d, s0.getHours(), s0.getMinutes(), s0.getSeconds());
        // Nth weekday of a month (n < 0 counts from the end)
        const nthWeekday = (y, m, wd, k) => {
            if (k > 0) {
                const first = new Date(y, m, 1).getDay();
                const d = 1 + (wd - first + 7) % 7 + (k - 1) * 7;
                return d <= new Date(y, m + 1, 0).getDate() ? d : null;
            }
            const lastDay = new Date(y, m + 1, 0);
            const d = lastDay.getDate() - (lastDay.getDay() - wd + 7) % 7 + (k + 1) * 7;
            return d >= 1 ? d : null;
        };
        // Without COUNT, jump close to the window instead of walking from the first occurrence
        let i0 = 0;
        if ((count === Infinity || (freq === "DAILY" && !byday && !bymonthday)) && from > s0) {
            from = new Date(from.getTime());
            const earliest = new Date(from.getTime() - Math.max(0, len));
            const months = (earliest.getFullYear() - s0.getFullYear()) * 12 + earliest.getMonth() - s0.getMonth();
            const steps = { DAILY: (earliest - s0) / (86400000 * interval), WEEKLY: (earliest - s0) / (7 * 86400000 * interval), MONTHLY: months / interval, YEARLY: months / 12 / interval }[freq] ?? 0;
            i0 = Math.max(0, Math.floor(steps) - 2);
        }
        if (count !== Infinity && freq === "DAILY" && !byday && !bymonthday) n = i0;
        for (let i = i0; n < count; i++) {
            let candidates = [];
            if (freq === "DAILY") {
                candidates = [at(s0.getFullYear(), s0.getMonth(), s0.getDate() + i * interval)];
            } else if (freq === "WEEKLY") {
                const weekStart = at(s0.getFullYear(), s0.getMonth(), s0.getDate() - s0.getDay() + i * 7 * interval);
                candidates = (byday ?? [{ d: s0.getDay() }]).map(b => at(weekStart.getFullYear(), weekStart.getMonth(), weekStart.getDate() + b.d)).sort((a, b) => a - b);
            } else if (freq === "MONTHLY") {
                const y = s0.getFullYear(), m = s0.getMonth() + i * interval;
                const base = new Date(y, m, 1);
                const last = new Date(base.getFullYear(), base.getMonth() + 1, 0).getDate();
                const selected = bymonthday ? bymonthday.map(d => d < 0 ? last + d + 1 : d).filter(d => d >= 1 && d <= last) : null;
                const weekdays = byday ? byday.flatMap(b => b.n ? [nthWeekday(base.getFullYear(), base.getMonth(), b.d, b.n)].filter(d => d !== null) : Array.from({length: last}, (_, k) => k + 1).filter(d => at(base.getFullYear(), base.getMonth(), d).getDay() === b.d)) : null;
                const dates = weekdays ? weekdays.filter(d => !selected || selected.includes(d)) : (selected ?? [s0.getDate()]);
                candidates = [...new Set(dates)].filter(d => d >= 1 && d <= last).map(d => at(base.getFullYear(), base.getMonth(), d));
                if (!Number.isFinite(base.getTime()) || base >= to || base.getTime() > until) break;
                candidates.sort((a, b) => a - b);
            } else if (freq === "YEARLY") {
                const year = s0.getFullYear() + i * interval;
                const candidate = at(year, s0.getMonth(), s0.getDate());
                if (!Number.isFinite(candidate.getTime()) || candidate >= to || candidate.getTime() > until) break;
                candidates = candidate.getMonth() === s0.getMonth() && candidate.getDate() === s0.getDate() ? [candidate] : [];
            } else {
                break;
            }
            if (candidates.length > 0 && (!Number.isFinite(candidates[0].getTime()) || candidates[0].getTime() >= to.getTime()))
                break;
            let stop = false;
            for (const c of candidates)
                if (!push(c) && c.getTime() > until) {
                    stop = true;
                    break;
                }
            if (stop)
                break;
        }
        return out;
    }

    // ---------- Fetching and storage ----------

    function refresh() {
        for (const src of sources)
            if (src.enabled !== false)
                fetch(src);
    }
    function fetch(src) {
        const isFile = src.url.startsWith("/") || src.url.startsWith("file://");
        const url = src.url.replace(/^webcal:/, "https:");
        const p = fetcher.createObject(root, { url: src.url, command: isFile ? ["cat", url.replace("file://", "")] : ["curl", "-sfL", "--max-time", "20", url] });
        p.running = true;
    }
    Component {
        id: fetcher
        Process {
            id: proc
            property string url
            stdout: StdioCollector {
                onStreamFinished: {
                    if (text.includes("BEGIN:VCALENDAR")) {
                        const r = Object.assign({}, root.remote);
                        r[proc.url] = root.parseIcs(text);
                        root.remote = r;
                        root.lastError = "";
                    }
                }
            }
            onExited: code => {
                if (code !== 0)
                    root.lastError = "Couldn't load a calendar. Check the link in Settings → Calendar.";
                proc.destroy();
            }
        }
    }
    Timer {
        running: root.sources.length > 0
        repeat: true
        interval: 15 * 60000
        onTriggered: root.refresh()
    }

    function addSource(name, url) {
        const colors = ["#6D7CDF", "#D05F43", "#409D48", "#BB5FAA", "#BE7100", "#1D9999"];
        sources = sources.concat([{ name: name || "Calendar", url: url.trim(), color: colors[sources.length % colors.length], enabled: true }]);
        saveSources();
        fetch(sources[sources.length - 1]);
    }
    function removeSource(url) {
        sources = sources.filter(s => s.url !== url);
        const r = Object.assign({}, remote);
        delete r[url];
        remote = r;
        saveSources();
    }
    function setSource(url, patch) {
        sources = sources.map(s => s.url === url ? Object.assign({}, s, patch) : s);
        saveSources();
    }

    function saveSources() {
        sourcesFile.setText(JSON.stringify({ sources: sources, defaultAlert: defaultAlert }, null, 2));
        sourcesPrivate.running = true;
    }
    // Calendar links can carry secret tokens: owner-only
    Process {
        id: sourcesPrivate
        command: ["chmod", "600", root.configPath]
    }
    function saveLocal() {
        localSaver.payload = JSON.stringify(local, null, 1);
        if (localSaver.running)
            localSaver.again = true; // save again once this write finishes
        else
            localSaver.running = true;
    }
    // Owner-only: events can be private
    Process {
        id: localSaver
        property string payload
        property bool again: false
        command: ["sh", "-c", "umask 077; mkdir -p \"$(dirname \"$1\")\"; cat > \"$1\"", "sh", root.statePath]
        stdinEnabled: true
        onStarted: {
            write(payload);
            stdinEnabled = false;
        }
        onExited: {
            stdinEnabled = true;
            if (again) {
                again = false;
                running = true;
            }
        }
    }

    FileView {
        id: sourcesFile
        path: root.configPath
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const j = JSON.parse(text());
                root.sources = j.sources ?? [];
                root.defaultAlert = j.defaultAlert ?? 10;
                root.refresh();
            } catch (e) {}
        }
    }
    FileView {
        path: root.statePath
        onLoaded: {
            try {
                root.local = JSON.parse(text());
            } catch (e) {}
        }
    }

    IpcHandler {
        target: "calendar"
        function remind(text: string): string {
            const q = root.parseQuick(text);
            if (!q)
                return "Couldn't read a time. Try “call mum at 5pm” or “stretch in 20 minutes”.";
            root.add(q.title, q.start, q.start, false, 0, "");
            return q.title + " — " + Qt.formatDateTime(q.start, "ddd h:mm AP");
        }
        function refresh(): void { root.refresh(); }
    }
}
