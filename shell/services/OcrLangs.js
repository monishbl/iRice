.pragma library

// Which tesseract languages Text Capture reads with: the system's languages
// (LC_ALL, LC_MESSAGES, LANG, LANGUAGE) whose tesseract-data-* package is
// installed, then English. ScreenTools passes them as `-l deu+eng`.

// ISO 639-1 (or -2/3 where locales use it) → tesseract's code
const codes = {
    af: "afr", am: "amh", ar: "ara", as: "asm", az: "aze", be: "bel", bg: "bul", bn: "ben", bo: "bod", bs: "bos",
    ca: "cat", cs: "ces", cy: "cym", da: "dan", de: "deu", el: "ell", en: "eng", eo: "epo", es: "spa", et: "est",
    eu: "eus", fa: "fas", fi: "fin", fil: "fil", fr: "fra", ga: "gle", gd: "gla", gl: "glg", gu: "guj", he: "heb",
    hi: "hin", hr: "hrv", ht: "hat", hu: "hun", hy: "hye", id: "ind", is: "isl", it: "ita", iu: "iku", ja: "jpn",
    jv: "jav", ka: "kat", kk: "kaz", km: "khm", kn: "kan", ko: "kor", ku: "kmr", ky: "kir", la: "lat", lb: "ltz",
    lo: "lao", lt: "lit", lv: "lav", mi: "mri", mk: "mkd", ml: "mal", mn: "mon", mr: "mar", ms: "msa", mt: "mlt",
    my: "mya", nb: "nor", ne: "nep", nl: "nld", nn: "nor", no: "nor", oc: "oci", or: "ori", pa: "pan", pl: "pol",
    ps: "pus", pt: "por", qu: "que", ro: "ron", ru: "rus", sa: "san", sd: "snd", si: "sin", sk: "slk", sl: "slv",
    sq: "sqi", sr: "srp", su: "sun", sv: "swe", sw: "swa", ta: "tam", te: "tel", tg: "tgk", th: "tha", ti: "tir",
    tl: "tgl", to: "ton", tr: "tur", tt: "tat", ug: "uig", uk: "ukr", ur: "urd", uz: "uzb", vi: "vie", yi: "yid",
    yo: "yor"
};

// "de_DE.UTF-8" → ["deu"]; zh by region and script; sr@latin → srp_latn
function codesFor(locale) {
    const l = String(locale || "").trim();
    if (l === "" || l === "C" || l === "POSIX" || /^C\./.test(l))
        return [];
    const m = l.match(/^([a-z]{2,3})(?:_([A-Za-z]{2,4}))?(?:\.[^@]*)?(?:@(\w+))?/);
    if (!m)
        return [];
    const lang = m[1], region = (m[2] || "").toUpperCase(), mod = m[3] || "";
    if (lang === "zh")
        return region === "TW" || region === "HK" || region === "MO" || region === "HANT" ? ["chi_tra"] : ["chi_sim"];
    if (lang === "sr")
        return mod === "latin" ? ["srp_latn"] : ["srp"];
    if (lang === "uz")
        return mod === "cyrillic" ? ["uzb_cyrl"] : ["uzb"];
    if (lang === "az" && mod === "cyrillic")
        return ["aze_cyrl"];
    return codes[lang] ? [codes[lang]] : [];
}

// env: { LC_ALL, LC_MESSAGES, LANG, LANGUAGE }; installed: `tesseract
// --list-langs` output (or a list). Returns { langs: ["deu", "eng"], missing:
// ["fra"] } (system languages without their data installed).
function pick(env, installed) {
    const have = {};
    const list = Array.isArray(installed) ? installed : String(installed || "").split("\n").filter(l => !/^List of/.test(l));
    for (const x of list)
        if (x.trim() !== "")
            have[x.trim()] = true;
    const wanted = [];
    const add = c => {
        if (wanted.indexOf(c) < 0)
            wanted.push(c);
    };
    for (const k of ["LC_ALL", "LC_MESSAGES", "LANG"])
        for (const c of codesFor(env[k]))
            add(c);
    for (const part of String(env.LANGUAGE || "").split(":"))
        for (const c of codesFor(part))
            add(c);
    const langs = [], missing = [];
    for (const c of wanted) {
        if (c === "eng")
            continue;
        if (have[c])
            langs.push(c);
        else
            missing.push(c);
    }
    // English last (it's the fallback, and the system language reads best
    // when it comes first). Without its data, only when nothing else is
    // there (tesseract fails on a language it doesn't have)
    if (have.eng || langs.length === 0)
        langs.push("eng");
    return { langs: langs, missing: missing };
}
