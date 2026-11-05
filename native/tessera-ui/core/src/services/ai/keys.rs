//! Credentials for the AI models, where shell/services/AiKeys.qml keeps them:
//! an environment variable, else the system keyring (secret-tool, the
//! attributes `application tessera provider <slot>`, or `application irice`
//! from before the rename), else the private 0600 file
//! $XDG_STATE_HOME/tessera/ai-keys.json when no keyring runs. Keys stay in
//! memory and only ever go into request headers: never printed, logged or
//! written anywhere but those two places. A key goes to secret-tool on its
//! stdin, never on a command line.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

const ENV: &[(&str, &str)] = &[
    ("anthropic", "ANTHROPIC_API_KEY"),
    ("gemini", "GEMINI_API_KEY"),
    ("aistudio", "GEMINI_API_KEY"),
    ("vertex", "VERTEX_API_KEY"),
    ("vertex-oauth", "GOOGLE_OAUTH_ACCESS_TOKEN"),
    ("openai", "OPENAI_API_KEY"),
    ("openrouter", "OPENROUTER_API_KEY"),
];
/// The slots looked up besides each model's own key
pub const SLOTS: &[&str] = &["anthropic", "gemini", "aistudio", "vertex", "vertex-oauth", "openai", "openrouter"];

#[derive(Clone, Default)]
pub struct Keys {
    keys: HashMap<String, String>,
    /// the fallback file's own contents: only these are written back to it
    file: HashMap<String, String>,
    /// slots this process changed (a late keyring answer doesn't undo them)
    edited: std::collections::HashSet<String>,
}

/// (never the keys themselves)
impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keys({} slots)", self.keys.len())
    }
}

fn file_path() -> std::path::PathBuf {
    super::chats::state_dir().join("ai-keys.json")
}

impl Keys {
    /// The credential for a model: `provider` is its credential provider
    /// (models::credential_provider), `slot` its key; `env` reads a variable
    pub fn get_with(&self, provider: &str, slot: &str, env: impl Fn(&str) -> Option<String>) -> String {
        let var = ENV.iter().find(|e| e.0 == provider).and_then(|e| env(e.1)).filter(|v| !v.is_empty());
        let google = if provider == "aistudio" || provider == "vertex" { env("GOOGLE_API_KEY").filter(|v| !v.is_empty()) } else { None };
        let k = |s: &str| self.keys.get(s).filter(|v| !v.is_empty()).cloned();
        var.or(google).or_else(|| k(slot)).or_else(|| k(provider)).or_else(|| if provider == "aistudio" { k("gemini") } else { None }).unwrap_or_default()
    }
    pub fn get(&self, provider: &str, slot: &str) -> String {
        self.get_with(provider, slot, |v| std::env::var(v).ok())
    }
    pub fn has(&self, provider: &str, slot: &str) -> bool {
        provider == "ollama" || !self.get(provider, slot).is_empty()
    }

    /// Everything there is: the file first, the keyring's entries over it
    /// (blocking: run it on a thread)
    pub fn load(slots: &[String]) -> Keys {
        let mut k = Keys::default();
        if let Ok(text) = std::fs::read_to_string(file_path()) {
            if let Ok(serde_json::Value::Object(o)) = serde_json::from_str(&text) {
                for (s, v) in o {
                    if let Some(v) = v.as_str() {
                        k.file.insert(s.clone(), v.to_string());
                        k.keys.insert(s, v.to_string());
                    }
                }
            }
        }
        for slot in slots {
            for app in ["tessera", "irice"] {
                let out = Command::new("secret-tool").args(["lookup", "application", app, "provider", slot]).stdin(Stdio::null()).stderr(Stdio::null()).output();
                let Ok(out) = out else { return k }; // (no secret-tool at all)
                let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if out.status.success() && !v.is_empty() {
                    k.keys.insert(slot.clone(), v);
                    break;
                }
            }
        }
        k
    }

    /// Keep one (the composer's key field, /key): in memory now, in the
    /// keyring on a thread, the private file if the keyring can't
    pub fn set(&mut self, slot: &str, key: &str, label: &str) {
        let key = key.trim().to_string();
        self.keys.insert(slot.to_string(), key.clone());
        self.edited.insert(slot.to_string());
        let (slot, label) = (slot.to_string(), label.to_string());
        let mut file = self.file.clone();
        std::thread::spawn(move || {
            let stored = Command::new("secret-tool")
                .args(["store", &format!("--label=Tessera · {label}"), "application", "tessera", "provider", &slot])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| {
                    c.stdin.take().unwrap().write_all(key.as_bytes())?;
                    c.wait()
                })
                .is_ok_and(|s| s.success());
            if !stored {
                file.insert(slot, key);
                let _ = super::chats::write_private(&file_path(), &serde_json::to_string(&file).unwrap());
            }
        });
    }

    /// A late reading (from load) over what's here, keeping what this process set
    pub fn merge(&mut self, other: Keys) {
        for (s, v) in other.keys {
            if !self.edited.contains(&s) {
                self.keys.insert(s, v);
            }
        }
        self.file = other.file;
    }

    #[cfg(test)]
    pub fn with(pairs: &[(&str, &str)]) -> Keys {
        Keys { keys: pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_order_as_the_shell() {
        let k = Keys::with(&[("custom|m", "slot-key"), ("openai", "provider-key"), ("gemini", "old-gemini")]);
        let none = |_: &str| None;
        assert_eq!(k.get_with("custom", "custom|m", none), "slot-key");
        assert_eq!(k.get_with("openai", "openai|x", none), "provider-key");
        assert_eq!(k.get_with("aistudio", "aistudio|x", none), "old-gemini");
        assert_eq!(k.get_with("openai", "openai|x", |v| (v == "OPENAI_API_KEY").then(|| "env".into())), "env");
        assert_eq!(k.get_with("vertex", "v", |v| (v == "GOOGLE_API_KEY").then(|| "g".into())), "g");
        assert_eq!(k.get_with("anthropic", "a", none), "");
        assert!(k.has("ollama", "anything"));
    }

    #[test]
    fn debug_hides_keys() {
        assert!(!format!("{:?}", Keys::with(&[("openai", "sk-secret")])).contains("sk-secret"));
    }
}
