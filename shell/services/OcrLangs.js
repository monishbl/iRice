.pragma library

// Which tesseract languages Text Capture and Live Text read with: the
// system's languages (LC_ALL, LC_MESSAGES, LANG, LANGUAGE) and your keyboard
// layouts' whose tesseract-data-* package is installed, then English.
// ScreenTools passes them as `-l deu+eng`.

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

// Keyboard layouts (hyprctl's, xkb's codes) → tesseract's. Most name a
// country; a few take the language from their variant (below).
const layoutCodes = {
    al: "sqi", et: "amh", am: "hye", ara: "ara", eg: "ara", iq: "ara", ma: "ara", sy: "ara", az: "aze", bd: "ben",
    by: "bel", be: ["nld", "fra"], ba: "bos", bg: "bul", mm: "mya", cn: "chi_sim", hr: "hrv", cz: "ces", dk: "dan",
    af: "fas", mv: "div", nl: "nld", bt: "dzo", au: "eng", cm: "eng", gh: "eng", nz: "eng", ng: "eng", za: "eng",
    gb: "eng", us: "eng", epo: "epo", ee: "est", fo: "fao", ph: "fil", fi: "fin", fr: "fra", ca: "fra", cd: "fra",
    tg: "fra", ge: "kat", de: "deu", at: "deu", ch: "deu", gr: "ell", il: "heb", hu: "hun", is: "isl", in: "hin",
    id: "ind", ie: "gle", it: "ita", jp: "jpn", kz: "kaz", kh: "khm", kr: "kor", kg: "kir", la: "lao", lv: "lav",
    lt: "lit", mk: "mkd", my: "msa", mt: "mlt", md: "ron", mn: "mon", me: "srp_latn", np: "nep", no: "nor",
    ir: "fas", pl: "pol", pt: "por", br: "por", ro: "ron", ru: "rus", rs: "srp", lk: "sin", sk: "slk", si: "slv",
    es: "spa", latam: "spa", ke: "swa", tz: "swa", se: "swe", tw: "chi_tra", tj: "tgk", th: "tha", tr: "tur",
    ua: "ukr", pk: "urd", uz: "uzb", vn: "vie"
};
// Variants that change the language: India's scripts, French Switzerland,
// English Canada, Serbian in Latin, Tibetan and Uyghur in China… ("": a
// language tesseract has no data for)
const variantCodes = {
    in: { ben: "ben", guj: "guj", kan: "kan", mal: "mal", tam: "tam", tel: "tel", ori: "ori", guru: "pan", jhelum: "pan",
        urd: "urd", mar: "mar", marathi: "mar", san: "san", asm: "asm", eng: "eng", hin: "hin", bolnagri: "hin", mara: "", mni: "", sat: "" },
    ch: { fr: "fra", it: "ita" },
    ca: { eng: "eng", ike: "iku" },
    rs: { latin: "srp_latn" },
    cn: { tib: "bod", ug: "uig", mon: "mon" },
    es: { cat: "cat" },
    fr: { bre: "bre", oci: "oci" },
    gb: { gla: "gla" },
    ru: { tt: "tat" },
    pk: { snd: "snd", ara: "ara" }
};

// "de" → ["deu"]; "in(hin)", or "in" with variant "tam" → ["tam"]
function layoutLangs(layout, variant) {
    let l = String(layout || "").trim().toLowerCase(), v = String(variant || "").trim().toLowerCase();
    const m = l.match(/^([a-z]+)\(([^)]*)\)$/);
    if (m) {
        l = m[1];
        v = v || m[2];
    }
    const vs = variantCodes[l];
    if (vs && v !== "") {
        // (its name, or its first part: "ben_probhat", "hin-wx", "fr_mac"; or
        // what it starts with: "tamilnet")
        const head = v.split(/[-_]/)[0];
        let c = vs[v] ?? vs[head];
        if (c === undefined)
            for (const k in vs)
                if (k.length >= 3 && head.indexOf(k) === 0)
                    c = vs[k];
        if (c !== undefined)
            return c === "" ? [] : [c];
    }
    const c = layoutCodes[l];
    return c === undefined ? [] : Array.isArray(c) ? c.slice() : [c];
}

// The languages wanted, in order: the system's, then the keyboard layouts'
// (layouts: ["de", "in(hin)"] or [{ layout, variant }]), English last
function wanted(env, layouts) {
    const out = [];
    const add = c => {
        if (out.indexOf(c) < 0)
            out.push(c);
    };
    for (const k of ["LC_ALL", "LC_MESSAGES", "LANG"])
        for (const c of codesFor(env[k]))
            add(c);
    for (const part of String(env.LANGUAGE || "").split(":"))
        for (const c of codesFor(part))
            add(c);
    for (const l of layouts || [])
        for (const c of typeof l === "string" ? layoutLangs(l, "") : layoutLangs(l.layout, l.variant))
            add(c);
    const i = out.indexOf("eng");
    if (i >= 0)
        out.splice(i, 1);
    out.push("eng");
    return out;
}

// `tesseract --list-langs` output (or a list) → { code: true }
function installedSet(installed) {
    const have = {};
    const list = Array.isArray(installed) ? installed : String(installed || "").split("\n").filter(l => !/^List of/.test(l));
    for (const x of list)
        if (x.trim() !== "")
            have[x.trim()] = true;
    return have;
}

// env: { LC_ALL, LC_MESSAGES, LANG, LANGUAGE }; installed: `tesseract
// --list-langs` output (or a list); layouts: as for wanted(). Returns
// { langs: ["deu", "eng"], missing: ["fra"] } (wanted languages without
// their data installed).
function pick(env, installed, layouts) {
    const have = installedSet(installed);
    const langs = [], missing = [];
    for (const c of wanted(env, layouts)) {
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

// Settings' names for them (Settings → Keyboard → Screenshots)
const names = {
    afr: "Afrikaans", amh: "Amharic", ara: "Arabic", asm: "Assamese", aze: "Azerbaijani", aze_cyrl: "Azerbaijani (Cyrillic)",
    bel: "Belarusian", ben: "Bengali", bod: "Tibetan", bos: "Bosnian", bre: "Breton", bul: "Bulgarian", cat: "Catalan",
    ces: "Czech", chi_sim: "Chinese (Simplified)", chi_tra: "Chinese (Traditional)", cym: "Welsh", dan: "Danish",
    deu: "German", div: "Dhivehi", dzo: "Dzongkha", ell: "Greek", eng: "English", epo: "Esperanto", est: "Estonian",
    eus: "Basque", fao: "Faroese", fas: "Persian", fil: "Filipino", fin: "Finnish", fra: "French", gla: "Scottish Gaelic",
    gle: "Irish", glg: "Galician", guj: "Gujarati", hat: "Haitian Creole", heb: "Hebrew", hin: "Hindi", hrv: "Croatian",
    hun: "Hungarian", hye: "Armenian", iku: "Inuktitut", ind: "Indonesian", isl: "Icelandic", ita: "Italian",
    jav: "Javanese", jpn: "Japanese", kan: "Kannada", kat: "Georgian", kaz: "Kazakh", khm: "Khmer", kir: "Kyrgyz",
    kmr: "Kurdish", kor: "Korean", lao: "Lao", lat: "Latin", lav: "Latvian", lit: "Lithuanian", ltz: "Luxembourgish",
    mal: "Malayalam", mar: "Marathi", mkd: "Macedonian", mlt: "Maltese", mon: "Mongolian", mri: "Maori", msa: "Malay",
    mya: "Burmese", nep: "Nepali", nld: "Dutch", nor: "Norwegian", oci: "Occitan", ori: "Odia", pan: "Punjabi",
    pol: "Polish", por: "Portuguese", pus: "Pashto", que: "Quechua", ron: "Romanian", rus: "Russian", san: "Sanskrit",
    sin: "Sinhala", slk: "Slovak", slv: "Slovenian", snd: "Sindhi", spa: "Spanish", sqi: "Albanian", srp: "Serbian",
    srp_latn: "Serbian (Latin)", sun: "Sundanese", swa: "Swahili", swe: "Swedish", tam: "Tamil", tat: "Tatar",
    tel: "Telugu", tgk: "Tajik", tgl: "Tagalog", tha: "Thai", tir: "Tigrinya", ton: "Tongan", tur: "Turkish",
    uig: "Uyghur", ukr: "Ukrainian", urd: "Urdu", uzb: "Uzbek", uzb_cyrl: "Uzbek (Cyrillic)", vie: "Vietnamese",
    yid: "Yiddish", yor: "Yoruba"
};
function nameOf(code) {
    return names[code] ?? code;
}
