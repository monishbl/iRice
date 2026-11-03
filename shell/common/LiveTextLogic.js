.pragma library

// Live Text's pure parts (services/LiveText, LiveTextLayer, Search),
// tested in Node (tests/live_text.test.cjs): tesseract's TSV as word boxes,
// the text of a selection, the links, addresses and numbers in it, the
// screenshot index's search, and when the index may be built.

// `tesseract img stdout tsv` → { width, height, words: [{ x, y, w, h, text,
// line, block, ly, lh }] } in reading order, in the picture's own pixels
// (ly, lh: the line's top and height, so a selection reads as one band).
// Marks tesseract reads in icons and lines (no letter or digit, low
// confidence) are left out.
function parseTsv(tsv) {
    const out = { width: 0, height: 0, words: [] };
    const lines = {};
    for (const l of String(tsv || "").split("\n")) {
        const f = l.split("\t");
        if (f.length < 12)
            continue;
        if (f[0] === "1") {
            out.width = parseInt(f[8]) || 0;
            out.height = parseInt(f[9]) || 0;
            continue;
        }
        if (f[0] === "4") {
            lines[f[2] + "." + f[3] + "." + f[4]] = { y: parseInt(f[7]) || 0, h: parseInt(f[9]) || 0 };
            continue;
        }
        const text = f.slice(11).join(" ").trim();
        const conf = parseFloat(f[10]);
        if (f[0] !== "5" || text === "" || !(conf >= 10))
            continue;
        if (conf < 60 && !/[0-9A-Za-z\u00C0-\uFFFF]/.test(text))
            continue;
        const w = {
            x: parseInt(f[6]) || 0, y: parseInt(f[7]) || 0, w: parseInt(f[8]) || 0, h: parseInt(f[9]) || 0,
            text: text, block: f[2], line: f[2] + "." + f[3] + "." + f[4]
        };
        const box = lines[w.line];
        w.ly = box ? Math.min(box.y, w.y) : w.y;
        w.lh = box ? Math.max(box.y + box.h, w.y + w.h) - w.ly : w.h;
        out.words.push(w);
    }
    return out;
}

// Words a..b (indexes, either order) as text: a space between the words of a
// line, a line break between lines, a blank line between blocks
function textOf(words, a, b) {
    if (!words || words.length === 0 || a < 0 || b < 0)
        return "";
    const from = Math.max(0, Math.min(a, b)), to = Math.min(words.length - 1, Math.max(a, b));
    let s = "";
    for (let i = from; i <= to; i++) {
        const w = words[i];
        if (i > from)
            s += w.block !== words[i - 1].block ? "\n\n" : w.line !== words[i - 1].line ? "\n" : " ";
        s += w.text;
    }
    return s;
}
function allText(words) {
    return textOf(words, 0, (words || []).length - 1);
}

// The word at (x, y) (picture pixels; `slack` around each box), else -1
function wordAt(words, x, y, slack) {
    const s = slack || 0;
    for (let i = 0; i < (words || []).length; i++) {
        const w = words[i];
        if (x >= w.x - s && x <= w.x + w.w + s && y >= w.y - s && y <= w.y + w.h + s)
            return i;
    }
    return -1;
}

// The word a drag at (x, y) reaches: on the line nearest to y, the word
// nearest to x (past a line's end: its last word), as on a phone
function nearestWord(words, x, y) {
    let best = -1, bestDy = Infinity;
    for (let i = 0; i < (words || []).length; i++) {
        const w = words[i];
        const dy = y < w.y ? w.y - y : y > w.y + w.h ? y - w.y - w.h : 0;
        if (dy < bestDy) {
            bestDy = dy;
            best = i;
        }
    }
    if (best < 0)
        return -1;
    const line = words[best].line;
    let pick = best, bestDx = Infinity;
    for (let i = 0; i < words.length; i++) {
        const w = words[i];
        if (w.line !== line)
            continue;
        const dx = x < w.x ? w.x - x : x > w.x + w.w ? x - w.x - w.w : 0;
        if (dx < bestDx) {
            bestDx = dx;
            pick = i;
        }
    }
    return pick;
}

// The first and last word of the line word i is on
function lineOf(words, i) {
    if (i < 0 || i >= (words || []).length)
        return [-1, -1];
    let a = i, b = i;
    while (a > 0 && words[a - 1].line === words[i].line)
        a--;
    while (b < words.length - 1 && words[b + 1].line === words[i].line)
        b++;
    return [a, b];
}

// The words containing any word of `query` (Search's "find screenshots by
// their text": what you searched for, selected in Quick Preview)
function matching(words, query) {
    const terms = termsOf(query);
    const out = [];
    for (let i = 0; i < (words || []).length; i++) {
        const t = words[i].text.toLowerCase();
        if (terms.some(q => t.indexOf(q) >= 0))
            out.push(i);
    }
    return out;
}
function termsOf(query) {
    return String(query || "").toLowerCase().split(/\s+/).filter(t => t.length > 0);
}

// ── Quick actions ───────────────────────────────────────────────────────
// Links (http, https, or www.), email addresses and phone numbers in found
// text, as one-tap actions: [{ kind: link | email | phone, value, label }].
// Nothing found in a picture ever runs as a command: links open only on the
// web (Launch.openLink), addresses as mailto:, numbers are copied.
const LINK = /\b(?:https?:\/\/|www\.)[^\s<>"'`]+/gi;
const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}/g;
const PHONE = /(?:\+|\(|\b)\d[\d ().\-\u2010-\u2012]{5,18}\d\b/g;

function actions(text, max) {
    const s = String(text || "");
    const out = [], seen = {}, taken = [];
    const add = (kind, value, label, at, len) => {
        taken.push([at, at + len]);
        if (seen[kind + value])
            return;
        seen[kind + value] = true;
        out.push({ kind: kind, value: value, label: label });
    };
    const inTaken = (a, b) => taken.some(r => a < r[1] && b > r[0]);
    let m;
    LINK.lastIndex = 0;
    while ((m = LINK.exec(s)) !== null) {
        const raw = m[0].replace(/[.,;:!?)\]}>'"]+$/, "");
        const url = /^www\./i.test(raw) ? "https://" + raw : raw;
        // (a name before an @ in a link only disguises where it goes: no chip)
        if (/^https?:\/\/[^\s/?#@]+\.[^\s/?#@]+(?:[/?#]|$)/i.test(url))
            add("link", url, raw.replace(/^https?:\/\//i, "").replace(/\/$/, ""), m.index, m[0].length);
        else
            taken.push([m.index, m.index + m[0].length]);
    }
    EMAIL.lastIndex = 0;
    while ((m = EMAIL.exec(s)) !== null)
        if (!inTaken(m.index, m.index + m[0].length))
            add("email", m[0], m[0], m.index, m[0].length);
    PHONE.lastIndex = 0;
    while ((m = PHONE.exec(s)) !== null) {
        const raw = m[0].trim();
        if (inTaken(m.index, m.index + m[0].length) || !looksLikePhone(raw))
            continue;
        add("phone", (raw[0] === "+" ? "+" : "") + raw.replace(/\D/g, ""), raw, m.index, m[0].length);
    }
    return out.slice(0, max || 6);
}

// A run of digits that reads as a phone number: 7 to 15 digits, written
// with a + or separators the way numbers are (not a date, a version, an
// address on the network, or one bare run: an amount, an order number)
function looksLikePhone(raw) {
    const digits = raw.replace(/\D/g, "");
    if (digits.length < 7 || digits.length > 15)
        return false;
    if (/^\d{4}[-./]\d{1,2}[-./]\d{1,2}$/.test(raw) || /^\d{1,2}[-./]\d{1,2}[-./]\d{2,4}$/.test(raw))
        return false; // a date
    if (/\d\.\d+\.\d/.test(raw) && !/[ ()\-]/.test(raw))
        return false; // a version or an IP address
    return raw[0] === "+" || /[ ()\-\u2010-\u2012]/.test(raw);
}

// ── The screenshot index ────────────────────────────────────────────────
// shots: { path: { m: modified (s), t: text } } (assets/scripts/shot-index.py).
// Every word of `query` must be in a shot's text; newest first, with the
// words around the first match: [{ path, modified, snippet }]
function search(shots, query, limit) {
    const terms = termsOf(query);
    if (terms.length === 0 || !shots)
        return [];
    const hits = [];
    for (const path in shots) {
        const e = shots[path];
        const t = String(e && e.t || "");
        const low = t.toLowerCase();
        if (terms.every(q => low.indexOf(q) >= 0))
            hits.push({ path: path, modified: e.m || 0, snippet: snippet(t, low.indexOf(terms[0]), terms[0].length) });
    }
    hits.sort((a, b) => b.modified - a.modified);
    return hits.slice(0, limit || 5);
}

// About 60 characters of `text` around position `at`, on word boundaries,
// on one line, "…" where it was cut
function snippet(text, at, len) {
    const flat = String(text).replace(/\s+/g, " ");
    // (the same position in the flattened text: count what collapsed before it)
    const pos = String(text).slice(0, Math.max(0, at)).replace(/\s+/g, " ").length;
    let a = Math.max(0, pos - 24), b = Math.min(flat.length, pos + len + 36);
    if (a > 0) {
        const sp = flat.indexOf(" ", a);
        a = sp >= 0 && sp < pos ? sp + 1 : a;
    }
    if (b < flat.length) {
        const sp = flat.lastIndexOf(" ", b);
        b = sp > pos + len ? sp : b;
    }
    return (a > 0 ? "…" : "") + flat.slice(a, b).trim() + (b < flat.length ? "…" : "");
}

// The index is built in the background only when it costs nothing you'd
// notice: not on battery below 30%, never in power saver
function mayIndex(p) {
    if (!p || p.saver)
        return false;
    return !(p.onBattery && p.percent >= 0 && p.percent < 30);
}
