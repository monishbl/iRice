//! The picker's data, as shell/modules/emoji/EmojiStore.qml: the emoji,
//! kaomoji and symbol lists (shell/assets/data/emoji.json, from
//! tools/gen-emoji-data.py), search, what you use most and the skin tone
//! you chose per emoji ($XDG_STATE_HOME/tessera/emoji.json, the same file
//! and format as the QML picker's, so either one finds the other's).

use serde_json::Value;
use std::collections::HashMap;

/// An emoji's skin tones (emoji.json's last field: 0, 1, or its five forms)
#[derive(Clone, Debug, PartialEq)]
pub enum Tones {
    None,
    /// a modifier after its first character
    Modifier,
    /// five spelled-out forms (people holding hands…)
    Forms(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Emoji {
    pub c: String,
    /// its name, "grinning face"
    pub n: String,
    nl: String,
    /// its shortcode, without colons
    pub s: String,
    pub t: Tones,
    hay: String,
}

/// A kaomoji or text symbol
#[derive(Clone, Debug)]
pub struct Item {
    pub c: String,
    pub n: String,
    nl: String,
    hay: String,
}

#[derive(Default)]
pub struct Data {
    /// (id, title), in order
    pub groups: Vec<(String, String)>,
    pub emoji: Vec<Emoji>,
    pub by_char: HashMap<String, usize>,
    /// group -> its emoji (indexes), in order
    pub by_group: Vec<Vec<usize>>,
    /// sections: (title, the range of `kaomoji_flat` / `symbols_flat`)
    pub kaomoji: Vec<(String, std::ops::Range<usize>)>,
    pub symbols: Vec<(String, std::ops::Range<usize>)>,
    pub kaomoji_flat: Vec<Item>,
    pub symbols_flat: Vec<Item>,
}

/// Search text: a leading space and single spaces between words of a-z0-9,
/// so "starts a word" is one `contains(" " + word)`
fn words(t: &str) -> String {
    let mut out = String::from(" ");
    let mut gap = false;
    for ch in t.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if gap && out.len() > 1 {
                out.push(' ');
            }
            gap = false;
            out.push(ch);
        } else {
            gap = true;
        }
    }
    out
}

fn snake(n: &str) -> String {
    words(n).trim().replace(' ', "_")
}

impl Data {
    pub fn parse(text: &str) -> Result<Self, String> {
        let j: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let s = |v: &Value| v.as_str().unwrap_or("").to_string();
        let groups: Vec<(String, String)> = j["groups"].as_array().ok_or("no groups")?.iter().map(|g| (s(&g[0]), s(&g[1]))).collect();
        let mut d = Data { by_group: vec![Vec::new(); groups.len()], groups, ..Default::default() };
        for (i, e) in j["emoji"].as_array().ok_or("no emoji")?.iter().enumerate() {
            let (c, n, k) = (s(&e[0]), s(&e[1]), s(&e[2]));
            let sc = Some(s(&e[3])).filter(|x| !x.is_empty()).unwrap_or_else(|| snake(&n));
            let t = match &e[5] {
                Value::Array(f) if f.len() == 5 => Tones::Forms(f.iter().map(s).collect()),
                Value::Number(x) if x.as_i64() == Some(1) => Tones::Modifier,
                _ => Tones::None,
            };
            let g = e[4].as_u64().unwrap_or(0) as usize;
            let hay = words(&format!("{n} {k} {sc}"));
            d.by_char.insert(c.clone(), i);
            if let Some(list) = d.by_group.get_mut(g) {
                list.push(i);
            }
            d.emoji.push(Emoji { nl: n.to_lowercase(), c, n, s: sc, t, hay });
        }
        let sections = |key: &str, flat: &mut Vec<Item>| -> Vec<(String, std::ops::Range<usize>)> {
            let mut out = Vec::new();
            for sec in j[key].as_array().into_iter().flatten() {
                let from = flat.len();
                for it in sec[1].as_array().into_iter().flatten() {
                    let (c, n) = (s(&it[0]), s(&it[1]));
                    flat.push(Item { hay: format!(" {}{}", c.to_lowercase(), words(&n)), nl: n.to_lowercase(), c, n });
                }
                out.push((s(&sec[0]), from..flat.len()));
            }
            out
        };
        let mut kf = Vec::new();
        d.kaomoji = sections("kaomoji", &mut kf);
        d.kaomoji_flat = kf;
        let mut sf = Vec::new();
        d.symbols = sections("symbols", &mut sf);
        d.symbols_flat = sf;
        Ok(d)
    }

    pub fn load() -> Result<Self, String> {
        let path = crate::util::asset("shell/assets/data/emoji.json").ok_or("emoji.json is missing")?;
        Self::parse(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
    }
}

// ── Search ─────────────────────────────────────────────────────────────────

trait Searchable {
    fn hay(&self) -> &str;
    fn nl(&self) -> &str;
    fn c(&self) -> &str;
}
impl Searchable for Emoji {
    fn hay(&self) -> &str {
        &self.hay
    }
    fn nl(&self) -> &str {
        &self.nl
    }
    fn c(&self) -> &str {
        &self.c
    }
}
impl Searchable for Item {
    fn hay(&self) -> &str {
        &self.hay
    }
    fn nl(&self) -> &str {
        &self.nl
    }
    fn c(&self) -> &str {
        &self.c
    }
}

/// Lower is better; None: no match. Every word of the query has to start a
/// word of the name, keywords or shortcode
fn score(e: &impl Searchable, words: &[String], q: &str) -> Option<f64> {
    let hay = e.hay();
    if !words.iter().all(|w| hay.contains(&format!(" {w}"))) {
        return None;
    }
    let n = e.nl();
    let len = n.chars().count() as f64 / 100.0;
    Some(if n == q {
        0.0
    } else if n.starts_with(q) {
        1.0 + len
    } else if hay.contains(&format!(" {q} ")) || hay.ends_with(&format!(" {q}")) {
        // the query is a whole word: the usual order decides (smileys
        // before "cat with wry smile" for "smile")
        1.5
    } else if n.contains(q) {
        2.0 + len
    } else {
        3.0 + len
    })
}

fn rank<T: Searchable>(list: &[T], words: &[String], q: &str, max: usize) -> Vec<usize> {
    let mut hits: Vec<(f64, usize)> = list.iter().enumerate().filter_map(|(i, e)| score(e, words, q).map(|s| (s, i))).collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    hits.into_iter().take(max).map(|h| h.1).collect()
}

/// Characters typed as they are ("^_^", "<3", "→")
fn literal<T: Searchable>(list: &[T], raw: &str, max: usize) -> Vec<usize> {
    list.iter().enumerate().filter(|(_, e)| e.c().to_lowercase().contains(raw)).map(|(i, _)| i).take(max).collect()
}

/// Literal matches first, then the word matches not already in
fn merge(a: Vec<usize>, b: Vec<usize>, max: usize) -> Vec<usize> {
    let mut out: Vec<usize> = a.into_iter().take(max).collect();
    for x in b {
        if out.len() >= max {
            break;
        }
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// Indexes into emoji, kaomoji_flat and symbols_flat
#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub emoji: Vec<usize>,
    pub kaomoji: Vec<usize>,
    pub symbols: Vec<usize>,
}

impl Data {
    pub fn search(&self, query: &str, max: usize) -> Found {
        let raw = query.trim().to_lowercase();
        if raw.is_empty() {
            return Found::default();
        }
        // (":smile:" and "thumbs_up" read as words)
        let q = raw.trim_matches(':').split(|c: char| c == '_' || c.is_whitespace()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
        let words: Vec<String> = q
            .split(|c| c == ' ' || c == '-')
            .map(|w| w.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect::<String>())
            .filter(|w| !w.is_empty())
            .collect();
        // punctuation (beyond a :shortcode:) means characters
        let chars = raw.chars().any(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c.is_whitespace() || c == '_' || c == ':'));
        if words.is_empty() {
            return Found { emoji: literal(&self.emoji, &raw, max), kaomoji: literal(&self.kaomoji_flat, &raw, max), symbols: literal(&self.symbols_flat, &raw, max) };
        }
        let (em, ka, sy) = (rank(&self.emoji, &words, &q, max), rank(&self.kaomoji_flat, &words, &q, max), rank(&self.symbols_flat, &words, &q, max));
        if !chars {
            return Found { emoji: em, kaomoji: ka, symbols: sy };
        }
        Found {
            emoji: merge(literal(&self.emoji, &raw, max), em, max),
            kaomoji: merge(literal(&self.kaomoji_flat, &raw, max), ka, max),
            symbols: merge(literal(&self.symbols_flat, &raw, max), sy, max),
        }
    }
}

// ── Skin tones ─────────────────────────────────────────────────────────────

pub const TONE_NAMES: [&str; 6] = ["Default", "Light", "Medium-light", "Medium", "Medium-dark", "Dark"];
const MODIFIERS: [char; 5] = ['\u{1F3FB}', '\u{1F3FC}', '\u{1F3FD}', '\u{1F3FE}', '\u{1F3FF}'];

impl Emoji {
    pub fn toned(&self) -> bool {
        self.t != Tones::None
    }

    /// The emoji in tone 1..5 (0: as it is). The modifier goes after the
    /// first character, in place of an emoji-style selector (U+FE0F)
    pub fn with_tone(&self, tone: u8) -> String {
        if tone == 0 || tone > 5 {
            return self.c.clone();
        }
        match &self.t {
            Tones::None => self.c.clone(),
            Tones::Forms(f) => f.get(tone as usize - 1).cloned().unwrap_or_else(|| self.c.clone()),
            Tones::Modifier => {
                let mut ch = self.c.chars();
                let Some(first) = ch.next() else { return self.c.clone() };
                let rest = ch.as_str();
                let rest = rest.strip_prefix('\u{FE0F}').unwrap_or(rest);
                format!("{first}{}{rest}", MODIFIERS[tone as usize - 1])
            }
        }
    }
}

/// "U+2192": a symbol's code points
pub fn code_points(s: &str) -> String {
    s.chars().map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" ")
}

// ── What you use, and your tones ───────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Use {
    /// emoji | kaomoji | symbol | gif
    pub k: String,
    pub n: u32,
    /// last used, ms
    pub t: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Remembered {
    pub usage: HashMap<String, Use>,
    /// base emoji -> 1..5
    pub tones: HashMap<String, u8>,
    /// a stable random id for KLIPY (its API asks for one per user)
    pub customer_id: String,
}

const WEEK_MS: f64 = 604_800_000.0;

impl Remembered {
    pub fn path() -> std::path::PathBuf {
        crate::util::state_dir().join("emoji.json")
    }

    pub fn from_json(text: &str) -> Self {
        let j: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let mut r = Self::default();
        for (c, u) in j["usage"].as_object().into_iter().flatten() {
            r.usage.insert(c.clone(), Use { k: u["k"].as_str().unwrap_or("emoji").into(), n: u["n"].as_f64().unwrap_or(0.0) as u32, t: u["t"].as_f64().unwrap_or(0.0) as i64 });
        }
        for (c, t) in j["tones"].as_object().into_iter().flatten() {
            if let Some(t) = t.as_u64().filter(|t| (1..=5).contains(t)) {
                r.tones.insert(c.clone(), t as u8);
            }
        }
        r.customer_id = j["customerId"].as_str().unwrap_or("").into();
        r
    }

    pub fn to_json(&self) -> String {
        let usage: serde_json::Map<String, Value> = self.usage.iter().map(|(c, u)| (c.clone(), serde_json::json!({"k": u.k, "n": u.n, "t": u.t}))).collect();
        serde_json::json!({"usage": usage, "tones": self.tones, "customerId": self.customer_id}).to_string()
    }

    pub fn load() -> Self {
        let mut r = std::fs::read_to_string(Self::path()).map(|t| Self::from_json(&t)).unwrap_or_default();
        if r.customer_id.is_empty() {
            r.customer_id = random_id();
            r.save();
        }
        r
    }

    pub fn save(&self) {
        if let Err(e) = crate::util::write_atomic(&Self::path(), self.to_json().as_bytes()) {
            eprintln!("tessera-ui: emoji: can't save what you used: {e}");
        }
    }

    /// Most used first, weighted toward recent use (a week halves the weight)
    pub fn frequent(&self, kind: &str, max: usize, now: i64) -> Vec<String> {
        let mut v: Vec<(&String, f64)> = self.usage.iter().filter(|(_, u)| u.k == kind).map(|(c, u)| (c, u.n as f64 * 0.5f64.powf((now - u.t) as f64 / WEEK_MS))).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        v.into_iter().take(max).map(|x| x.0.clone()).collect()
    }

    pub fn record(&mut self, text: &str, kind: &str, now: i64) {
        let n = self.usage.get(text).map_or(0, |u| u.n);
        self.usage.insert(text.into(), Use { k: kind.into(), n: (n + 1).min(999), t: now });
        // the file stays small: the 200 most recent
        if self.usage.len() > 200 {
            let mut by_age: Vec<(String, i64)> = self.usage.iter().map(|(c, u)| (c.clone(), u.t)).collect();
            by_age.sort_by_key(|x| x.1);
            for (c, _) in by_age.into_iter().take(self.usage.len() - 200) {
                self.usage.remove(&c);
            }
        }
    }

    pub fn tone(&self, e: &Emoji) -> u8 {
        self.tones.get(&e.c).copied().unwrap_or(0)
    }

    pub fn set_tone(&mut self, e: &Emoji, tone: u8) {
        if tone == 0 {
            self.tones.remove(&e.c);
        } else {
            self.tones.insert(e.c.clone(), tone.min(5));
        }
    }

    /// How it shows: in the tone you chose for it
    pub fn display(&self, e: &Emoji) -> String {
        e.with_tone(self.tone(e))
    }
}

fn random_id() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        let t = crate::util::now_ms() as u128 ^ ((std::process::id() as u128) << 64);
        b = t.to_le_bytes();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A small list in emoji.json's shape
    pub fn sample() -> Data {
        Data::parse(
            r#"{"version":1,"groups":[["smileys","Smileys & people"],["animals","Animals & nature"]],
            "emoji":[
              ["😀","grinning face","cheerful grin happy smile","grinning",0,0],
              ["😺","grinning cat","cat face smile","",1,0],
              ["😼","cat with wry smile","cat face ironic smile wry","smirk_cat",1,0],
              ["👋","waving hand","bye hello wave","wave",0,1],
              ["🧑‍🤝‍🧑","people holding hands","couple hand","",0,["A","B","C","D","E"]],
              ["❤️","red heart","love","heart",0,0],
              ["☝️","index pointing up","finger point","point_up",0,1]
            ],
            "kaomoji":[["Happy",[["(＾▽＾)","happy smile joy"],["^_^","happy smile"]]],["Cats",[["(=^･ω･^=)","cat"]]]],
            "symbols":[["Arrows",[["→","rightwards arrow"],["←","leftwards arrow"]]],["Hearts",[["<3","heart"]]]]}"#,
        )
        .unwrap()
    }

    fn names(d: &Data, f: &[usize]) -> Vec<String> {
        f.iter().map(|&i| d.emoji[i].n.clone()).collect()
    }

    #[test]
    fn reads_the_list() {
        let d = sample();
        assert_eq!(d.groups.len(), 2);
        assert_eq!(d.by_group, vec![vec![0, 3, 4, 5, 6], vec![1, 2]]);
        assert_eq!(d.emoji[1].s, "grinning_cat"); // (no shortcode: one from the name)
        assert_eq!(d.kaomoji.len(), 2);
        assert_eq!(d.kaomoji[1].1, 2..3);
        assert_eq!(d.symbols_flat[2].c, "<3");
        assert!(Data::parse("{}").is_err() && Data::parse("nope").is_err());
    }

    #[test]
    fn the_real_list_loads() {
        let d = Data::load().expect("shell/assets/data/emoji.json");
        assert!(d.emoji.len() > 1500 && d.groups.len() == 8);
        assert!(!d.kaomoji_flat.is_empty() && !d.symbols_flat.is_empty());
        // a search over all of it is quick (it runs on every keystroke)
        let t = std::time::Instant::now();
        let f = d.search("smile", 120);
        assert!(!f.emoji.is_empty());
        assert!(t.elapsed().as_millis() < 200, "{:?}", t.elapsed());
    }

    #[test]
    fn search_ranks_like_the_shell() {
        let d = sample();
        // the exact name first (the cat's keywords have "face" too)
        assert_eq!(names(&d, &d.search("grinning face", 10).emoji), vec!["grinning face", "grinning cat"]);
        assert_eq!(names(&d, &d.search("grin", 10).emoji), vec!["grinning cat", "grinning face"]);
        // a whole word: the list's order (smileys before the wry cat)
        assert_eq!(names(&d, &d.search("smile", 10).emoji), vec!["grinning face", "grinning cat", "cat with wry smile"]);
        // every word has to start a word somewhere
        assert_eq!(names(&d, &d.search("cat wry", 10).emoji), vec!["cat with wry smile"]);
        assert!(d.search("at", 10).emoji.is_empty());
        // shortcodes, with or without colons and underscores
        assert_eq!(names(&d, &d.search(":smirk_cat:", 10).emoji), vec!["cat with wry smile"]);
        assert_eq!(names(&d, &d.search("point up", 10).emoji), vec!["index pointing up"]);
        // kaomoji and symbols by their names
        assert_eq!(d.search("cat", 10).kaomoji, vec![2]);
        assert_eq!(d.search("arrow", 10).symbols, vec![0, 1]);
        assert_eq!(d.search("  ", 10), Found::default());
        assert_eq!(d.search("grin", 1).emoji.len(), 1);
    }

    #[test]
    fn characters_are_found_as_typed() {
        let d = sample();
        // only punctuation: literal matches
        assert_eq!(d.search("^_^", 10).kaomoji, vec![1]);
        assert_eq!(d.search("→", 10).symbols, vec![0]);
        // characters and words: the literal ones first, then the rest
        assert_eq!(d.search("<3", 10).symbols, vec![2]);
        assert!(d.search("→", 10).emoji.is_empty());
    }

    #[test]
    fn skin_tones() {
        let d = sample();
        let wave = &d.emoji[3];
        assert!(wave.toned() && !d.emoji[0].toned());
        assert_eq!(wave.with_tone(0), "👋");
        assert_eq!(wave.with_tone(1), "👋\u{1F3FB}");
        assert_eq!(wave.with_tone(5), "👋\u{1F3FF}");
        // the emoji-style selector gives way to the modifier
        assert_eq!(d.emoji[6].with_tone(3), "☝\u{1F3FD}");
        // spelled-out forms
        assert_eq!(d.emoji[4].with_tone(2), "B");
        // no tones: as it is, whatever is asked
        assert_eq!(d.emoji[0].with_tone(4), "😀");
        let mut r = Remembered::default();
        r.set_tone(wave, 4);
        assert_eq!(r.display(wave), "👋\u{1F3FE}");
        r.set_tone(wave, 0);
        assert_eq!(r.display(wave), "👋");
        assert_eq!(code_points("→ "), "U+2192 U+0020");
        assert_eq!(code_points("😀"), "U+1F600");
    }

    #[test]
    fn recents_weigh_count_and_age() {
        let mut r = Remembered::default();
        let now = 10 * WEEK_MS as i64;
        r.record("😀", "emoji", now - 2 * WEEK_MS as i64);
        for _ in 0..3 {
            r.record("😀", "emoji", now - 2 * WEEK_MS as i64); // 4 uses, two weeks ago: weight 1
        }
        r.record("👋", "emoji", now);
        r.record("👋", "emoji", now); // 2 uses now: weight 2
        r.record("^_^", "kaomoji", now);
        assert_eq!(r.frequent("emoji", 10, now), vec!["👋", "😀"]);
        assert_eq!(r.frequent("emoji", 1, now), vec!["👋"]);
        assert_eq!(r.frequent("kaomoji", 10, now), vec!["^_^"]);
        assert!(r.frequent("gif", 10, now).is_empty());
        assert_eq!(r.usage["😀"].n, 4);
        // only the 200 most recent are kept
        for i in 0..250 {
            r.record(&format!("x{i}"), "symbol", now + i);
        }
        assert_eq!(r.usage.len(), 200);
        assert!(r.usage.contains_key("x249") && !r.usage.contains_key("😀"));
    }

    #[test]
    fn remembered_round_trips_the_shells_file() {
        // as the QML picker writes it
        let r = Remembered::from_json(r#"{"usage":{"😀":{"k":"emoji","n":3,"t":1730000000000}},"tones":{"👋":2,"x":9},"customerId":"abc"}"#);
        assert_eq!(r.usage["😀"], Use { k: "emoji".into(), n: 3, t: 1730000000000 });
        assert_eq!(r.tones.get("👋"), Some(&2));
        assert!(!r.tones.contains_key("x")); // (not a tone)
        assert_eq!(r.customer_id, "abc");
        assert_eq!(Remembered::from_json(&r.to_json()), r);
        assert_eq!(Remembered::from_json("broken"), Remembered::default());
        assert_eq!(random_id().len(), 32);
    }
}
