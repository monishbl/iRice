//! Live Text: the text in a picture, read on this computer by tesseract
//! (shell/services/LiveText.qml, with common/LiveTextLogic.js's parseTsv,
//! search, snippet and mayIndex; nothing is uploaded). `livetext.read` gives
//! a picture's words with their boxes (the editor's and Quick Look's layer
//! select them); the answer goes to both surfaces as
//! `{"livetext": key, "result": {status, width, height, words}}`. The
//! screenshot index lets Search find a shot by its words: built in the
//! background by one low-priority process (assets/scripts/shot-index.py),
//! only for screenshots it hasn't read yet, and not on battery below 30% or
//! in power saver.
//! Differences: no Wi-Fi code lookup (WifiShare.find is the system host's):
//! `{"wifi": key, "found": []}` is said at once.

use super::service::*;
use super::util;
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use tessera_core::host::Host;

/// LiveTextLogic.parseTsv: tesseract's tsv → words with their line boxes
pub fn parse_tsv(tsv: &str) -> Value {
    let (mut width, mut height) = (0i64, 0i64);
    let mut lines: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
    let mut words = Vec::new();
    let n = |s: &str| s.trim().parse::<i64>().unwrap_or(0);
    for l in tsv.split('\n') {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 12 {
            continue;
        }
        match f[0] {
            "1" => {
                width = n(f[8]);
                height = n(f[9]);
                continue;
            }
            "4" => {
                lines.insert(format!("{}.{}.{}", f[2], f[3], f[4]), (n(f[7]), n(f[9])));
                continue;
            }
            _ => {}
        }
        let text = f[11..].join(" ").trim().to_string();
        let conf: f64 = f[10].trim().parse().unwrap_or(f64::NAN);
        if f[0] != "5" || text.is_empty() || !(conf >= 10.0) {
            continue;
        }
        if conf < 60.0 && !text.chars().any(|c| c.is_ascii_alphanumeric() || ('\u{00C0}'..='\u{FFFF}').contains(&c)) {
            continue;
        }
        let (x, y, w, h) = (n(f[6]), n(f[7]), n(f[8]), n(f[9]));
        let line = format!("{}.{}.{}", f[2], f[3], f[4]);
        let (ly, lh) = match lines.get(&line) {
            Some((by, bh)) => {
                let ly = (*by).min(y);
                (ly, (by + bh).max(y + h) - ly)
            }
            None => (y, h),
        };
        words.push(json!({"x": x, "y": y, "w": w, "h": h, "text": text, "block": f[2], "line": line, "ly": ly, "lh": lh}));
    }
    json!({"width": width, "height": height, "words": words})
}

pub fn terms_of(q: &str) -> Vec<String> {
    q.to_lowercase().split_whitespace().map(String::from).collect()
}

/// About 60 characters of `text` around char position `at`, on word
/// boundaries, on one line, "…" where it was cut
pub fn snippet(text: &str, at: usize, len: usize) -> String {
    // (whitespace runs become one space, nothing trimmed: as replace(/\s+/g, " "))
    let collapse = |t: &str| {
        let mut out = String::new();
        let mut prev_ws = false;
        for c in t.chars() {
            if c.is_whitespace() {
                if !prev_ws {
                    out.push(' ');
                }
                prev_ws = true;
            } else {
                out.push(c);
                prev_ws = false;
            }
        }
        out
    };
    let flat: Vec<char> = collapse(text).chars().collect();
    let pos = collapse(&text.chars().take(at).collect::<String>()).chars().count();
    let find = |from: usize| flat.iter().skip(from).position(|c| *c == ' ').map(|i| i + from);
    let rfind = |upto: usize| flat[..upto.min(flat.len())].iter().rposition(|c| *c == ' ');
    let mut a = pos.saturating_sub(24);
    let mut b = (pos + len + 36).min(flat.len());
    if a > 0 {
        a = match find(a) {
            Some(sp) if sp < pos => sp + 1,
            _ => a,
        };
    }
    if b < flat.len() {
        b = match rfind(b) {
            Some(sp) if sp > pos + len => sp,
            _ => b,
        };
    }
    let body: String = flat[a.min(flat.len())..b.max(a).min(flat.len())].iter().collect();
    format!("{}{}{}", if a > 0 { "…" } else { "" }, body.trim(), if b < flat.len() { "…" } else { "" })
}

/// A hit in the screenshot index
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub path: String,
    pub modified: i64,
    pub snippet: String,
}

/// LiveTextLogic.search over the index's `shots` object ({path: {t, m}})
pub fn search(shots: &Value, query: &str, limit: usize) -> Vec<Shot> {
    let terms = terms_of(query);
    let Some(map) = shots.as_object() else { return vec![] };
    if terms.is_empty() {
        return vec![];
    }
    let mut hits: Vec<Shot> = map
        .iter()
        .filter_map(|(path, e)| {
            let t = e["t"].as_str().unwrap_or("");
            let low = t.to_lowercase();
            if !terms.iter().all(|q| low.contains(q.as_str())) {
                return None;
            }
            let at = low.find(terms[0].as_str()).map(|b| low[..b].chars().count()).unwrap_or(0);
            Some(Shot { path: path.clone(), modified: e["m"].as_i64().unwrap_or(0), snippet: snippet(t, at, terms[0].chars().count()) })
        })
        .collect();
    hits.sort_by(|a, b| b.modified.cmp(&a.modified));
    hits.truncate(limit);
    hits
}

pub fn index_file() -> std::path::PathBuf {
    tessera_core::util::state_dir().join("screenshot-text.json")
}

/// The index, read from its file ("shots" object)
pub fn read_index() -> Value {
    std::fs::read_to_string(index_file()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| v["shots"].clone()).unwrap_or_else(|| json!({}))
}

/// LiveTextLogic.mayIndex
pub fn may_index(on_battery: bool, percent: i32, saver: bool) -> bool {
    !(saver || (on_battery && (0..30).contains(&percent)))
}

/// The battery and power mode, read now (Power.onBattery / percent / mode)
fn power_now() -> (bool, i32, bool) {
    let mut on_battery = false;
    let mut percent = -1;
    if let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") {
        for e in rd.flatten() {
            let p = e.path();
            if std::fs::read_to_string(p.join("type")).map(|t| t.trim() == "Battery").unwrap_or(false) {
                let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
                on_battery = status.trim() == "Discharging";
                percent = std::fs::read_to_string(p.join("capacity")).ok().and_then(|c| c.trim().parse().ok()).unwrap_or(-1);
                break;
            }
        }
    }
    let saver = Command::new("powerprofilesctl").arg("get").stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "power-saver").unwrap_or(false);
    (on_battery, percent, saver)
}

impl ScreenSvc {
    fn indexing(&self, h: &Host) -> bool {
        h.cfg()["screenshot"]["textSearch"].as_bool().unwrap_or(true)
    }

    pub(super) fn lt_start(&mut self, h: &mut Host) {
        // a number opens a phone app only when one handles tel: links
        let send = h.sender("ScreenTools");
        std::thread::spawn(move || {
            let on = Command::new("xdg-mime").args(["query", "default", "x-scheme-handler/tel"]).stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty()).unwrap_or(false);
            send(Box::new(TelHandler(on)));
        });
        self.index_soon(h);
    }

    /// Search off removes the index; on builds it (LiveText.onIndexingChanged)
    pub(super) fn lt_config(&mut self, h: &mut Host) {
        let on = self.indexing(h);
        if on && !self.index_pending && !self.indexing_now && self.index_seen_off {
            self.index_seen_off = false;
            self.index_soon(h);
        } else if !on && !self.index_seen_off {
            self.index_seen_off = true;
            let f = index_file();
            util::detached(&["rm", "-f", "--", &f.to_string_lossy(), &format!("{}.tmp", f.display())]);
        }
    }

    /// A while after a screenshot, and at start
    pub(super) fn index_soon(&mut self, h: &mut Host) {
        if !self.indexing(h) || self.index_pending {
            return;
        }
        // (the screenshot's own Live Text goes first)
        self.index_pending = true;
        util::later(h.sender("ScreenTools"), 20_000, Box::new(IndexNow));
    }

    pub(super) fn index_now(&mut self, h: &mut Host) {
        self.index_pending = false;
        if !self.indexing(h) || !self.has_tesseract() || self.indexing_now {
            return;
        }
        self.indexing_now = true;
        let (send, again) = (h.sender("ScreenTools"), h.sender("ScreenTools"));
        let (dir, file, langs, script) = (Self::shots_dir(), index_file().to_string_lossy().into_owned(), self.ocr_lang_arg(), util::script("shot-index.py"));
        std::thread::spawn(move || {
            let (b, p, s) = power_now();
            if !may_index(b, p, s) {
                // (asked while it may not run: runs once it may)
                send(Box::new(IndexEnded));
                util::later(again, 300_000, Box::new(IndexNow));
                return;
            }
            let _ = Command::new("nice")
                .args(["-n", "19", "ionice", "-c", "3", "env", "OMP_THREAD_LIMIT=1", "python3", "-I", &script, "run", &dir, &file, &langs])
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                .status();
            send(Box::new(IndexEnded));
        });
    }

    // ── A picture's words ───────────────────────────────────────────────
    fn key_of(path: &str, rev: &str) -> String {
        format!("{path}#{rev}")
    }

    fn lt_said(&self, h: &mut Host, key: &str, result: &Value) {
        util::tell_both(h, json!({"livetext": key, "result": result}));
    }

    pub(super) fn lt_read(&mut self, h: &mut Host, path: &str, rev: &str) {
        let k = Self::key_of(path, rev);
        // (and its Wi-Fi code lookup: none here)
        util::tell_both(h, json!({"wifi": k, "found": []}));
        if let Some((_, r)) = self.recent.iter().find(|(key, _)| *key == k) {
            let r = r.clone();
            return self.lt_said(h, &k, &r);
        }
        if !self.has_tesseract() {
            return self.lt_said(h, &k, &json!({"status": "missing", "width": 0, "height": 0, "words": []}));
        }
        if !self.reading.insert(k.clone()) {
            return;
        }
        let (send, path, lang) = (h.sender("ScreenTools"), path.to_string(), self.ocr_lang_arg());
        std::thread::spawn(move || {
            let out = Command::new("tesseract").args([&path, "stdout", "-l", &lang, "tsv"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let (code, text) = match out {
                Ok(o) => (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
                Err(_) => (-1, String::new()),
            };
            send(Box::new(ReadDone(k, code, text)));
        });
    }

    pub(super) fn lt_finished(&mut self, h: &mut Host, k: String, code: i32, out: String) {
        self.reading.remove(&k);
        if code != 0 {
            return self.lt_said(h, &k, &json!({"status": "error", "width": 0, "height": 0, "words": []}));
        }
        let mut r = parse_tsv(&out);
        r["status"] = json!(if r["words"].as_array().is_some_and(|w| !w.is_empty()) { "ready" } else { "none" });
        self.recent.retain(|(key, _)| *key != k);
        self.recent.insert(0, (k.clone(), r.clone()));
        self.recent.truncate(4);
        self.lt_said(h, &k, &r);
    }

    // ── Quick actions ───────────────────────────────────────────────────
    /// a: { kind: link | email | phone | wifi, value, label, wifi }
    pub(super) fn lt_act(&mut self, h: &mut Host, a: &Value) {
        let value = a["value"].as_str().unwrap_or("").to_string();
        match a["kind"].as_str().unwrap_or("") {
            "link" => h.ask("launch.openLink", json!([value])), // (web links only)
            "email" => h.ask("launch.openLink", json!([format!("mailto:{value}")])),
            "phone" if self.tel_handler => {
                let n: String = value.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
                h.ask("launch.open", json!([format!("tel:{n}")]))
            }
            "phone" => util::copy_text(&value),
            "wifi" if a["wifi"]["joinable"].as_bool().unwrap_or(false) => h.ask("network.joinWifi", json!([a["wifi"]])),
            "wifi" => util::copy_text(a["wifi"]["password"].as_str().unwrap_or("")),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_come_out_of_tsv() {
        let tsv = "1\t1\t0\t0\t0\t0\t0\t0\t640\t480\t-1\t\n4\t1\t1\t1\t1\t0\t10\t20\t100\t30\t-1\t\n5\t1\t1\t1\t1\t1\t10\t22\t50\t26\t95\tHello\n5\t1\t1\t1\t1\t2\t70\t22\t40\t26\t5\tnoise\n";
        let r = parse_tsv(tsv);
        assert_eq!((r["width"].as_i64(), r["height"].as_i64()), (Some(640), Some(480)));
        let w = r["words"].as_array().unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!((w[0]["text"].as_str(), w[0]["ly"].as_i64(), w[0]["lh"].as_i64()), (Some("Hello"), Some(20), Some(30)));
    }

    #[test]
    fn index_search_finds_all_terms_newest_first() {
        let shots = json!({"/a.png": {"t": "the quick brown fox", "m": 5}, "/b.png": {"t": "quick fox jumps", "m": 9}, "/c.png": {"t": "nothing", "m": 1}});
        let hits = search(&shots, "quick fox", 5);
        assert_eq!(hits.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(), vec!["/b.png", "/a.png"]);
        assert!(hits[0].snippet.contains("quick"));
    }

    #[test]
    fn indexing_waits_on_a_low_battery() {
        assert!(!may_index(true, 20, false));
        assert!(may_index(true, 80, false));
        assert!(may_index(false, 5, false));
        assert!(!may_index(false, 90, true));
    }
}
