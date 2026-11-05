//! Search's slow answers, each asked off the UI thread as Search.qml asks
//! them: plain-language answers (assets/scripts/answers.py --serve, kept
//! running while search is used, stopped after two idle minutes),
//! conversions (assets/scripts/convert.sh, qalc), definitions
//! (dictionaryapi.dev), files by name (plocate, else find) and documents by
//! what's in them (ripgrep). Also the lists it reads once: emoji.js, and the
//! icon fonts' maps (phosphor.js, material-symbols.js).

use super::rank::{Answer, Definition};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::OnceLock;

fn shell_file(rel: &str) -> Option<PathBuf> {
    crate::services::apps::shell_file(rel)
}

// ── answers.py ──────────────────────────────────────────────────────────

/// One line of answers.py's: its id and the answer (no title: none)
pub fn parse_answer(line: &str) -> Option<(u64, Answer)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let id = v["id"].as_u64()?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    Some((id, Answer { kind: s("kind"), title: s("title"), subtitle: s("subtitle"), copy: s("copy") }))
}

/// answers.py --serve, started when first asked
pub struct Answerer {
    child: Option<(Child, ChildStdin)>,
    pub id: u64,
}

impl Answerer {
    pub fn new() -> Self {
        Self { child: None, id: 0 }
    }

    /// Ask (starting it if needed); replies come to `got` from a thread
    pub fn ask(&mut self, q: &str, got: impl Fn(u64, Answer) + Send + Clone + 'static) {
        if self.child.as_mut().is_some_and(|(c, _)| c.try_wait().ok().flatten().is_some()) {
            self.child = None;
        }
        if self.child.is_none() {
            let Some(script) = shell_file("shell/assets/scripts/answers.py") else { return };
            let Ok(mut c) = Command::new("python3").arg("-I").arg(script).arg("--serve").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                return;
            };
            let (Some(stdin), Some(stdout)) = (c.stdin.take(), c.stdout.take()) else { return };
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some((id, a)) = parse_answer(&line) {
                        got(id, a);
                    }
                }
            });
            self.child = Some((c, stdin));
        }
        self.id += 1;
        let line = serde_json::json!({"id": self.id, "q": q}).to_string() + "\n";
        if let Some((_, stdin)) = &mut self.child {
            if stdin.write_all(line.as_bytes()).and_then(|_| stdin.flush()).is_err() {
                self.child = None;
            }
        }
    }

    /// Unused a while: it stops
    pub fn stop(&mut self) {
        if let Some((mut c, stdin)) = self.child.take() {
            drop(stdin);
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for Answerer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── convert.sh, the dictionary ──────────────────────────────────────────

/// qalc's answer for "10 km to mi" ("" none)
pub fn convert(q: &str) -> String {
    let Some(script) = shell_file("shell/assets/scripts/convert.sh") else { return String::new() };
    Command::new("bash").arg(script).arg(q).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// dictionaryapi.dev's first meaning
pub fn parse_definition(word: &str, json: &str) -> Definition {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let e = &v[0];
    let m = &e["meanings"][0];
    let text = m["definitions"][0]["definition"].as_str();
    match (e["word"].as_str(), text) {
        (Some(w), Some(t)) => {
            let phonetic = e["phonetic"].as_str().map(str::to_string).or_else(|| e["phonetics"].as_array().and_then(|a| a.iter().find_map(|p| p["text"].as_str().map(str::to_string)))).unwrap_or_default();
            Definition { word: w.into(), phonetic, part: m["partOfSpeech"].as_str().unwrap_or("").into(), text: t.into(), missing: false }
        }
        _ => Definition { word: word.into(), missing: true, ..Default::default() },
    }
}

pub fn define(word: &str) -> Definition {
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", crate::surfaces::emoji::klipy::encode(word));
    let out = Command::new("curl").args(["-sf", "--max-time", "5", &url]).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    parse_definition(word, &out)
}

// ── Files ───────────────────────────────────────────────────────────────

/// The name search's command (Search.qml's fileDebounce): plocate's index
/// when there is one, else find; lines "d|f\tpath"
pub fn names_command(fq: &str, plocate: bool) -> Vec<String> {
    let script = if plocate {
        "plocate -i -l 60 -- \"$1\" 2>/dev/null | grep -F \"$HOME/\" | grep -v '/\\.' | head -25 | while IFS= read -r p; do [ -d \"$p\" ] && t=d || t=f; printf '%s\\t%s\\n' \"$t\" \"$p\"; done"
    } else {
        "find \"$HOME\" -maxdepth 6 \\( -name '.*' -o -name node_modules -o -name target \\) -prune -o -iname \"*$1*\" -printf '%y\\t%p\\n' 2>/dev/null | head -25"
    };
    ["sh", "-c", script, "sh", fq].iter().map(|s| s.to_string()).collect()
}

/// Documents with it inside (ripgrep, the usual folders, not code trees)
pub fn contents_command(fq: &str) -> Vec<String> {
    let script = "cd \"$HOME\" && timeout 3 rg -l -i -F --max-filesize 4M --max-depth 5 -g '!.*' -g '!node_modules' -t txt -t markdown -t org -t tex -t csv -t json -t yaml -t html -t py -- \"$1\" Documents Desktop Downloads Notes notes 2>/dev/null | head -15 | sed \"s|^|$HOME/|\"";
    ["sh", "-c", script, "sh", fq].iter().map(|s| s.to_string()).collect()
}

/// "type\tpath" lines: (is a folder, path)
pub fn parse_names(out: &str) -> Vec<(bool, String)> {
    out.lines().filter_map(|l| l.split_once('\t')).map(|(t, p)| (t == "d", p.to_string())).collect()
}

pub fn run_lines(argv: &[String]) -> String {
    Command::new(&argv[0]).args(&argv[1..]).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

// ── Lists read once ─────────────────────────────────────────────────────

/// `"key": "value"` and `["a", "b"]` lines of a generated .js list
fn js_pairs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for l in text.lines() {
        let t = l.trim().trim_end_matches(',');
        if t.starts_with('[') {
            if let Ok([a, b]) = serde_json::from_str::<[String; 2]>(t) {
                out.push((a, b));
            }
        } else if t.starts_with('"') {
            if let Ok(v) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&format!("{{{t}}}")) {
                for (k, v) in v {
                    if let Some(s) = v.as_str() {
                        out.push((k, s.to_string()));
                    }
                }
            }
        }
    }
    out
}

/// emoji.js: (emoji, name)
pub fn emoji() -> &'static [(String, String)] {
    static E: OnceLock<Vec<(String, String)>> = OnceLock::new();
    E.get_or_init(|| shell_file("shell/common/emoji.js").and_then(|p| std::fs::read_to_string(p).ok()).map(|t| js_pairs(&t)).unwrap_or_default())
}

fn map(rel: &str) -> HashMap<String, String> {
    shell_file(rel).and_then(|p| std::fs::read_to_string(p).ok()).map(|t| js_pairs(&t).into_iter().collect()).unwrap_or_default()
}

/// A glyph as Icon.qml draws it: Material's Material Symbol when there's one
/// (1.2× the size), else Phosphor's. (text, font family, size factor)
pub fn glyph(name: &str, material: bool, fill: bool, bold: bool) -> (String, String, f32) {
    static P: OnceLock<HashMap<String, String>> = OnceLock::new();
    static S: OnceLock<HashMap<String, String>> = OnceLock::new();
    if material {
        if let Some(s) = S.get_or_init(|| map("shell/common/material-symbols.js")).get(name) {
            return (s.clone(), "Tessera Symbols".into(), 1.2);
        }
    }
    let p = P.get_or_init(|| map("shell/common/phosphor.js")).get(name).cloned().unwrap_or_default();
    (p, if fill { "Phosphor-Fill" } else if bold { "Phosphor-Bold" } else { "Phosphor" }.into(), 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_lines() {
        let (id, a) = parse_answer(r#"{"kind": "math", "title": "441", "subtitle": "18% of 2450", "copy": "441", "id": 7}"#).unwrap();
        assert_eq!((id, a.title.as_str(), a.copy.as_str()), (7, "441", "441"));
        let (_, none) = parse_answer(r#"{"id": 8}"#).unwrap();
        assert!(none.title.is_empty());
        assert!(parse_answer(r#"{"id": null}"#).is_none());
        assert!(parse_answer("nonsense").is_none());
    }

    #[test]
    fn definitions() {
        let d = parse_definition("cat", r#"[{"word":"cat","phonetics":[{"audio":""},{"text":"/kæt/"}],"meanings":[{"partOfSpeech":"noun","definitions":[{"definition":"A small animal."}]}]}]"#);
        assert_eq!((d.word.as_str(), d.phonetic.as_str(), d.part.as_str(), d.text.as_str(), d.missing), ("cat", "/kæt/", "noun", "A small animal.", false));
        assert!(parse_definition("zzx", r#"{"title":"No Definitions Found"}"#).missing);
        assert!(parse_definition("zzx", "").missing);
    }

    #[test]
    fn file_searches() {
        assert_eq!(parse_names("d\t/home/u/Notes\nf\t/home/u/a b.txt\njunk\n"), vec![(true, "/home/u/Notes".into()), (false, "/home/u/a b.txt".into())]);
        // (what's typed is an argument, never part of the script)
        let c = names_command("x\"; rm -rf ~", true);
        assert_eq!((c[0].as_str(), c[4].as_str()), ("sh", "x\"; rm -rf ~"));
        assert!(!c[2].contains("rm -rf"));
        assert!(names_command("a", false)[2].starts_with("find "));
        assert!(contents_command("a")[2].contains("rg -l -i -F"));
    }

    #[test]
    fn generated_lists() {
        let e = js_pairs("var list = [\n    [\"😀\", \"grinning face\"],\n    [\"\\u00a9\", \"copyright\"]\n];");
        assert_eq!(e, vec![("😀".into(), "grinning face".into()), ("©".into(), "copyright".into())]);
        let m = js_pairs("var glyphs = {\n    \"acorn\": \"\\ueb9a\",\n    \"x\": \"\\ue4f6\"\n};");
        assert_eq!(m[0], ("acorn".into(), "\u{eb9a}".into()));
        // the repo's own lists
        assert!(emoji().len() > 1000);
        assert_eq!(glyph("magnifying-glass", false, false, false), ("\u{e30c}".into(), "Phosphor".into(), 1.0));
        assert_eq!(glyph("sparkle", false, true, false).1, "Phosphor-Fill");
        assert_eq!(glyph("magnifying-glass", true, false, false).1, "Tessera Symbols");
    }
}
