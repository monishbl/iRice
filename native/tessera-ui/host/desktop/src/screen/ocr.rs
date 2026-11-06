//! Which tesseract languages Text Capture and Live Text read with
//! (shell/services/OcrLangs.js): the system's languages and the keyboard
//! layouts' whose tesseract-data-* is installed, then English.

use std::collections::HashSet;

const CODES: &[(&str, &str)] = &[
    ("af", "afr"), ("am", "amh"), ("ar", "ara"), ("as", "asm"), ("az", "aze"), ("be", "bel"), ("bg", "bul"), ("bn", "ben"), ("bo", "bod"), ("bs", "bos"),
    ("ca", "cat"), ("cs", "ces"), ("cy", "cym"), ("da", "dan"), ("de", "deu"), ("el", "ell"), ("en", "eng"), ("eo", "epo"), ("es", "spa"), ("et", "est"),
    ("eu", "eus"), ("fa", "fas"), ("fi", "fin"), ("fil", "fil"), ("fr", "fra"), ("ga", "gle"), ("gd", "gla"), ("gl", "glg"), ("gu", "guj"), ("he", "heb"),
    ("hi", "hin"), ("hr", "hrv"), ("ht", "hat"), ("hu", "hun"), ("hy", "hye"), ("id", "ind"), ("is", "isl"), ("it", "ita"), ("iu", "iku"), ("ja", "jpn"),
    ("jv", "jav"), ("ka", "kat"), ("kk", "kaz"), ("km", "khm"), ("kn", "kan"), ("ko", "kor"), ("ku", "kmr"), ("ky", "kir"), ("la", "lat"), ("lb", "ltz"),
    ("lo", "lao"), ("lt", "lit"), ("lv", "lav"), ("mi", "mri"), ("mk", "mkd"), ("ml", "mal"), ("mn", "mon"), ("mr", "mar"), ("ms", "msa"), ("mt", "mlt"),
    ("my", "mya"), ("nb", "nor"), ("ne", "nep"), ("nl", "nld"), ("nn", "nor"), ("no", "nor"), ("oc", "oci"), ("or", "ori"), ("pa", "pan"), ("pl", "pol"),
    ("ps", "pus"), ("pt", "por"), ("qu", "que"), ("ro", "ron"), ("ru", "rus"), ("sa", "san"), ("sd", "snd"), ("si", "sin"), ("sk", "slk"), ("sl", "slv"),
    ("sq", "sqi"), ("sr", "srp"), ("su", "sun"), ("sv", "swe"), ("sw", "swa"), ("ta", "tam"), ("te", "tel"), ("tg", "tgk"), ("th", "tha"), ("ti", "tir"),
    ("tl", "tgl"), ("to", "ton"), ("tr", "tur"), ("tt", "tat"), ("ug", "uig"), ("uk", "ukr"), ("ur", "urd"), ("uz", "uzb"), ("vi", "vie"), ("yi", "yid"),
    ("yo", "yor"),
];

/// Keyboard layouts (xkb's codes) → tesseract's; "be" takes two
const LAYOUTS: &[(&str, &[&str])] = &[
    ("al", &["sqi"]), ("et", &["amh"]), ("am", &["hye"]), ("ara", &["ara"]), ("eg", &["ara"]), ("iq", &["ara"]), ("ma", &["ara"]), ("sy", &["ara"]),
    ("az", &["aze"]), ("bd", &["ben"]), ("by", &["bel"]), ("be", &["nld", "fra"]), ("ba", &["bos"]), ("bg", &["bul"]), ("mm", &["mya"]),
    ("cn", &["chi_sim"]), ("hr", &["hrv"]), ("cz", &["ces"]), ("dk", &["dan"]), ("af", &["fas"]), ("mv", &["div"]), ("nl", &["nld"]), ("bt", &["dzo"]),
    ("au", &["eng"]), ("cm", &["eng"]), ("gh", &["eng"]), ("nz", &["eng"]), ("ng", &["eng"]), ("za", &["eng"]), ("gb", &["eng"]), ("us", &["eng"]),
    ("epo", &["epo"]), ("ee", &["est"]), ("fo", &["fao"]), ("ph", &["fil"]), ("fi", &["fin"]), ("fr", &["fra"]), ("ca", &["fra"]), ("cd", &["fra"]),
    ("tg", &["fra"]), ("ge", &["kat"]), ("de", &["deu"]), ("at", &["deu"]), ("ch", &["deu"]), ("gr", &["ell"]), ("il", &["heb"]), ("hu", &["hun"]),
    ("is", &["isl"]), ("in", &["hin"]), ("id", &["ind"]), ("ie", &["gle"]), ("it", &["ita"]), ("jp", &["jpn"]), ("kz", &["kaz"]), ("kh", &["khm"]),
    ("kr", &["kor"]), ("kg", &["kir"]), ("la", &["lao"]), ("lv", &["lav"]), ("lt", &["lit"]), ("mk", &["mkd"]), ("my", &["msa"]), ("mt", &["mlt"]),
    ("md", &["ron"]), ("mn", &["mon"]), ("me", &["srp_latn"]), ("np", &["nep"]), ("no", &["nor"]), ("ir", &["fas"]), ("pl", &["pol"]), ("pt", &["por"]),
    ("br", &["por"]), ("ro", &["ron"]), ("ru", &["rus"]), ("rs", &["srp"]), ("lk", &["sin"]), ("sk", &["slk"]), ("si", &["slv"]), ("es", &["spa"]),
    ("latam", &["spa"]), ("ke", &["swa"]), ("tz", &["swa"]), ("se", &["swe"]), ("tw", &["chi_tra"]), ("tj", &["tgk"]), ("th", &["tha"]), ("tr", &["tur"]),
    ("ua", &["ukr"]), ("pk", &["urd"]), ("uz", &["uzb"]), ("vn", &["vie"]),
];

/// Variants that change the language ("": a language tesseract has no data for)
const VARIANTS: &[(&str, &[(&str, &str)])] = &[
    ("in", &[("ben", "ben"), ("guj", "guj"), ("kan", "kan"), ("mal", "mal"), ("tam", "tam"), ("tel", "tel"), ("ori", "ori"), ("guru", "pan"), ("jhelum", "pan"),
        ("urd", "urd"), ("mar", "mar"), ("marathi", "mar"), ("san", "san"), ("asm", "asm"), ("eng", "eng"), ("hin", "hin"), ("bolnagri", "hin"), ("mara", ""), ("mni", ""), ("sat", "")]),
    ("ch", &[("fr", "fra"), ("it", "ita")]),
    ("ca", &[("eng", "eng"), ("ike", "iku")]),
    ("rs", &[("latin", "srp_latn")]),
    ("cn", &[("tib", "bod"), ("ug", "uig"), ("mon", "mon")]),
    ("es", &[("cat", "cat")]),
    ("fr", &[("bre", "bre"), ("oci", "oci")]),
    ("gb", &[("gla", "gla")]),
    ("ru", &[("tt", "tat")]),
    ("pk", &[("snd", "snd"), ("ara", "ara")]),
];

/// "de_DE.UTF-8" → ["deu"]; zh by region; sr@latin → srp_latn
pub fn codes_for(locale: &str) -> Vec<String> {
    let l = locale.trim();
    if l.is_empty() || l == "C" || l == "POSIX" || l.starts_with("C.") {
        return vec![];
    }
    let (main, modifier) = match l.split_once('@') {
        Some((a, b)) => (a, b.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("")),
        None => (l, ""),
    };
    let main = main.split('.').next().unwrap_or("");
    let (lang, region) = main.split_once('_').unwrap_or((main, ""));
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return vec![];
    }
    let region = region.to_uppercase();
    let one = |s: &str| vec![s.to_string()];
    match lang {
        "zh" => one(if matches!(region.as_str(), "TW" | "HK" | "MO" | "HANT") { "chi_tra" } else { "chi_sim" }),
        "sr" => one(if modifier == "latin" { "srp_latn" } else { "srp" }),
        "uz" => one(if modifier == "cyrillic" { "uzb_cyrl" } else { "uzb" }),
        "az" if modifier == "cyrillic" => one("aze_cyrl"),
        _ => CODES.iter().find(|(k, _)| *k == lang).map(|(_, v)| one(v)).unwrap_or_default(),
    }
}

/// "de" → ["deu"]; "in(hin)", or "in" with variant "tam" → ["tam"]
pub fn layout_langs(layout: &str, variant: &str) -> Vec<String> {
    let mut l = layout.trim().to_lowercase();
    let mut v = variant.trim().to_lowercase();
    if let Some((a, rest)) = l.clone().split_once('(') {
        if let Some(inner) = rest.strip_suffix(')') {
            l = a.to_string();
            if v.is_empty() {
                v = inner.to_string();
            }
        }
    }
    if let (Some((_, vs)), false) = (VARIANTS.iter().find(|(k, _)| *k == l), v.is_empty()) {
        let head = v.split(['-', '_']).next().unwrap_or("").to_string();
        let find = |k: &str| vs.iter().find(|(n, _)| *n == k).map(|(_, c)| *c);
        let mut c = find(&v).or_else(|| find(&head));
        if c.is_none() {
            for (k, code) in vs.iter() {
                if k.len() >= 3 && head.starts_with(k) {
                    c = Some(*code);
                }
            }
        }
        if let Some(c) = c {
            return if c.is_empty() { vec![] } else { vec![c.to_string()] };
        }
    }
    LAYOUTS.iter().find(|(k, _)| *k == l).map(|(_, v)| v.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
}

/// The languages wanted, in order: the system's, then the layouts', English last.
/// `env`: LC_ALL, LC_MESSAGES, LANG, LANGUAGE; `layouts`: (layout, variant)
pub fn wanted(env: &[(&str, String)], layouts: &[(String, String)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |c: String| {
        if !out.contains(&c) {
            out.push(c);
        }
    };
    let get = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        for c in codes_for(&get(k)) {
            add(c);
        }
    }
    for part in get("LANGUAGE").split(':') {
        for c in codes_for(part) {
            add(c);
        }
    }
    for (l, v) in layouts {
        for c in layout_langs(l, v) {
            add(c);
        }
    }
    out.retain(|c| c != "eng");
    out.push("eng".into());
    out
}

/// `tesseract --list-langs` output → the codes
pub fn installed_set(list: &str) -> HashSet<String> {
    list.lines().filter(|l| !l.starts_with("List of")).map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

/// (langs to read with, wanted ones whose data is missing)
pub fn pick(env: &[(&str, String)], installed: &str, layouts: &[(String, String)]) -> (Vec<String>, Vec<String>) {
    let have = installed_set(installed);
    let (mut langs, mut missing) = (Vec::new(), Vec::new());
    for c in wanted(env, layouts) {
        if c == "eng" {
            continue;
        }
        if have.contains(&c) {
            langs.push(c);
        } else {
            missing.push(c);
        }
    }
    if have.contains("eng") || langs.is_empty() {
        langs.push("eng".into());
    }
    (langs, missing)
}

const NAMES: &[(&str, &str)] = &[
    ("afr", "Afrikaans"), ("amh", "Amharic"), ("ara", "Arabic"), ("asm", "Assamese"), ("aze", "Azerbaijani"), ("aze_cyrl", "Azerbaijani (Cyrillic)"),
    ("bel", "Belarusian"), ("ben", "Bengali"), ("bod", "Tibetan"), ("bos", "Bosnian"), ("bre", "Breton"), ("bul", "Bulgarian"), ("cat", "Catalan"),
    ("ces", "Czech"), ("chi_sim", "Chinese (Simplified)"), ("chi_tra", "Chinese (Traditional)"), ("cym", "Welsh"), ("dan", "Danish"),
    ("deu", "German"), ("div", "Dhivehi"), ("dzo", "Dzongkha"), ("ell", "Greek"), ("eng", "English"), ("epo", "Esperanto"), ("est", "Estonian"),
    ("eus", "Basque"), ("fao", "Faroese"), ("fas", "Persian"), ("fil", "Filipino"), ("fin", "Finnish"), ("fra", "French"), ("gla", "Scottish Gaelic"),
    ("gle", "Irish"), ("glg", "Galician"), ("guj", "Gujarati"), ("hat", "Haitian Creole"), ("heb", "Hebrew"), ("hin", "Hindi"), ("hrv", "Croatian"),
    ("hun", "Hungarian"), ("hye", "Armenian"), ("iku", "Inuktitut"), ("ind", "Indonesian"), ("isl", "Icelandic"), ("ita", "Italian"),
    ("jav", "Javanese"), ("jpn", "Japanese"), ("kan", "Kannada"), ("kat", "Georgian"), ("kaz", "Kazakh"), ("khm", "Khmer"), ("kir", "Kyrgyz"),
    ("kmr", "Kurdish"), ("kor", "Korean"), ("lao", "Lao"), ("lat", "Latin"), ("lav", "Latvian"), ("lit", "Lithuanian"), ("ltz", "Luxembourgish"),
    ("mal", "Malayalam"), ("mar", "Marathi"), ("mkd", "Macedonian"), ("mlt", "Maltese"), ("mon", "Mongolian"), ("mri", "Maori"), ("msa", "Malay"),
    ("mya", "Burmese"), ("nep", "Nepali"), ("nld", "Dutch"), ("nor", "Norwegian"), ("oci", "Occitan"), ("ori", "Odia"), ("pan", "Punjabi"),
    ("pol", "Polish"), ("por", "Portuguese"), ("pus", "Pashto"), ("que", "Quechua"), ("ron", "Romanian"), ("rus", "Russian"), ("san", "Sanskrit"),
    ("sin", "Sinhala"), ("slk", "Slovak"), ("slv", "Slovenian"), ("snd", "Sindhi"), ("spa", "Spanish"), ("sqi", "Albanian"), ("srp", "Serbian"),
    ("srp_latn", "Serbian (Latin)"), ("sun", "Sundanese"), ("swa", "Swahili"), ("swe", "Swedish"), ("tam", "Tamil"), ("tat", "Tatar"),
    ("tel", "Telugu"), ("tgk", "Tajik"), ("tgl", "Tagalog"), ("tha", "Thai"), ("tir", "Tigrinya"), ("ton", "Tongan"), ("tur", "Turkish"),
    ("uig", "Uyghur"), ("ukr", "Ukrainian"), ("urd", "Urdu"), ("uzb", "Uzbek"), ("uzb_cyrl", "Uzbek (Cyrillic)"), ("vie", "Vietnamese"),
    ("yid", "Yiddish"), ("yor", "Yoruba"),
];

/// Settings' name for a code (the code itself when unknown)
pub fn name_of(code: &str) -> String {
    NAMES.iter().find(|(k, _)| *k == code).map(|(_, v)| v.to_string()).unwrap_or_else(|| code.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_and_layouts_then_english() {
        let env = [("LANG", "de_DE.UTF-8".to_string())];
        let w = wanted(&env, &[("in".into(), "tam".into())]);
        assert_eq!(w, vec!["deu", "tam", "eng"]);
        assert_eq!(codes_for("zh_TW.UTF-8"), vec!["chi_tra"]);
        assert!(codes_for("C.UTF-8").is_empty());
    }

    #[test]
    fn only_installed_languages_are_read() {
        let env = [("LANG", "fr_FR.UTF-8".to_string())];
        let (langs, missing) = pick(&env, "List of available languages (2):\neng\nosd\n", &[]);
        assert_eq!(langs, vec!["eng"]);
        assert_eq!(missing, vec!["fra"]);
        assert_eq!(layout_langs("in(hin)", ""), vec!["hin"]);
    }
}
