// Search's ranking as the QML shell computes it, for the Rust one to match
// (rank.rs's tests read golden.json): Search.qml's own functions (score,
// calculate, looksLikeConversion, wordToDefine, query, aiRow, the actions),
// taken from the file as they are and run under node with the services
// they call stubbed (no answers, App Store, calendar, contacts, notes or
// screenshots yet: those are the shell's, sent to Rust as they are).
//   node golden.js            print the golden JSON
// tests/test_rust_ui.py runs it and checks golden.json is what Search.qml gives now.
"use strict";
const fs = require("fs");
const path = require("path");
const root = path.resolve(__dirname, "../../../../..");
const src = fs.readFileSync(path.join(root, "shell/services/Search.qml"), "utf8");

// A function or property of the singleton, by its first line, to its closing brace
function grab(start) {
    const i = src.indexOf(start);
    if (i < 0)
        throw new Error("not in Search.qml: " + start);
    let depth = 0, j = src.indexOf(start.endsWith("[") ? "[" : "{", i + start.length - 1);
    for (; j < src.length; j++) {
        const c = src[j];
        if (c === "{" || c === "[" || c === "(")
            depth++;
        else if (c === "}" || c === "]" || c === ")") {
            depth--;
            if (depth === 0)
                break;
        }
    }
    return src.slice(i, j + 1);
}

const functions = ["function score(", "function calculate(", "function looksLikeConversion(", "function wordToDefine(", "function query(", "function aiRow(", "function iconForFile("].map(grab).join("\n");
const actions = grab("readonly property var actions: [").replace("readonly property var actions:", "const actions =");
const emojiList = (() => {
    const t = fs.readFileSync(path.join(root, "shell/common/emoji.js"), "utf8");
    return eval(t.replace(".pragma library", "").replace("var list =", "(") .replace(/;\s*$/, ")"));
})();

const APPS = [
    { id: "firefox", name: "Firefox", genericName: "Web Browser", comment: "Browse the World Wide Web", keywords: ["internet", "www", "browser"] },
    { id: "chromium", name: "Chromium", genericName: "Web Browser", comment: "", keywords: ["browser"] },
    { id: "org.kde.dolphin", name: "Dolphin", genericName: "File Manager", comment: "", keywords: ["files", "folder"] },
    { id: "thunar", name: "Thunar File Manager", genericName: "File Manager", comment: "Browse the filesystem", keywords: [] },
    { id: "org.kde.krita", name: "Krita", genericName: "Digital Painting", comment: "", keywords: ["paint", "draw"] },
    { id: "gimp", name: "GNU Image Manipulation Program", genericName: "Image Editor", comment: "", keywords: ["gimp", "photo"] },
    { id: "org.gnome.Calculator", name: "Calculator", genericName: "", comment: "Perform calculations", keywords: ["math", "arithmetic"] },
    { id: "foot", name: "Foot", genericName: "Terminal", comment: "A wayland native terminal", keywords: ["shell", "prompt", "command"] },
    { id: "code-oss", name: "Code - OSS", genericName: "Text Editor", comment: "Code Editing. Redefined.", keywords: ["vscode"] },
    { id: "mousepad", name: "Mousepad", genericName: "Text Editor", comment: "Simple text editor", keywords: ["text", "editor"] },
    { id: "org.gnome.Settings", name: "Settings", genericName: "", comment: "", keywords: ["preferences"] },
    { id: "steam", name: "Steam", genericName: "", comment: "Game store", keywords: ["games"] },
    { id: "spotify", name: "Spotify", genericName: "Music Player", comment: "", keywords: ["music"] },
    { id: "hidden", name: "Hidden Helper", genericName: "", comment: "", keywords: [], noDisplay: true },
];

const QUERIES = [
    "fire", "firefox", "f", "fi", "browser", "web", "file", "files", "manager", "kr", "krita", "paint", "gimp", "image",
    "calc", "calculator", "2+2", "2*3", "12/4+1", "sqrt(16)", "(1+2)*3", "18% of 2450", "lock", "sleep", "restart", "shut",
    "dark", "settings", "screenshot", "record", "term", "terminal", "code", "text", "edit", "music", "spot", "steam", "game",
    "ask ai", "ai", "translate", "wallpaper", "update", "bluetooth", "wifi", "zzzz", "the quick brown fox", "define cat",
    "10 km to mi", ">ls -la", "> htop", "?why is the sky blue", "?", ":cat", ":heart", ":thumbs up", ":skin", "/a", "/notes",
    "  fire  ", "Fire", "FIREFOX", "ff", "dol", "dolph", "tnb", "mouse", "pad", "quick share", "share", "send", "kb", "keyboard",
];

function run() {
    const Apps = { entries: APPS };
    const IconLookup = { path: (n, f) => n || f };
    const stubs = {
        Apps, IconLookup,
        Intents: { rows: () => [] },
        AppStore: { available: false, quick: [], quickLoading: false, lookUp() {}, forget() {}, sourceName: () => "", iconFor: () => "" },
        Calendar: { search: () => [], parseQuick: () => null, timeText: () => "" },
        CloudAccounts: { searchContacts: () => [] },
        Notes: { search: () => [] },
        LiveText: { search: () => [] },
        Clipboard: { filtered: () => [] },
        Tools: { has: () => false },
        Ai: { configured: false, model: { name: "Add a model" } },
        Quickshell: { env: () => "/home/u", execDetached() {} },
        Launch: {}, Panels: {}, Files: {}, Config: { options: {} }, Theme: {}, Island: {}, Qt: { formatDate: () => "", formatDateTime: () => "" },
        Emoji: { list: emojiList },
    };
    const body = `
        let files = [], contents = [], fileQuery = "", revision = 0;
        let convQuery = "", convResult = "", ansQuery = "", ansResult = null, ansId = 0, defWord = "", definition = null;
        const ansGlyphs = ({ math: "calculator", date: "calendar-blank", time: "globe-hemisphere-west", unit: "ruler", currency: "currency-circle-dollar" });
        const fileDebounce = { restart() {} }, convDebounce = { restart() {} }, ansDebounce = { restart() {} }, defDebounce = { restart() {} };
        const castActions = [];
        ${actions};
        ${functions}
        return { score, calculate, looksLikeConversion, wordToDefine, query, actions };`;
    const S = new Function(...Object.keys(stubs), body)(...Object.values(stubs));
    const out = {
        apps: APPS.map(a => ({ id: a.id, name: a.name, generic: a.genericName, comment: a.comment, keywords: a.keywords, noDisplay: !!a.noDisplay })),
        actions: S.actions.map(a => [a.title, a.glyph, a.keywords]),
        cases: QUERIES.map(q => ({ q, rows: S.query(q).map(r => [r.section, r.title, r.subtitle ?? ""]) })),
        score: [],
        calc: [],
        conversion: [],
        define: [],
    };
    for (const t of ["Firefox", "Visual Studio Code", "org.kde.dolphin", "Thunderbird", "A very long application name here", "Krita", "x-y_z.w", "Ünïcode Äpp"])
        for (const q of ["f", "fire", "stu", "dol", "der", "tnb", "a", "krt", "zz", "y", "w", "äpp", "ünï"])
            out.score.push([t, q, S.score(t, q)]);
    for (const e of ["2+2", "0.1 + 0.2", "2^10", "2^3^2", "(-2)^2", "-2^2", "2^-2", "7 % 3", "-7 % 3", "3 × 4 ÷ 2", "1,000 * 3", "sqrt(16)", "sqrt16",
            "pi*2", "2pi", "e^1", "log(1000)", "ln(1)", "1/3", "1/0", "2(3)", "1++2", "1+-2", "08+1", "0.5+1", ".5*2", "1e5+1", "2^80", "1/1000000000",
            "2/*3*/", "1//2", "12", "hello", "(1+2", "pie^0", "sqrt()", "sin(0)+cos(0)", "tan(0)", "2.e3+1", "100-99.99", "3*(4+5)/2", "10%3*2", "((2))", "-(3)", "+3-1", "2**3", "1e"])
        out.calc.push([e, S.calculate(e) === null ? null : String(S.calculate(e))]);
    for (const q of ["10 km to mi", "$20 in eur", "€5 as usd", "72 f to c", "100 usd in inr", "3 cups IN ml", "5 to 6", "km to mi", "10 km to", "hello in there", "10 km mi", "-5 c to f", "1.5 kg into lb"])
        out.conversion.push([q, S.looksLikeConversion(q)]);
    for (const q of ["define serendipity", "Define Cat?", "what does ephemeral mean", "what does ephemeral mean?", "meaning of life", "definition of rock-n-roll", "hubris meaning", "define two words", "definex", "firefox", "cat define", "define   spaced", "WHAT DOES X MEAN"])
        out.define.push([q, S.wordToDefine(q)]);
    return out;
}

process.stdout.write(JSON.stringify(run(), null, 1) + "\n");
