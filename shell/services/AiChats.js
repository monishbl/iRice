.pragma library

// Chat history for the AI sidebar (services/Ai.qml): the pure parts, tested
// in tests/ai_chats.test.cjs. Each chat is a session file,
//   ai-chats/<id>.json  { id, title, created, updated, model, modelName, pinned, rows, history }
// and ai-chats/index.json lists them lightly, so the history list never opens
// every file:
//   { version, current, trash, chats: [{ id, title, created, updated, model, modelName, pinned, count, preview }] }
// (times in ms; `model` is the model's key, `modelName` what it was called;
// `trash` holds chats deleted with Undo still offered, removed at the next start)

var TITLE_MAX = 60;
var PREVIEW_MAX = 300;
var MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
var DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
var DAY = 86400000;

// File names are ids: only these characters, so a damaged index can't name
// a path outside the folder
function validId(id) {
    return typeof id === "string" && /^[a-z0-9-]{1,64}$/.test(id);
}
function newId(now, random) {
    return now.toString(36) + "-" + Math.floor(random * 1679616).toString(36);
}

// One line, spaces collapsed
function oneLine(text) {
    return String(text ?? "").replace(/\s+/g, " ").trim();
}

// The first thing the person asked, on one line, cut at a word near 60
// characters. (No model writes titles: that would send the chat again.)
function titleFrom(text) {
    const line = String(text ?? "").split("\n").map(oneLine).find(l => l !== "") ?? "";
    if (line === "")
        return "New chat";
    if (line.length <= TITLE_MAX)
        return line;
    const cut = line.slice(0, TITLE_MAX);
    const space = cut.lastIndexOf(" ");
    return (space > TITLE_MAX * 0.6 ? cut.slice(0, space) : cut).replace(/[\s,.;:!?-]+$/, "") + "…";
}
function cleanTitle(text) {
    return oneLine(text).slice(0, 120);
}

// What search reads besides the title: the words of the chat, on one line
function previewOf(rows) {
    let out = "";
    for (const r of rows ?? []) {
        if (r.role !== "user" && r.role !== "assistant")
            continue;
        out += (out ? " " : "") + oneLine(r.text);
        if (out.length >= PREVIEW_MAX)
            break;
    }
    return out.slice(0, PREVIEW_MAX);
}
function messageCount(rows) {
    return (rows ?? []).filter(r => r.role === "user" || r.role === "assistant").length;
}
// A chat is kept once the person has sent something in it
function worthKeeping(rows) {
    return (rows ?? []).some(r => r.role === "user");
}

// Pictures are never stored: a saved turn says there was one
function stripPictures(history) {
    return (history ?? []).map(h => {
        const n = (h.images ?? (h.image ? [h.image] : [])).length;
        if (!n)
            return h;
        return { role: h.role, text: "[" + (n === 1 ? "A picture was" : n + " pictures were") + " attached here]\n" + h.text };
    });
}

// The whole session, ready to write
function session(meta, rows, history) {
    return {
        id: meta.id,
        title: meta.title,
        created: meta.created,
        updated: meta.updated,
        model: meta.model ?? "",
        modelName: meta.modelName ?? "",
        pinned: !!meta.pinned,
        rows: rows,
        history: stripPictures(history)
    };
}
// Its line in the index
function entryOf(s) {
    return {
        id: s.id,
        title: s.title || "New chat",
        created: s.created,
        updated: s.updated,
        model: s.model ?? "",
        modelName: s.modelName ?? "",
        pinned: !!s.pinned,
        count: messageCount(s.rows),
        preview: previewOf(s.rows)
    };
}

function byNewest(a, b) {
    return b.updated - a.updated;
}
// The index with this entry added or replaced, newest first
function upsert(chats, entry) {
    return (chats ?? []).filter(c => c.id !== entry.id).concat([entry]).sort(byNewest);
}
function without(chats, id) {
    return (chats ?? []).filter(c => c.id !== id);
}
function patch(chats, id, fields) {
    return (chats ?? []).map(c => c.id === id ? Object.assign({}, c, fields) : c);
}

// A tolerant read of index.json: anything unusable is left out
function parseIndex(text) {
    let j = null;
    try {
        j = JSON.parse(text);
    } catch (e) {
        return null;
    }
    if (!j || typeof j !== "object" || !Array.isArray(j.chats))
        return null;
    const seen = {};
    const chats = j.chats.filter(c => {
        if (!c || !validId(c.id) || seen[c.id] || typeof c.updated !== "number")
            return false;
        seen[c.id] = true;
        return true;
    }).map(c => ({
        id: c.id,
        title: typeof c.title === "string" && c.title !== "" ? c.title : "New chat",
        created: typeof c.created === "number" ? c.created : c.updated,
        updated: c.updated,
        model: typeof c.model === "string" ? c.model : "",
        modelName: typeof c.modelName === "string" ? c.modelName : "",
        pinned: c.pinned === true,
        count: typeof c.count === "number" ? c.count : 0,
        preview: typeof c.preview === "string" ? c.preview : ""
    })).sort(byNewest);
    return {
        current: validId(j.current) && seen[j.current] ? j.current : "",
        trash: (Array.isArray(j.trash) ? j.trash : []).filter(validId),
        chats: chats
    };
}
function indexText(current, chats, trash) {
    return JSON.stringify({ version: 1, current: current || "", trash: trash ?? [], chats: chats ?? [] });
}

// The old single chat (ai-chat.json) as the first session, or null when it
// had nothing the person sent
function migrate(old, id, now) {
    const rows = old?.rows ?? [];
    if (!worthKeeping(rows))
        return null;
    const first = rows.find(r => r.role === "user");
    const named = rows.find(r => r.role === "assistant" && r.model);
    return session({ id: id, title: titleFrom(first.text), created: now, updated: now, model: "", modelName: named?.model ?? "", pinned: false }, rows, old.history ?? []);
}

// Chats older than `days` (by their last message), except pinned ones and
// those in `keep` (the open chat): what to delete. 0 days: none.
function prune(chats, days, now, keep) {
    if (!(days > 0))
        return [];
    const before = now - days * DAY;
    return (chats ?? []).filter(c => !c.pinned && c.updated < before && (keep ?? []).indexOf(c.id) < 0).map(c => c.id);
}

// ── Search ────────────────────────────────────────────────────────────
function fold(text) {
    return String(text ?? "").toLowerCase();
}
function words(query) {
    return oneLine(fold(query)).split(" ").filter(w => w !== "");
}
// Every word of the query, somewhere in the title or the chat's words
function matches(entry, query) {
    const ws = words(query);
    if (ws.length === 0)
        return true;
    const hay = fold(entry.title + " " + entry.preview);
    return ws.every(w => hay.indexOf(w) >= 0);
}
// `found`: ids a search through the whole files also matched
function filter(chats, query, found) {
    return (chats ?? []).filter(c => matches(c, query) || (found ?? []).indexOf(c.id) >= 0);
}

// ── Groups and times ─────────────────────────────────────────────────
function dayStart(t) {
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}
// Whole calendar days between two times (local time)
function daysAgo(t, now) {
    return Math.round((dayStart(now) - dayStart(t)) / DAY);
}
function groupName(t, now) {
    const ago = daysAgo(t, now);
    if (ago <= 0)
        return "Today";
    if (ago === 1)
        return "Yesterday";
    if (ago <= 7)
        return "Previous 7 days";
    const d = new Date(t);
    return MONTHS[d.getMonth()] + (d.getFullYear() === new Date(now).getFullYear() ? "" : " " + d.getFullYear());
}
// The list as shown: headers and chats, Pinned first, then newest first by
// day and month. Rows: { header } or { chat }.
function group(chats, now) {
    const out = [];
    const sorted = (chats ?? []).slice().sort(byNewest);
    const pinned = sorted.filter(c => c.pinned);
    if (pinned.length) {
        out.push({ header: "Pinned" });
        pinned.forEach(c => out.push({ chat: c }));
    }
    let last = "";
    sorted.filter(c => !c.pinned).forEach(c => {
        const name = groupName(c.updated, now);
        if (name !== last)
            out.push({ header: name });
        last = name;
        out.push({ chat: c });
    });
    return out;
}

// "Just now", "5 min ago", "3 h ago", "Yesterday", "Monday", "3 Oct", "3 Oct 2025"
function relativeTime(t, now) {
    const s = Math.max(0, now - t);
    const ago = daysAgo(t, now);
    if (s < 60000)
        return "Just now";
    if (ago <= 0 && s < 3600000)
        return Math.floor(s / 60000) + " min ago";
    if (ago <= 0)
        return Math.floor(s / 3600000) + " h ago";
    if (ago === 1)
        return "Yesterday";
    const d = new Date(t);
    if (ago < 7)
        return DAYS[d.getDay()];
    return d.getDate() + " " + MONTHS[d.getMonth()].slice(0, 3) + (d.getFullYear() === new Date(now).getFullYear() ? "" : " " + d.getFullYear());
}
