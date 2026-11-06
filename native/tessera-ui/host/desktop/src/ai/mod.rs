//! The AI service (shell/services/Ai.qml and its helpers), standalone.
//!
//! The chat itself, the models, the credentials, the system prompt, the SSE
//! parser and the chat history files are already Rust, in
//! tessera_core::services::ai (chats.rs = AiChats.js, keys.rs = AiKeys.qml,
//! models.rs = AiModels.js, prompt.rs = AiPrompt.js, sse.rs = SseParser.qml):
//! the sidebar surface owns the open chat and writes the history. What
//! Ai.qml still did for the surfaces, and is answered here:
//!
//!   ai.quickAsk [question]  Spotlight's one-shot answer; progress goes back as
//!                           {"to":"spotlight","quick":{busy,answer,error}}
//!                           (what Search.qml told on every change of Ai.quick)
//!   ai.stopQuick            stop that answer (the answer so far stays)
//!   ai.continueQuick        move the exchange into the sidebar chat
//!   ai.reloadHistory        the sidebar wrote the chat files; in the QML shell
//!                           its own chat then reread them. Standalone there is
//!                           no second chat, so there is nothing to reread.

use serde_json::{json, Value};
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tessera_core::host::{Host, Service};
use tessera_core::services::ai::keys::{self, Keys};
use tessera_core::services::ai::models::{self, Model, Opts, Settings};
use tessera_core::services::ai::{prompt, stream, Event};

/// What Search.qml put in the extras of a query: whether a model is set up and
/// its name ({"ai": {"configured", "name"}}); for the search service
pub fn meta(cfg: &Value) -> Value {
    let m = selected(cfg);
    json!({"configured": m.configured(), "name": m.name})
}

/// The chosen model (`ai.model`), else the first, else the empty one (Ai.qml's `model`)
pub fn selected(cfg: &Value) -> Model {
    let list = models::read(&cfg["ai"]["models"]);
    let want = cfg["ai"]["model"].as_str().unwrap_or("");
    list.iter().find(|m| m.key == want).or(list.first()).cloned().unwrap_or_else(Model::none)
}

/// HTTP errors: what happened and how to fix it (Ai.qml's errorText)
pub fn error_text(m: &Model, status: u16, body: &str) -> String {
    let j: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = j["error"]["message"].as_str().or_else(|| j[0]["error"]["message"].as_str()).unwrap_or("");
    let p = models::provider_name(&m.provider);
    match status {
        0 => format!("Couldn't reach {p}. Check your internet connection{}.", if m.provider == "ollama" { " or that Ollama is running" } else { "" }),
        401 | 403 => format!("Your {p} credential was rejected. Check it in Settings → AI{}.", if m.auth == "token" { "; OAuth access tokens expire and must be refreshed" } else { "" }),
        402 => format!("There's a billing problem on your {p} account."),
        404 => format!("The model “{}” isn't available. Check its ID, provider and connection in Settings → AI.", m.id),
        429 => format!("Rate limited by {p}. Wait a moment and try again."),
        s if s == 529 || s >= 500 => format!("{p} is having trouble right now. Try again in a moment."),
        _ if !msg.is_empty() => msg.to_string(),
        s => format!("Request failed ({s})."),
    }
}

/// Ai.quick: the question, the answer so far, whether it runs, what went wrong
#[derive(Clone, Debug, Default, PartialEq)]
struct Quick {
    question: String,
    answer: String,
    busy: bool,
    error: String,
}

/// From the request's thread
enum Inner {
    Ev(Event),
    /// it never started: the person is told why
    Failed(u64, String),
}

#[derive(Default)]
pub struct Ai {
    quick: Quick,
    told: Option<(bool, String, String)>,
    /// the running answer's number (0: none); the thread stops when this changes
    cancel: Arc<AtomicU64>,
    gen: u64,
    model: Option<Model>,
    /// ai.model as last read: a change stops the quick answer (Ai.setModel)
    last_key: Option<String>,
    /// the model removed last, for Undo: (its record, was it the chosen one)
    removed: Option<(Value, bool)>,
}

/// The records of `ai.models` as written: JSON strings, as Ai.qml saves them
fn record(m: &Model) -> Value {
    let mut r = json!({"key": m.key, "id": m.id, "name": m.name, "provider": m.provider, "endpoint": m.endpoint,
        "project": m.project, "location": m.location, "auth": m.auth, "thinking": m.thinking, "vision": m.vision});
    if m.provider == "anthropic" {
        r["maxTokens"] = json!(m.max_tokens);
    }
    r
}

fn write_models(list: &[Model], extra: &[Value]) -> Vec<Value> {
    list.iter().map(record).chain(extra.iter().cloned()).map(|r| json!(r.to_string())).collect()
}

/// A saved credential that can't come back (Ai.qml's AiKeys.clear): the keyring entry and the file's
fn clear_credential(slot: String) {
    std::thread::spawn(move || {
        for app in ["tessera", "irice"] {
            let _ = std::process::Command::new("secret-tool")
                .args(["clear", "application", app, "provider", &slot])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        let file = tessera_core::services::ai::chats::state_dir().join("ai-keys.json");
        if let Some(Value::Object(mut o)) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()) {
            if o.remove(&slot).is_some() {
                let _ = tessera_core::services::ai::chats::write_private(&file, &Value::Object(o).to_string());
            }
        }
    });
}

/// Settings -> AI -> Delete all chats (Ai.deleteAllChats): the files and an empty index
fn delete_all_chats() {
    let dir = tessera_core::services::ai::chats::Store::default_dir();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let idx = tessera_core::services::ai::chats::Index::default();
    let _ = tessera_core::services::ai::chats::write_private(&dir.join("index.json"), &tessera_core::services::ai::chats::index_text(&idx));
}

pub fn services() -> Vec<Box<dyn Service>> {
    vec![Box::new(Ai::default())]
}

impl Ai {
    /// Search.qml told spotlight on every change of Ai.quick
    fn tell(&mut self, h: &mut Host) {
        let now = (self.quick.busy, self.quick.answer.clone(), self.quick.error.clone());
        if self.told.as_ref() == Some(&now) {
            return;
        }
        self.told = Some(now);
        h.tell("spotlight", json!({"quick": {"busy": self.quick.busy, "answer": self.quick.answer, "error": self.quick.error}}));
    }

    fn stop_quick(&mut self, h: &mut Host) {
        self.cancel.store(0, Ordering::SeqCst);
        self.quick.busy = false;
        self.tell(h);
    }

    fn ask(&mut self, h: &mut Host, question: String) {
        self.stop_quick(h);
        self.quick = Quick { question: question.clone(), ..Default::default() };
        let cfg = h.cfg().clone();
        let m = selected(&cfg);
        self.model = Some(m.clone());
        self.quick.busy = true;
        self.tell(h);
        self.gen += 1;
        let gen = self.gen;
        self.cancel.store(gen, Ordering::SeqCst);
        let cancel = self.cancel.clone();
        let cached = h.st.shell.get::<Keys>().cloned();
        let send = h.sender("Ai");
        // (on a thread: a credential not found yet is looked up in the keyring)
        std::thread::spawn(move || {
            let fail = |e: String| send(Box::new(Inner::Failed(gen, e)));
            if !m.configured() {
                return fail("Add a model and provider in Settings → AI first.".into());
            }
            let cred = models::credential_provider(&m);
            let mut ks = cached.unwrap_or_default();
            if m.auth != "none" && !ks.has(&cred, &m.key) {
                let mut slots: Vec<String> = keys::SLOTS.iter().map(|s| s.to_string()).collect();
                slots.push(m.key.clone());
                ks.merge(Keys::load(&slots));
            }
            if m.auth != "none" && !ks.has(&cred, &m.key) {
                return fail(format!("Add your {} {} in Settings → AI first.", models::provider_name(&m.provider), models::credential_label(&m).to_lowercase()));
            }
            let opts = Opts { system: prompt::system(&cfg["ai"], true), max_tokens: Some(2048), thinking: Some(false), effort: Some("low".into()) };
            let turns = [json!({"role": "user", "text": question})];
            let req = match models::build_request(&m, &turns, &opts, &Settings::from(&cfg["ai"]), &ks.get(&cred, &m.key)) {
                Ok(r) => r,
                Err(e) => return fail(e),
            };
            let s = send.clone();
            stream(req, models::protocol(&m).to_string(), gen, cancel, move |e| s(Box::new(Inner::Ev(e))));
        });
    }

    /// Ai.setModel: another chosen model stops a quick answer
    fn set_model(&mut self, h: &mut Host, id: &str) {
        let list = models::read(&h.cfg()["ai"]["models"]);
        let Some(m) = list.iter().find(|m| m.key == id || m.id == id) else { return };
        if m.key == selected(h.cfg()).key {
            return;
        }
        self.stop_quick(h);
        crate::screen::util::cfg_set("ai.model", json!(m.key));
    }

    /// Ai.saveModel + saveCredential: `raw` is the page's draft, `old` the key it edits; the
    /// page checked it already, this checks again and says nothing when it fails (no error path back)
    fn accounts_save(&mut self, h: &mut Host, raw: &Value, old: &str, credential: &str) {
        let raw = match raw {
            Value::String(t) => serde_json::from_str(t).unwrap_or(Value::Null),
            v => v.clone(),
        };
        let m = models::normalize(&raw);
        if !models::validate(&m).is_empty() {
            eprintln!("tessera-ui: ai.accountsSave: {}", models::validate(&m));
            return;
        }
        let list = models::read(&h.cfg()["ai"]["models"]);
        if list.iter().any(|x| x.key == m.key && x.key != old) {
            return;
        }
        let prev = list.iter().find(|x| x.key == old).cloned();
        // renaming a model ID on the same connection keeps its saved key
        let same = |a: &Model, b: &Model| a.provider == b.provider && a.endpoint == b.endpoint && a.auth == b.auth && a.project == b.project && a.location == b.location;
        let moved = prev.as_ref().filter(|p| p.key != m.key && same(p, &m)).map(|p| p.key.clone());
        self.stop_quick(h);
        let kept: Vec<Model> = list.into_iter().filter(|x| x.key != old).chain([m.clone()]).collect();
        crate::screen::util::cfg_set("ai.models", json!(write_models(&kept, &[])));
        crate::screen::util::cfg_set("ai.modelsMigrated", json!(true));
        crate::screen::util::cfg_set("ai.model", json!(m.key));
        let secret = credential.trim().to_string();
        if !secret.is_empty() {
            let mut ks = Keys::default();
            ks.set(&m.key, &secret, &format!("{} · {}", m.name, models::credential_label(&m)));
        } else if let Some(old_key) = moved {
            // (the old entry's key follows the model: read it, keep it under the new slot, drop the old)
            let (slot, label) = (m.key.clone(), format!("{} · {}", m.name, models::credential_label(&m)));
            std::thread::spawn(move || {
                let mut ks = Keys::load(&[old_key.clone()]);
                let v = ks.get("", &old_key);
                if !v.is_empty() {
                    ks.set(&slot, &v, &label);
                }
            });
            clear_credential(old_key);
        }
    }

    /// Ai.removeModel: kept for Undo until forget_removed
    fn remove_model(&mut self, h: &mut Host, key: &str) {
        let list = models::read(&h.cfg()["ai"]["models"]);
        let Some(gone) = list.iter().find(|m| m.key == key).cloned() else { return };
        if selected(h.cfg()).key == key {
            self.stop_quick(h);
        }
        let was = h.cfg()["ai"]["model"].as_str() == Some(key);
        let rest: Vec<Model> = list.into_iter().filter(|m| m.key != key).collect();
        crate::screen::util::cfg_set("ai.models", json!(write_models(&rest, &[])));
        if was {
            crate::screen::util::cfg_set("ai.model", json!(rest.first().map(|m| m.key.clone()).unwrap_or_default()));
        }
        self.removed = Some((record(&gone), was));
    }

    /// Ai.restoreModel (Undo)
    fn restore_removed(&mut self) {
        let Some((rec, was)) = self.removed.take() else { return };
        let cfg: Value = std::fs::read_to_string(tessera_core::theme::config_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let list = models::read(&cfg["ai"]["models"]);
        let key = rec["key"].as_str().unwrap_or("").to_string();
        if list.iter().any(|m| m.key == key) {
            return;
        }
        crate::screen::util::cfg_set("ai.models", json!(write_models(&list, &[rec])));
        if was {
            crate::screen::util::cfg_set("ai.model", json!(key));
        }
    }

    /// Ai.forgetCredential: the Undo time is over, the credential goes too
    fn forget_removed(&mut self) {
        if let Some((rec, _)) = self.removed.take() {
            clear_credential(rec["key"].as_str().unwrap_or("").to_string());
        }
    }

    /// Move the quick exchange into the sidebar chat (the open one)
    fn continue_quick(&mut self, h: &mut Host) {
        if self.quick.answer.is_empty() {
            return;
        }
        h.command("continue", "sidebar", None, json!({"question": self.quick.question, "answer": self.quick.answer}));
        h.ask("sidebar.tab", json!([0]));
        h.ask("panels.show", json!(["sidebar", ""]));
    }
}

impl Service for Ai {
    fn name(&self) -> &'static str {
        "Ai"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["ai"]
    }
    fn start(&mut self, h: &mut Host) {
        // (the credentials, read once; quickAsk looks again if one is missing)
        h.st.shell.want("ai");
        self.last_key = Some(selected(h.cfg()).key);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "ai.quickAsk" => self.ask(h, args[0].as_str().unwrap_or("").to_string()),
            "ai.stopQuick" => self.stop_quick(h),
            "ai.continueQuick" => self.continue_quick(h),
            "ai.reloadHistory" => {}
            // Settings -> AI (host-system's accounts bridge asks these; Ai.qml's setModel,
            // saveModel, removeModel/restoreModel/forgetCredential, deleteAllChats)
            "ai.setModel" => self.set_model(h, args[0].as_str().unwrap_or("")),
            "ai.accountsSave" => self.accounts_save(h, &args[0], args[1].as_str().unwrap_or(""), args[2].as_str().unwrap_or("")),
            "ai.removeModel" => self.remove_model(h, args[0].as_str().unwrap_or("")),
            "ai.restoreRemoved" => self.restore_removed(),
            "ai.forgetRemoved" => self.forget_removed(),
            "ai.deleteAllChats" => delete_all_chats(),
            _ => eprintln!("tessera-ui: host: no call {call}"),
        }
    }
    fn config_changed(&mut self, h: &mut Host) {
        // another model (or one edited/removed): the quick answer stops
        let key = selected(h.cfg()).key;
        if self.last_key.as_ref() != Some(&key) {
            self.last_key = Some(key);
            if self.quick.busy {
                self.stop_quick(h);
            }
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(inner) = msg.downcast::<Inner>() else { return };
        match *inner {
            Inner::Failed(gen, e) => {
                if gen == self.gen && self.quick.busy {
                    self.quick.busy = false;
                    self.quick.error = e;
                    self.tell(h);
                }
            }
            Inner::Ev(Event::Update { gen, text, .. }) => {
                if gen == self.gen && self.quick.busy {
                    self.quick.answer = text;
                    self.tell(h);
                }
            }
            Inner::Ev(Event::Done { gen, status, parser, body }) => {
                if gen != self.gen || !self.quick.busy {
                    return;
                }
                self.quick.busy = false;
                self.quick.answer = parser.text.clone();
                let m = self.model.clone().unwrap_or_else(Model::none);
                if status != 200 {
                    self.quick.error = error_text(&m, status, &body);
                } else if parser.stop_reason == "refusal" {
                    self.quick.error = "This question can't be answered here. Try rephrasing it.".into();
                } else if !parser.error.is_empty() {
                    self.quick.error = parser.error.clone();
                }
                self.tell(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_model_is_not_configured() {
        assert_eq!(meta(&json!({})), json!({"configured": false, "name": "Add a model"}));
        assert!(!selected(&json!({"ai": {"models": []}})).configured());
    }

    #[test]
    fn errors_say_how_to_fix() {
        let m = Model { provider: "ollama".into(), id: "x".into(), ..Default::default() };
        assert!(error_text(&m, 0, "").contains("Ollama is running"));
        assert!(error_text(&m, 404, "").contains("“x”"));
        assert_eq!(error_text(&m, 418, r#"{"error":{"message":"teapot"}}"#), "teapot");
        assert_eq!(error_text(&m, 418, ""), "Request failed (418).");
    }
}
