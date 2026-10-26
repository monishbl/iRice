pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import "../common/emoji.js" as Emoji

// Spotlight's result providers. query() returns rows:
//   { section, title, subtitle, icon (image path) | glyph (Phosphor name), run() }
// Prefixes: ">" runs a command, ";" clipboard history, ":" emoji, "/" files,
// "?" asks AI. Rows with ai: true answer inline instead of closing Spotlight.
Singleton {
    id: root

    property var files: [] // [{ type, path }] for fileQuery
    property string fileQuery: ""

    // Fuzzy match score, 0 = no match. Prefers whole-string prefix, then word
    // prefix, then substring, then in-order subsequence.
    function score(text, q) {
        if (!text)
            return 0;
        const t = text.toLowerCase();
        if (t.startsWith(q))
            return 100 - Math.min(20, t.length - q.length) * 0.5;
        if (t.split(/[\s\-_.]+/).some(w => w.startsWith(q)))
            return 80;
        if (t.includes(q))
            return 60;
        let i = 0, gaps = 0;
        for (const c of t) {
            if (c === q[i])
                i++;
            else if (i > 0)
                gaps++;
            if (i === q.length)
                return Math.max(1, 40 - gaps);
        }
        return 0;
    }

    // Arithmetic only: digits, operators, parentheses, a few functions.
    function calculate(q) {
        const expr = q.replace(/\s+/g, "").replace(/×/g, "*").replace(/÷/g, "/").replace(/,/g, "");
        if (!/[0-9]/.test(expr) || !/[-+*/^%()]|sqrt|pi/.test(expr))
            return null;
        if (!/^(?:[0-9.+\-*/%^()]|sqrt|sin|cos|tan|log|ln|pi|e)+$/.test(expr))
            return null;
        const js = expr.replace(/\^/g, "**").replace(/sqrt/g, "Math.sqrt").replace(/sin/g, "Math.sin").replace(/cos/g, "Math.cos").replace(/tan/g, "Math.tan").replace(/ln/g, "Math.log").replace(/log/g, "Math.log10").replace(/pi/g, "Math.PI").replace(/(^|[^a-zA-Z.])e(?![a-zA-Z])/g, "$1Math.E");
        try {
            const v = Function('"use strict"; return (' + js + ");")();
            if (typeof v !== "number" || !isFinite(v))
                return null;
            return Math.round(v * 1e10) / 1e10;
        } catch (e) {
            return null;
        }
    }

    readonly property var actions: [
        { title: "Lock Screen", glyph: "lock", keywords: "lock", run: () => System.power("lock") },
        { title: "Sleep", glyph: "moon", keywords: "suspend sleep", run: () => System.power("sleep") },
        { title: "Restart…", glyph: "arrow-clockwise", keywords: "reboot restart", run: () => Panels.show("session") },
        { title: "Shut Down…", glyph: "power", keywords: "shutdown power off", run: () => Panels.show("session") },
        { title: "Log Out…", glyph: "sign-out", keywords: "logout exit", run: () => Panels.show("session") },
        { title: "Toggle Dark Mode", glyph: "circle-half", keywords: "dark light appearance theme", run: () => Config.options.appearance.mode = Theme.dark ? "light" : "dark" },
        { title: "Control Center", glyph: "sliders-horizontal", keywords: "control center settings toggles", run: () => Panels.show("controlCenter") },
        { title: "Notification Center", glyph: "bell", keywords: "notifications calendar", run: () => Panels.show("notificationCenter") },
        { title: "Ask AI", glyph: "sparkle", keywords: "ai chat claude assistant gpt gemini", run: () => Panels.show("sidebar") },
        { title: "Ask AI About the Screen", glyph: "sparkle", keywords: "ai ask screen screenshot explain what is this error", run: () => ScreenTools.select("ask") },
        { title: "Translate", glyph: "translate", keywords: "translate translator language", run: () => Panels.show("sidebar") },
        { title: "System Settings", glyph: "gear-six", keywords: "settings preferences config options", run: () => Panels.openSettings() },
        { title: "Take Screenshot", glyph: "camera", keywords: "screenshot capture screen grab", run: () => ScreenTools.select("screenshot") },
        { title: "Copy Text from Screen", glyph: "text-t", keywords: "ocr text recognize copy screen", run: () => ScreenTools.select("ocr") },
        { title: "Record Screen", glyph: "record", keywords: "record recording video screencast", run: () => ScreenTools.select("record") },
        { title: "Keyboard Shortcuts", glyph: "keyboard", keywords: "shortcuts keybinds cheatsheet keys", run: () => Panels.show("cheatsheet") },
        { title: "Periodic Table", glyph: "atom", keywords: "periodic table elements chemistry", run: () => Panels.show("cheatsheet") },
        { title: "On-Screen Keyboard", glyph: "keyboard", keywords: "osk keyboard virtual touch", run: () => Panels.show("keyboard") },
        { title: "Clipboard History", glyph: "clipboard-text", keywords: "clipboard history paste copied pins", run: () => Panels.show("clipboard") },
        { title: "Overlay Widgets", glyph: "squares-four", keywords: "overlay widgets notes crosshair resources game", run: () => Panels.show("overlay") },
        { title: "Identify Song", glyph: "music-notes", keywords: "song recognize shazam songrec music what playing", run: () => System.identifySong() },
        { title: "Check for Updates", glyph: "arrow-circle-up", keywords: "update upgrade pacman packages", run: () => Updates.count > 0 ? Updates.install() : Quickshell.execDetached(["notify-send", "-a", "Software Update", "Your system is up to date"]) },
        { title: "Change Wallpaper", glyph: "image", keywords: "wallpaper background desktop picture", run: () => Panels.show("wallpaper") },
        { title: "Stage Manager", glyph: "squares-four", keywords: "stage manager windows group strip", run: () => Config.options.stageManager.enabled = !Config.options.stageManager.enabled },
        { title: "App Library", glyph: "squares-four", keywords: "apps launchpad library all applications grid", run: () => Panels.show("launchpad") },
        { title: "Mission Control", glyph: "squares-four", keywords: "overview workspaces windows expose", run: () => Panels.show("missionControl") }
    ]

    function query(text) {
        const raw = text.trim();
        const q = raw.toLowerCase();
        if (q === "")
            return [];

        if (raw.startsWith("?")) {
            const aq = raw.slice(1).trim();
            return aq === "" ? [] : [aiRow(aq)];
        }

        if (raw.startsWith(">")) {
            const cmd = raw.slice(1).trim();
            return cmd === "" ? [] : [{ section: "Command", title: cmd, subtitle: "Run in terminal", glyph: "terminal-window", run: () => Quickshell.execDetached(["foot", "-H", "sh", "-c", cmd]) }];
        }

        if (raw.startsWith(":")) {
            const eq = raw.slice(1).trim().toLowerCase();
            if (eq === "")
                return [];
            const skin = eq.includes("skin");
            return Emoji.list.filter(e => (skin || !e[1].includes("skin tone")) && e[1].includes(eq)).sort((a, b) => (b[1].split(" ").includes(eq) - a[1].split(" ").includes(eq)) || (a[1].length - b[1].length)).slice(0, 40).map(e => ({
                section: "Emoji",
                title: e[1].replace(/^./, c => c.toUpperCase()),
                subtitle: "",
                emoji: e[0],
                run: () => Quickshell.execDetached(["wl-copy", e[0]])
            }));
        }

        if (raw.startsWith("/")) {
            const fq = raw.slice(1).trim();
            if (fq.length < 2)
                return [];
            if (fq !== fileQuery) {
                fileQuery = fq;
                fileDebounce.restart();
            }
            const home = Quickshell.env("HOME");
            return files.map(f => ({
                section: "Files",
                title: f.path.split("/").pop(),
                subtitle: f.path.slice(0, f.path.lastIndexOf("/")).replace(home, "~"),
                icon: IconLookup.path(f.type === "d" ? "folder" : iconForFile(f.path), "text-x-generic"),
                run: () => Qt.openUrlExternally("file://" + f.path)
            }));
        }

        if (raw.startsWith(";")) {
            return Clipboard.filtered(raw.slice(1), "all").slice(0, 30).map(r => ({
                section: r.pinned ? "Pinned" : "Clipboard",
                title: r.image ? "Image" : r.text.replace(/\s+/g, " ").slice(0, 120),
                subtitle: "",
                glyph: r.image ? "image" : "clipboard-text",
                run: () => Clipboard.paste(r.pinned ? r.pin : r.item, false)
            }));
        }

        const out = [];

        const value = calculate(raw);
        if (value !== null)
            out.push({ section: "Calculator", title: String(value), subtitle: raw + " =", glyph: "calculator", run: () => Quickshell.execDetached(["wl-copy", String(value)]) });

        // Reminders in plain words: "remind me to call mum at 5pm", "stretch in 20 minutes"
        if (/^(remind|add (an? )?(event|reminder))/i.test(raw) || /\s(at\s+\d|in\s+\d+\s*(m|h))/i.test(raw)) {
            const r = Calendar.parseQuick(raw);
            if (r)
                out.push({ section: "Calendar", title: "Add reminder: " + r.title, subtitle: Qt.formatDateTime(r.start, "ddd d MMM, h:mm AP"), glyph: "bell-ringing", run: () => {
                        Calendar.add(r.title, r.start, r.start, false, 0, "");
                        Quickshell.execDetached(["notify-send", "-a", "Calendar", "-i", "x-office-calendar", "Reminder added", r.title + " · " + Qt.formatDateTime(r.start, "ddd h:mm AP")]);
                    } });
        }

        const apps = DesktopEntries.applications.values.filter(a => !a.noDisplay).map(a => ({
            entry: a,
            s: Math.max(score(a.name, q), score(a.genericName, q) * 0.8, score((a.keywords ?? []).join(" "), q) * 0.6, score(a.id, q) * 0.7)
        })).filter(x => x.s >= 30).sort((a, b) => b.s - a.s).slice(0, 8);
        for (const x of apps)
            out.push({ section: "Applications", title: x.entry.name, subtitle: x.entry.genericName || x.entry.comment || "", icon: IconLookup.path(x.entry.icon, "application-x-executable"), run: () => x.entry.execute() });

        const acts = actions.map(a => ({ a: a, s: Math.max(score(a.title, q), score(a.keywords, q) * 0.9) })).filter(x => x.s > 0).sort((a, b) => b.s - a.s).slice(0, 4);
        for (const x of acts)
            out.push({ section: "Actions", title: x.a.title, subtitle: "", glyph: x.a.glyph, run: x.a.run });

        if (q.length >= 3)
            for (const e of Calendar.search(raw, 3))
                out.push({ section: "Calendar", title: e.title, subtitle: Qt.formatDate(e.start, "ddd d MMM") + " · " + Calendar.timeText(e), glyph: "calendar-blank", run: () => {
                        Panels.ncTab = 0;
                        Panels.show("notificationCenter");
                    } });

        if (raw.length >= 3)
            out.push(aiRow(raw));
        out.push({ section: "Web", title: "Search the web for “" + raw + "”", subtitle: "", glyph: "globe", run: () => Qt.openUrlExternally("https://duckduckgo.com/?q=" + encodeURIComponent(raw)) });

        // Best app or action becomes the top hit
        const top = out.findIndex(r => r.section === "Applications" || r.section === "Actions" || r.section === "Calculator");
        if (top > 0 || (top === 0 && out[0].section !== "Calculator")) {
            const hit = Object.assign({}, out[top], { section: "Top Hit" });
            out.splice(top, 1);
            out.unshift(hit);
        }
        return out;
    }

    function aiRow(question) {
        return { section: "Ask AI", title: "Ask " + Ai.model.name.replace(/ \(.*\)$/, ""), subtitle: "“" + question + "”", glyph: "sparkle", ai: true, question: question, run: () => Ai.quickAsk(question) };
    }

    function iconForFile(path) {
        const ext = path.split(".").pop().toLowerCase();
        if (["png", "jpg", "jpeg", "webp", "gif", "svg"].includes(ext)) return "image-x-generic";
        if (["mp4", "mkv", "webm", "mov"].includes(ext)) return "video-x-generic";
        if (["mp3", "flac", "ogg", "wav", "m4a"].includes(ext)) return "audio-x-generic";
        if (ext === "pdf") return "application-pdf";
        if (["zip", "tar", "gz", "xz", "7z", "rar"].includes(ext)) return "package-x-generic";
        return "text-x-generic";
    }

    Timer {
        id: fileDebounce
        interval: 200
        onTriggered: {
            fileProc.running = false;
            fileProc.command = ["sh", "-c", "find \"$HOME\" -maxdepth 6 \\( -name '.*' -o -name node_modules -o -name target \\) -prune -o -iname \"*$1*\" -printf '%y\\t%p\\n' 2>/dev/null | head -25", "sh", root.fileQuery];
            fileProc.running = true;
        }
    }
    Process {
        id: fileProc
        stdout: StdioCollector {
            onStreamFinished: root.files = text.split("\n").filter(l => l.includes("\t")).map(l => ({ type: l.slice(0, l.indexOf("\t")), path: l.slice(l.indexOf("\t") + 1) }))
        }
    }

    function refreshClipboard() {
        Clipboard.refresh();
    }
}
