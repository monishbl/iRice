.pragma library

// What Settings search reads from the page files: each row's and group's
// caption (label, title, detail, pageTitle), as "file.qml:line:key: "text""
// in file order. The pages are shared by both looks, so a part shown in one
// look only is left out of the other's: a block whose `visible:` or
// `active:` is `Theme.material` (or `!Theme.material`, maybe `&& more`),
// with everything inside it. A caption written `Theme.material ? "a" : "b"`
// gives the look's text.
//   captions(grepOutput, material) -> ["dock.qml:12:label: "Size"", …]
// grepOutput: `grep -Hn` lines ("file.qml:line:text") of the page files,
// each line with a brace, a caption or a look's condition at least.

var LINE = /^([^:]+):(\d+):(.*)$/;
var LOOK = /\b(?:visible|active): *(!?)Theme\.material *(?:$|;|&&)/;
var CAPTION = /(label|title|detail|pageTitle): (?:Theme\.material \? "([^"]*)" : )?"([^"]*)"/g;

// A line's braces, captions and look conditions, in the order they come
function events(text) {
    // (strings and comments blanked to the same length: their braces don't count)
    const code = text.replace(/"(?:[^"\\]|\\.)*"/g, s => '"' + " ".repeat(s.length - 2) + '"').replace(/\/\/.*$/, s => " ".repeat(s.length));
    const out = [];
    for (let k = 0; k < code.length; k++)
        if (code[k] === "{" || code[k] === "}")
            out.push({ at: k, brace: code[k] });
    const look = code.match(LOOK);
    if (look)
        out.push({ at: look.index, look: look[1] === "!" ? "glass" : "material" });
    CAPTION.lastIndex = 0;
    let c;
    while ((c = CAPTION.exec(text)) !== null)
        if (code[c.index] !== " ") // (not inside a string or comment)
            out.push({ at: c.index, key: c[1], material: c[2], glass: c[3] });
    return out.sort((a, b) => a.at - b.at);
}

function captions(text, material) {
    const lines = [];
    for (const l of text.split("\n")) {
        const m = l.match(LINE);
        if (m)
            lines.push({ file: m[1], line: m[2], events: events(m[3]) });
    }
    // Two passes over the blocks (numbered by their opening brace): which
    // look each is for (its condition can come after captions in it), then
    // the captions in no block of the other look
    const other = material ? "glass" : "material";
    const lookOf = {};
    const walk = onCaption => {
        let file = "";
        let next = 0;
        let open = [];
        for (const l of lines) {
            if (l.file !== file) {
                file = l.file;
                open = [];
            }
            for (const e of l.events) {
                if (e.brace === "{")
                    open.push(next++);
                else if (e.brace === "}")
                    open.pop();
                else if (e.look && open.length > 0)
                    lookOf[open[open.length - 1]] = e.look;
                else if (e.key && onCaption && !open.some(b => lookOf[b] === other))
                    onCaption(l, e);
            }
        }
    };
    walk(null);
    const out = [];
    walk((l, e) => {
        const said = material && e.material !== undefined ? e.material : e.glass;
        if (said !== "")
            out.push(l.file + ":" + l.line + ":" + e.key + ": \"" + said + "\"");
    });
    return out;
}
