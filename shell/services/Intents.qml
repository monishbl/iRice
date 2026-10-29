pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import qs.common

// Spotlight in plain words: what to do with a typed phrase. Each match is a
// result row on top of the search results:
//   tasks      "remind me to call mum at 6", "todo buy milk friday"
//   notes      "note gate code is 4512", "save https://… recipe"
//   events     "meeting with Sam tomorrow 3pm", "event dentist on fri at 9"
//   timers     "timer 10 min", "tea timer 4m", "alarm 7am", "wake me up at 6:30"
//   system     "caffeine 1h", "dark mode", "wifi off", "bluetooth on",
//              "focus 30m", "night light on", "volume 40", "mute",
//              "brightness 70", "lock", "sleep"
// Rows: { section, title, subtitle, glyph, run }.
Singleton {
    id: root

    function row(title, subtitle, glyph, run) {
        return { section: "Do", title: title, subtitle: subtitle, glyph: glyph, run: run };
    }
    function done(icon, title, subtitle, color) {
        Island.post({ kind: "custom", icon: icon, title: title, subtitle: subtitle, color: color ?? Theme.accent });
    }

    function rows(raw) {
        const t = raw.trim();
        if (t.length < 2)
            return [];
        const out = [];
        tasks(t, out);
        notes(t, out);
        events(t, out);
        timers(t, out);
        system(t, out);
        return out;
    }

    // ── Tasks and reminders ─────────────────────────────────────────────
    function tasks(t, out) {
        const m = t.match(/^(remind me(?: to)?|to-?do|task|add (?:a )?(?:task|to-?do))\s*:?\s+(.+)$/i);
        if (!m)
            return;
        const p = Todos.parse(m[2]);
        if (!p.text)
            return;
        out.push(row("Add to-do: " + p.text, p.due ? "Due " + Todos.dueLabel({ due: p.due, allDay: p.allDay }) : "Reminders · no due date", "check-circle", () => {
            if (!Todos.add(m[2]))
                return;
            done("check-circle", Config.options.google.taskListId ? "Sending to Google Tasks" : "To-do added", p.text + (p.due ? " · " + Todos.dueLabel({ due: p.due, allDay: p.allDay }) : ""), Theme.accents.orange);
        }));
        // A reminder with a time can also go in the calendar
        if (/^remind/i.test(m[1]) && p.due && !p.allDay)
            out.push(row("Add to the calendar instead", Qt.formatDateTime(new Date(p.due), "ddd d MMM, h:mm AP"), "calendar-plus", () => {
                Calendar.add(p.text, new Date(p.due), new Date(p.due), false, 0, "");
                done("calendar-plus", "Reminder added", p.text + " · " + Qt.formatDateTime(new Date(p.due), "ddd h:mm AP"), Theme.accents.red);
            }));
    }

    // ── Notes ───────────────────────────────────────────────────────────
    function notes(t, out) {
        const n = t.match(/^(?:note|jot(?: down)?|write down|new note)\s*:?\s+([\s\S]+)$/i);
        if (n) {
            const title = Notes.titleOf(n[1]);
            out.push(row("New note: " + title, "Saved to " + Notes.folder.replace(Quickshell.env("HOME"), "~"), "note-pencil", () => Notes.create(n[1], false)));
            out.push(row("New note and open it", title, "note-pencil", () => Notes.create(n[1], true)));
            return;
        }
        const s = t.match(/^(?:save|keep|bookmark)\s+([\s\S]+)$/i);
        if (s) {
            const url = s[1].match(/\bhttps?:\/\/\S+/i)?.[0] ?? s[1].match(/\b[\w-]+(?:\.[\w-]+)+\.[a-z]{2,}(?:\/\S*)?/i)?.[0];
            if (url) {
                const full = /^https?:/i.test(url) ? url : "https://" + url;
                const words = s[1].replace(url, "").trim();
                out.push(row("Save link" + (words ? ": " + words : ""), full, "bookmark-simple", () => Notes.saveLink(full, words)));
            } else {
                out.push(row("Save as a note: " + Notes.titleOf(s[1]), "Saved to your Notes folder", "note-pencil", () => Notes.create(s[1], false)));
            }
            return;
        }
        if (/^(?:open )?notes?$/i.test(t))
            out.push(row("Open the Notes folder", Notes.folder.replace(Quickshell.env("HOME"), "~") + " · " + Notes.count + " notes", "folder-open", () => Notes.openFolder()));
    }

    // The calendar's parser reads "… [today | tomorrow | on <day>] at <time>"
    // at the end; put everyday phrasings into that shape:
    //   "tomorrow 3pm" -> "tomorrow at 3pm", "at 3pm tomorrow" -> "tomorrow at 3pm",
    //   "fri at 9" -> "on fri at 9"
    function normalizeWhen(text) {
        const time = "(\\d{1,2}(?:[:.]\\d{2})?\\s*(?:am|pm)?)";
        const day = "(today|tonight|tomorrow|(?:on\\s+)?(?:mon|tue|wed|thu|fri|sat|sun)\\w*)";
        let t = text.trim();
        t = t.replace(new RegExp("\\s+at\\s+" + time + "\\s+" + day + "$", "i"), " $2 at $1");
        t = t.replace(new RegExp("\\s+" + day + "\\s+" + time + "$", "i"), " $1 at $2");
        if (!/\sat\s+\d/i.test(t))
            t = t.replace(/\s+(\d{1,2}(?:[:.]\d{2})?\s*(?:am|pm))$/i, " at $1");
        if (!/\son\s+(?:mon|tue|wed|thu|fri|sat|sun)/i.test(t))
            t = t.replace(/\s+(mon|tue|wed|thu|fri|sat|sun)(\w*)\s+at\s+/i, " on $1$2 at ");
        return t;
    }

    // ── Calendar events ─────────────────────────────────────────────────
    function events(t, out) {
        const lead = /^(?:event|add (?:an? )?event|meeting|meet|call|lunch|dinner|breakfast|appointment|schedule)\b/i.test(t);
        const withWhen = /\bwith\b/i.test(t) && /\b(?:at|on|today|tomorrow|tonight|mon|tue|wed|thu|fri|sat|sun)\w*\b/i.test(t);
        if (!lead && !withWhen)
            return;
        const r = Calendar.parseQuick(normalizeWhen(t.replace(/^(?:event|add (?:an? )?event|schedule)\s*:?\s+/i, "")));
        if (!r)
            return;
        out.push(row("Add event: " + r.title, Qt.formatDateTime(r.start, "dddd d MMM, h:mm AP"), "calendar-plus", () => {
            Calendar.add(r.title, r.start, new Date(r.start.getTime() + 3600000), false, Calendar.defaultAlert, "");
            done("calendar-plus", "Event added", r.title + " · " + Qt.formatDateTime(r.start, "ddd h:mm AP"), Theme.accents.red);
        }));
    }

    // ── Timers and alarms ───────────────────────────────────────────────
    function timers(t, out) {
        // "timer 10 min", "set a timer for 5m pasta", "pasta timer 10 min", "10 min timer"
        const m = t.match(/^(?:set (?:a )?)?timer(?: for)?\s+(.+)$/i) || t.match(/^(.+?)\s+timer(?:\s+(.+))?$/i);
        if (m) {
            const text = m[2] ? m[1] + " " + m[2] : m[1];
            const secs = Alarms.parseDuration(text);
            if (secs > 0) {
                const label = text.replace(/(\d+(?:\.\d+)?)\s*(h|hr|hrs|hours?|m|min|mins|minutes?|s|sec|secs|seconds?)\b/gi, "").replace(/^\s*for\s+/i, "").replace(/\s+/g, " ").trim();
                out.push(row("Start a " + Alarms.describe(secs) + " timer" + (label ? ": " + label : ""), "Ends at " + Qt.formatTime(new Date(Date.now() + secs * 1000), "h:mm AP"), "timer", () => {
                    Alarms.addTimer(secs, label);
                    done("timer", "Timer started", Alarms.describe(secs) + (label ? " · " + label : ""), Theme.accents.orange);
                }));
            }
            return;
        }
        const a = t.match(/^(?:set (?:an )?alarm|alarm|wake me(?: up)?)(?: at| for)?\s+(\d{1,2}(?:[:.]\d{2})?\s*(?:am|pm)?)(?:\s+(.+))?$/i);
        if (a) {
            const when = Alarms.parseClock(a[1]);
            if (when) {
                const label = (a[2] || "").trim();
                const day = when.getDate() === new Date().getDate() ? "today" : "tomorrow";
                out.push(row("Set an alarm for " + Qt.formatTime(when, "h:mm AP") + " " + day + (label ? ": " + label : ""), "Sounds even with interface sounds off", "alarm", () => {
                    Alarms.addAlarm(when, label);
                    done("alarm", "Alarm set", Qt.formatTime(when, "h:mm AP") + " " + day, Theme.accents.orange);
                }));
            }
            return;
        }
        if (/^(?:timers?|alarms?)$/i.test(t)) {
            if (Alarms.list.length === 0)
                out.push(row("No timers or alarms", "Try “timer 10 min” or “alarm 7am”", "timer", () => {}));
            for (const x of Alarms.list)
                out.push(row("Cancel " + (x.kind === "timer" ? "timer" : "alarm") + (x.label ? ": " + x.label : ""), (x.kind === "timer" ? "Ends " : "Rings ") + Qt.formatDateTime(new Date(x.at), "ddd h:mm AP"), x.kind === "timer" ? "timer" : "alarm", () => Alarms.cancel(x.id)));
        }
    }

    // ── System ──────────────────────────────────────────────────────────
    function system(t, out) {
        const low = t.toLowerCase().replace(/\s+/g, " ");
        let m;
        if ((m = low.match(/^(?:caffeine|keep awake|stay awake)(?: (on|off))?(?: (?:for )?(.+))?$/))) {
            if (m[1] === "off")
                out.push(row("Turn Caffeine off", "Sleeps when idle again", "coffee", () => System.setKeepAwake(false, 0)));
            else {
                const mins = m[2] ? Math.round(Alarms.parseDuration(m[2]) / 60) : 0;
                out.push(row(mins > 0 ? "Caffeine for " + Alarms.describe(mins * 60) : "Turn Caffeine on", "The screen stays awake" + (mins > 0 ? "" : " until you turn it off"), "coffee", () => System.setKeepAwake(true, mins)));
            }
            return;
        }
        if ((m = low.match(/^(?:turn on |toggle |switch to )?(dark|light)(?: mode| appearance)?$/))) {
            out.push(row("Switch to " + m[1] + " appearance", "", m[1] === "dark" ? "moon" : "sun", () => Config.options.appearance.mode = m[1]));
            return;
        }
        if ((m = low.match(/^(?:turn )?wi-?fi (on|off)$/) || low.match(/^turn (on|off) wi-?fi$/))) {
            const on = m[1] === "on";
            out.push(row("Turn Wi-Fi " + m[1], "", on ? "wifi-high" : "wifi-slash", () => Networking.wifiEnabled = on));
            return;
        }
        if ((m = low.match(/^(?:turn )?bluetooth (on|off)$/) || low.match(/^turn (on|off) bluetooth$/))) {
            const on = m[1] === "on";
            out.push(row("Turn Bluetooth " + m[1], "", "bluetooth", () => {
                if (Bluetooth.defaultAdapter)
                    Bluetooth.defaultAdapter.enabled = on;
            }));
            return;
        }
        if ((m = low.match(/^(?:focus|dnd|do not disturb)(?: (on|off))?(?: (?:for )?(.+))?$/))) {
            if (m[1] === "off")
                out.push(row("Turn Focus off", "", "moon", () => Focus.deactivate()));
            else {
                const secs = m[2] ? Alarms.parseDuration(m[2]) : 0;
                out.push(row("Do Not Disturb" + (secs > 0 ? " for " + Alarms.describe(secs) : ""), secs > 0 ? "Until " + Qt.formatTime(new Date(Date.now() + secs * 1000), "h:mm AP") : "Until you turn it off", "moon", () => Focus.activate("dnd", secs * 1000, false)));
            }
            return;
        }
        if ((m = low.match(/^night ?light(?: (on|off))?$/))) {
            const on = m[1] ? m[1] === "on" : !System.nightLight;
            out.push(row("Turn Night Light " + (on ? "on" : "off"), "", "sun-dim", () => System.nightLight = on));
            return;
        }
        if ((m = low.match(/^(?:set )?volume (?:to )?(\d{1,3})%?$/))) {
            const v = Math.min(100, parseInt(m[1]));
            out.push(row("Set the volume to " + v + "%", "", "speaker-high", () => Audio.setVolume(v / 100)));
            return;
        }
        if (/^(?:mute|unmute)$/.test(low)) {
            out.push(row(low === "mute" ? "Mute" : "Unmute", "", low === "mute" ? "speaker-slash" : "speaker-high", () => {
                if ((low === "mute") !== Audio.muted)
                    Audio.toggleMute();
            }));
            return;
        }
        if ((m = low.match(/^(?:set )?brightness (?:to )?(\d{1,3})%?$/))) {
            const v = Math.min(100, parseInt(m[1]));
            out.push(row("Set the brightness to " + v + "%", "", "sun", () => Brightness.set(v / 100)));
            return;
        }
        const power = ({ "lock": ["lock", "Lock the screen", "lock"], "lock screen": ["lock", "Lock the screen", "lock"], "sleep": ["sleep", "Sleep", "moon-stars"], "log out": ["logout", "Log out", "sign-out"], "restart": ["restart", "Restart", "arrow-clockwise"], "shut down": ["shutdown", "Shut down", "power"], "shutdown": ["shutdown", "Shut down", "power"] })[low];
        if (power)
            out.push(row(power[1], "", power[2], () => System.power(power[0])));
    }
}
