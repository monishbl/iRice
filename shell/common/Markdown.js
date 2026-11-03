.pragma library
// Markdown to Qt rich text, for what models write: headings, paragraphs,
// emphasis, inline code, fenced code, lists (nested, numbered, tasks),
// quotes, tables, rules and links. Everything is escaped first, so a reply
// can't bring its own HTML, styles or images; links open only for http(s)
// and mailto. Colours come from the caller (each look's), as opaque #rrggbb.
//   split(md)            -> [{ kind: "text", md } | { kind: "code", lang, code, open }]
//   html(md, style)      -> rich text for one text part (code fences as <pre>)
// style: { text, muted, link, codeBg, rule, mono, body (px) }

function escape(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

const FENCE = /^ {0,3}(`{3,}|~{3,})\s*([\w+#.-]*)[^`]*$/;

// The reply in parts: prose, and each fenced code block on its own (open:
// still streaming, its closing fence not here yet)
function split(md) {
    const parts = [];
    const lines = (md ?? "").replace(/\r\n?/g, "\n").split("\n");
    let prose = [];
    let i = 0;
    const flush = () => {
        const text = prose.join("\n");
        if (text.trim() !== "")
            parts.push({ kind: "text", md: text, lang: "", code: "", open: false });
        prose = [];
    };
    while (i < lines.length) {
        const m = lines[i].match(FENCE);
        if (!m) {
            prose.push(lines[i++]);
            continue;
        }
        flush();
        const fence = m[1];
        const code = [];
        i++;
        let closed = false;
        while (i < lines.length) {
            const l = lines[i++];
            if (l.trim().startsWith(fence[0].repeat(fence.length)) && l.trim().replace(new RegExp("^\\" + fence[0] + "+"), "") === "") {
                closed = true;
                break;
            }
            code.push(l);
        }
        parts.push({ kind: "code", md: "", lang: m[2] || "", code: code.join("\n"), open: !closed });
    }
    flush();
    return parts;
}

function safeUrl(url) {
    const u = url.trim();
    return /^(https?:\/\/|mailto:)[^\s"'<>]+$/i.test(u) ? u : "";
}

// Inline formatting of one escaped line or paragraph
function inline(text, st) {
    const slots = [];
    const keep = html => "\u0000" + (slots.push(html) - 1) + "\u0000";
    let s = escape(text);
    // Code spans first: nothing inside them is formatted
    s = s.replace(/(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/g, (_, __, code) =>
        keep('<span style="font-family:\'' + st.mono + '\'; background-color:' + st.codeBg + ';">&#8202;' + code.trim() + '&#8202;</span>'));
    // Images: their text as a link (no image is fetched)
    s = s.replace(/!\[([^\]]*)\]\(([^)\s]+)(?:\s+&quot;[^&]*&quot;)?\)/g, (all, alt, url) => {
        const u = safeUrl(url.replace(/&amp;/g, "&"));
        return u ? keep('<a href="' + escape(u) + '" style="color:' + st.link + ';">' + (alt || "image") + "</a>") : alt;
    });
    s = s.replace(/\[([^\]]+)\]\(([^)\s]+)(?:\s+&quot;[^&]*&quot;)?\)/g, (all, label, url) => {
        const u = safeUrl(url.replace(/&amp;/g, "&"));
        return u ? keep('<a href="' + escape(u) + '" style="color:' + st.link + ';">' + label + "</a>") : label;
    });
    // Bare and <angle> web addresses
    s = s.replace(/&lt;(https?:\/\/[^\s&]+)&gt;|(^|[\s(])(https?:\/\/[^\s<]+[^\s<.,;:!?)'&])/g, (all, angled, lead, bare) => {
        const u = angled || bare;
        return (lead ?? "") + keep('<a href="' + u + '" style="color:' + st.link + ';">' + u + "</a>");
    });
    s = s.replace(/(\*\*|__)(?=\S)([\s\S]*?\S)\1/g, "<b>$2</b>");
    s = s.replace(/(^|[^*\w])\*(?=\S)([^*]*?\S)\*(?!\*)/g, "$1<i>$2</i>");
    s = s.replace(/(^|[^_\w])_(?=\S)([^_]*?\S)_(?![_\w])/g, "$1<i>$2</i>");
    s = s.replace(/~~(?=\S)([\s\S]*?\S)~~/g, "<s>$1</s>");
    return s.replace(/\u0000(\d+)\u0000/g, (_, n) => slots[+n]);
}

const LIST = /^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$/;

function list(lines, st) {
    // Items with their indent; deeper ones nest under the item before
    const items = lines.map(l => {
        const m = l.match(LIST);
        return m ? { indent: m[1].replace(/\t/g, "    ").length, ordered: /\d/.test(m[2]), start: parseInt(m[2]) || 1, text: m[3] } : null;
    });
    let out = "";
    const stack = [];
    for (let k = 0; k < items.length; k++) {
        const it = items[k];
        if (!it) { // a continuation line: part of the item before
            out += " " + inline(lines[k].trim(), st);
            continue;
        }
        while (stack.length && it.indent < stack[stack.length - 1].indent)
            out += "</li></" + stack.pop().tag + ">";
        const top = stack[stack.length - 1];
        if (!top || it.indent > top.indent) {
            const tag = it.ordered ? "ol" : "ul";
            stack.push({ indent: it.indent, tag: tag });
            out += "<" + tag + (it.ordered && it.start !== 1 ? ' start="' + it.start + '"' : "") + ' style="margin-top:2px; margin-bottom:2px;">';
        } else {
            out += "</li>";
        }
        let text = it.text;
        const task = text.match(/^\[([ xX])\]\s+(.*)$/);
        if (task)
            text = (task[1] === " " ? "☐ " : "☑ ") + task[2];
        out += '<li style="margin-bottom:2px;">' + inline(text, st);
    }
    while (stack.length)
        out += "</li></" + stack.pop().tag + ">";
    return out;
}

function cells(line) {
    // (an escaped \| stays in its cell)
    return line.trim().replace(/^\|/, "").replace(/\|$/, "").replace(/\\\|/g, "\u0001").split("|").map(c => c.trim().replace(/\u0001/g, "|"));
}

function table(lines, st) {
    const head = cells(lines[0]);
    const align = cells(lines[1]).map(c => /^:-+:$/.test(c) ? "center" : /-+:$/.test(c) ? "right" : "left");
    let out = '<table cellspacing="0" cellpadding="6" style="border-collapse:collapse; border-color:' + st.rule + '; border-style:solid; border-width:1px; margin-top:4px; margin-bottom:4px;"><tr>';
    head.forEach((c, k) => out += '<th align="' + (align[k] ?? "left") + '" style="border:1px solid ' + st.rule + ';">' + inline(c, st) + "</th>");
    out += "</tr>";
    for (const l of lines.slice(2)) {
        out += "<tr>";
        cells(l).forEach((c, k) => out += '<td align="' + (align[k] ?? "left") + '" style="border:1px solid ' + st.rule + ';">' + inline(c, st) + "</td>");
        out += "</tr>";
    }
    return out + "</table>";
}

const HEAD = [0, 20, 18, 16, 15, 14, 14]; // (px; product UI stays at or under 24)

// Rich text for prose (fenced code, if any, as plain <pre>)
function html(md, st) {
    const lines = (md ?? "").replace(/\r\n?/g, "\n").split("\n");
    let out = "";
    let i = 0;
    const isTableSep = l => /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/.test(l) && l.includes("-");
    while (i < lines.length) {
        const l = lines[i];
        if (l.trim() === "") { i++; continue; }
        const fence = l.match(FENCE);
        if (fence) {
            const code = [];
            i++;
            while (i < lines.length && !lines[i].trim().startsWith(fence[1])) code.push(lines[i++]);
            i++;
            out += '<pre style="font-family:\'' + st.mono + '\'; background-color:' + st.codeBg + ';">' + escape(code.join("\n")) + "</pre>";
            continue;
        }
        const h = l.match(/^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$/);
        if (h) {
            const n = h[1].length;
            out += '<p style="margin-top:' + (n <= 2 ? 10 : 8) + 'px; margin-bottom:4px;"><span style="font-size:' + HEAD[n] + 'px; font-weight:600;">' + inline(h[2], st) + "</span></p>";
            i++;
            continue;
        }
        if (/^ {0,3}([-*_])(\s*\1){2,}\s*$/.test(l)) {
            out += '<hr style="background-color:' + st.rule + ';"/>';
            i++;
            continue;
        }
        if (/^ {0,3}&gt;|^ {0,3}>/.test(l)) {
            const q = [];
            while (i < lines.length && /^ {0,3}>/.test(lines[i])) q.push(lines[i++].replace(/^ {0,3}> ?/, ""));
            out += '<blockquote style="color:' + st.muted + '; margin-left:12px;">' + html(q.join("\n"), st) + "</blockquote>";
            continue;
        }
        if (l.includes("|") && i + 1 < lines.length && isTableSep(lines[i + 1])) {
            const t = [l, lines[i + 1]];
            i += 2;
            while (i < lines.length && lines[i].includes("|") && lines[i].trim() !== "") t.push(lines[i++]);
            out += table(t, st);
            continue;
        }
        if (LIST.test(l)) {
            const items = [];
            while (i < lines.length && lines[i].trim() !== "" && (LIST.test(lines[i]) || /^\s{2,}\S/.test(lines[i]))) items.push(lines[i++]);
            out += list(items, st);
            continue;
        }
        // A paragraph: lines joined (a line ending in two spaces or "\" breaks)
        const para = [];
        while (i < lines.length && lines[i].trim() !== "" && !lines[i].match(FENCE) && !/^ {0,3}(#{1,6}\s|>)/.test(lines[i]) && !(para.length && LIST.test(lines[i])))
            para.push(lines[i++]);
        out += '<p style="margin-top:0px; margin-bottom:8px;">' + para.map((p, k) => {
            const br = k < para.length - 1 && (/ {2,}$/.test(p) || /\\$/.test(p));
            return inline(p.replace(/\\$/, "").trim(), st) + (br ? "<br/>" : "");
        }).join(" ") + "</p>";
    }
    return out;
}
