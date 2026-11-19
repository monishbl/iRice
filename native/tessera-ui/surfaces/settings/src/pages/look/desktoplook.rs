//! Appearance → Tessera's desktop look (Rust Settings only): the switch for
//! shell/assets/scripts/desktop-look.py, which gives every install the same
//! desktop as the one it was designed on: the apps' fonts (Google Sans Flex,
//! JetBrains Mono Nerd Font), the Qt style and icons (Darkly, Breeze Plus,
//! when installed), grayscale font smoothing, and the clean terminal (no
//! greeting, the starship prompt in Tessera's colours, kitty to match).
//! Off puts back what it replaced.

use super::super::super::model::{Ctx, Page, Row};
use serde_json::{json, Value};

/// The script's status (`desktop-look.py status`, JSON; Null while it's
/// read) → the switch: whether it's on, whether it can be used now (not
/// while it's being read or changed), and the words under it, with what it
/// left alone or still needs
pub fn switch_state(status: &Value, busy: bool) -> (bool, bool, String) {
    let mut words = vec!["App fonts, Qt style and icons, font smoothing and a clean terminal with the starship prompt, all in Tessera's style. Off puts back what it replaced.".to_string()];
    if status.is_null() {
        return (true, false, words.join(" "));
    }
    let on = status["on"].as_bool().unwrap_or(true);
    if on {
        if status["starship"] == false {
            words.push("starship isn't installed yet: the prompt comes once it is (sudo pacman -S starship).".into());
        }
        if status["edited"] == true {
            words.push("You changed its starship.toml, so Tessera leaves it as it is.".into());
        }
        // (what it left alone, what's still missing)
        for n in status["notes"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if n.contains("left alone") || n.contains("left to") || n.starts_with("! ") {
                let mut t = n.trim_start_matches("! ").trim_end_matches('.').to_string();
                if let Some(c) = t.get(0..1).map(str::to_uppercase) {
                    t.replace_range(0..1, &c);
                }
                words.push(format!("{t}."));
            }
        }
    }
    let error = status["error"].as_str();
    if let Some(e) = error {
        words.push(e.to_string());
    }
    (on, !busy && error.is_none(), words.join(" "))
}

const KEY: &str = "look.desktopLook";

fn status_of(script: &Option<std::path::PathBuf>) -> Value {
    match script {
        Some(s) => serde_json::from_str(&Ctx::run(&["python3", "-I", &s.to_string_lossy(), "status"])).unwrap_or_else(|_| json!({"error": "Couldn't read the desktop look's state."})),
        None => json!({"error": "desktop-look.py is missing: update Tessera."}),
    }
}

/// The group on Appearance
pub fn group(cx: &mut Ctx, p: &mut Page) {
    let script = crate::util::asset("shell/assets/scripts/desktop-look.py");
    let s1 = script.clone();
    let status = cx.once(KEY, move || status_of(&s1));
    let (on, usable, words) = switch_state(&status, cx.local_b(&format!("{KEY}.busy")));
    let t = cx.on(move |cx, _| {
        let next = !cx.local.get(KEY).and_then(|v| v["on"].as_bool()).unwrap_or(true);
        // (the switch moves at once; the script's answer replaces this)
        let mut v = cx.local.get(KEY).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
        v["on"] = json!(next);
        cx.local_set(KEY, v);
        cx.toast(if next { "Tessera's desktop look is on. Apps and terminals use it when they next start." } else { "Your own look is back. Apps and terminals use it when they next start." }, None);
        let s2 = script.clone();
        cx.spawn(KEY, move || {
            if let Some(s) = &s2 {
                let _ = Ctx::run(&["python3", "-I", &s.to_string_lossy(), if next { "on" } else { "off" }]);
            }
            status_of(&s2)
        });
    });
    let g = p.group("Apps and terminal");
    g.footer(words);
    g.row(Row::new("Tessera's desktop look").icon("terminal-window").switch(on, t).enabled(usable));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_look_switch() {
        // (while it's read: shown on, not usable yet)
        let (on, usable, _) = switch_state(&Value::Null, false);
        assert!(on && !usable);
        let st = json!({"on": true, "starship": true, "notes": ["fish: no greeting, the starship prompt", "zsh: oh-my-zsh sets your prompt, so it's left alone", "kitty: Tessera's font", "! Darkly isn't installed, so Qt apps keep their style (yay -S darkly-bin)"]});
        let (on, usable, words) = switch_state(&st, false);
        assert!(on && usable);
        assert!(words.contains("Zsh: oh-my-zsh sets your prompt, so it's left alone."), "{words}");
        assert!(words.contains("Darkly isn't installed"), "{words}");
        assert!(!words.contains("kitty:"), "only what was left alone or is missing is said: {words}");
        // (being changed: not usable)
        assert!(!switch_state(&st, true).1);
        // off: nothing about what it set up
        let (on, usable, words) = switch_state(&json!({"on": false, "notes": ["zsh: x left alone"]}), false);
        assert!(!on && usable && !words.contains("Zsh"));
        // starship missing, its file changed by you
        let (_, _, words) = switch_state(&json!({"on": true, "starship": false, "edited": true}), false);
        assert!(words.contains("sudo pacman -S starship") && words.contains("leaves it as it is"), "{words}");
        // the script missing: said, and the switch can't be used
        let (_, usable, words) = switch_state(&json!({"error": "desktop-look.py is missing: update Tessera."}), false);
        assert!(!usable && words.contains("update Tessera"));
    }
}
