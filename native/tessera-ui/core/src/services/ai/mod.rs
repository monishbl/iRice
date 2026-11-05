//! The AI service: shell/services/Ai.qml's chat in Rust, for the sidebar.
//! The person's models from config.json (`ai.models`, `ai.model`), their
//! credentials where AiKeys.qml keeps them (keys.rs), Tessera's system prompt
//! (prompt.rs), each provider's request (models.rs), the streamed answer
//! (sse.rs over HTTP, below) and the chat history in the shell's own files
//! (chats.rs), so a chat goes on whichever shell draws the sidebar.
//!
//! `Chat` is the conversation and its history, with no UI: the sidebar calls
//! it and draws its rows. A request runs on a thread of its own (`stream`)
//! and sends `Event`s back to the part that started it.
//!
//! HTTP: ureq (blocking, small, no async runtime) with rustls, checking
//! certificates against the system's trust store (rustls-platform-verifier),
//! as Qt's network stack does for the QML shell.

pub mod chats;
pub mod keys;
pub mod models;
pub mod prompt;
pub mod sse;

use chats::{Entry, Index, Row, Session, Store};
use models::{Model, Opts, Request};
use serde_json::{json, Value};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MAX_IMAGES: usize = 4;
pub const CIRCLE_QUESTION: &str = "What is this? Identify it and tell me the most useful things about it.";

/// A picture going with the next message (`path`: a small JPEG copy in the
/// runtime folder, for the previews; `data`: it in base64)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picture {
    pub path: String,
    pub data: String,
    pub media_type: String,
    /// circled (circle to search): it can be searched on the web
    pub lens: bool,
}

/// What a request's thread says, for the part (Msg::Part)
#[derive(Debug)]
pub enum Event {
    /// The answer so far (sent at most every 50 ms, as Ai.qml's flushTimer)
    Update { gen: u64, text: String, thinking: String },
    /// It ended: HTTP status (0: never reached), the parser, the body when it failed
    Done { gen: u64, status: u16, parser: Box<sse::Parser>, body: String },
}

/// The deleted chat Undo brings back
#[derive(Clone, Debug, PartialEq)]
pub struct PendingDelete {
    pub entry: Entry,
    pub was_current: bool,
}

pub struct Chat {
    pub rows: Vec<Row>,
    /// API-facing turns: { role, text, images?, blocks? }
    pub history: Vec<Value>,
    pub attachments: Vec<Picture>,
    pub models: Vec<Model>,
    pub model: Model,
    /// config.json's `ai`
    pub ai: Value,
    pub keys: keys::Keys,
    pub store: Option<Store>,
    pub sessions: Vec<Entry>,
    pub current_id: String,
    pub trash: Vec<String>,
    pub pending_delete: Option<PendingDelete>,
    pub notice: String,
    changed: bool,
    /// the running request's number (0: none); its thread stops when this changes
    pub gen: u64,
    cancel: Arc<AtomicU64>,
    next_gen: u64,
    streamed: (String, String),
    /// the index was written (the other shell is told to read it again)
    pub wrote: bool,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            rows: Vec::new(),
            history: Vec::new(),
            attachments: Vec::new(),
            models: Vec::new(),
            model: Model::none(),
            ai: Value::Null,
            keys: keys::Keys::default(),
            store: None,
            sessions: Vec::new(),
            current_id: String::new(),
            trash: Vec::new(),
            pending_delete: None,
            notice: String::new(),
            changed: false,
            gen: 0,
            cancel: Arc::new(AtomicU64::new(0)),
            next_gen: 0,
            streamed: Default::default(),
            wrote: false,
        }
    }
}

impl Chat {
    pub fn new(store: Option<Store>) -> Self {
        Chat { store, ..Default::default() }
    }

    // ── Settings ────────────────────────────────────────────────────────

    /// config.json changed (or first read)
    pub fn configure(&mut self, cfg: &Value) {
        self.ai = cfg["ai"].clone();
        self.models = models::read(&self.ai["models"]);
        let want = self.ai["model"].as_str().unwrap_or("");
        self.model = self.models.iter().find(|m| m.key == want).or(self.models.first()).cloned().unwrap_or_else(Model::none);
    }
    pub fn keep_history(&self) -> bool {
        self.ai["keepHistory"].as_bool().unwrap_or(true)
    }
    pub fn configured(&self) -> bool {
        self.model.configured()
    }
    pub fn provider_name(&self) -> String {
        models::provider_name(&self.model.provider)
    }
    pub fn credential_label(&self) -> &'static str {
        models::credential_label(&self.model)
    }
    pub fn has_credential(&self, m: &Model) -> bool {
        m.auth == "none" || self.keys.has(&models::credential_provider(m), &m.key)
    }
    pub fn needs_key(&self) -> bool {
        self.configured() && !self.has_credential(&self.model)
    }
    pub fn busy(&self) -> bool {
        self.gen != 0
    }
    pub fn vision_models(&self) -> Vec<&Model> {
        self.models.iter().filter(|m| m.vision).collect()
    }
    /// The model that reads pictures to suggest: one with a credential first
    pub fn can_see(&self) -> Option<&Model> {
        let v = self.vision_models();
        v.iter().find(|m| self.has_credential(m)).or(v.first()).copied()
    }
    /// Choose a model (by key or ID): the key to save as `ai.model`, if it changed
    pub fn set_model(&mut self, id: &str) -> Option<String> {
        let m = self.models.iter().find(|m| m.key == id || m.id == id)?.clone();
        if m.key == self.model.key {
            return None;
        }
        self.stop();
        self.model = m;
        Some(self.model.key.clone())
    }

    /// Keep a credential for the chosen model
    pub fn save_credential(&mut self, key: &str) {
        let m = self.model.clone();
        let label = format!("{} · {}", m.name, models::credential_label(&m));
        self.keys.set(&m.key, key, &label);
    }

    // ── History ─────────────────────────────────────────────────────────

    /// Read the index once at start: the chat that was open comes back
    pub fn start_history(&mut self) {
        let Some(store) = &self.store else { return };
        let idx = store.read_index();
        self.sessions = idx.chats.clone();
        if self.keep_history() && !idx.current.is_empty() && self.rows.is_empty() {
            if let Some(s) = store.read_session(&idx.current) {
                self.fill(&s);
                self.current_id = idx.current;
            }
        }
        self.prune_old();
    }

    /// The index again, as the other shell may have changed it
    pub fn reload_sessions(&mut self) {
        if let Some(store) = &self.store {
            self.sessions = store.read_index().chats;
        }
    }

    fn fill(&mut self, s: &Value) {
        self.rows = s["rows"].as_array().into_iter().flatten().map(Row::from_value).collect();
        self.history = s["history"].as_array().cloned().unwrap_or_default();
    }

    fn write_index(&mut self) {
        let Some(store) = &self.store else { return };
        let current = if self.keep_history() { self.current_id.clone() } else { String::new() };
        let trash = self.trash.clone();
        let idx = store.update_index(|i| {
            i.current = current;
            i.trash = trash;
        });
        self.sessions = idx.chats;
        self.wrote = true;
    }

    /// Change the index on disk (the other shell's entries kept)
    fn edit_index(&mut self, f: impl FnOnce(&mut Index)) {
        let Some(store) = &self.store else { return };
        let current = if self.keep_history() { self.current_id.clone() } else { String::new() };
        let trash = self.trash.clone();
        let idx = store.update_index(|i| {
            f(i);
            i.current = current;
            i.trash = trash;
        });
        self.sessions = idx.chats;
        self.wrote = true;
    }

    /// Write the open chat when it has news (or `force`: its title or pin changed)
    pub fn save(&mut self, force: bool) {
        if self.store.is_none() || !self.keep_history() || (!self.changed && !force) {
            return;
        }
        let rows: Vec<Row> = self.rows.iter().filter(|r| r.status != "streaming").cloned().collect();
        if !chats::worth_keeping(&rows) {
            return;
        }
        let now = chats::now_ms();
        if self.current_id.is_empty() {
            self.current_id = chats::new_id(now, rand01());
        }
        let old = self.sessions.iter().find(|c| c.id == self.current_id).cloned();
        let s = Session {
            id: self.current_id.clone(),
            title: old.as_ref().map(|o| o.title.clone()).unwrap_or_else(|| chats::title_from(&rows.iter().find(|r| r.role == "user").unwrap().text)),
            created: old.as_ref().map_or(now, |o| o.created),
            updated: if self.changed || old.is_none() { now } else { old.as_ref().unwrap().updated },
            model: self.model.key.clone(),
            model_name: if self.model.configured() { self.model.name.clone() } else { old.as_ref().map(|o| o.model_name.clone()).unwrap_or_default() },
            pinned: old.as_ref().is_some_and(|o| o.pinned),
            rows,
            history: chats::strip_pictures(&self.history),
        };
        self.store.as_ref().unwrap().write_session(&s);
        let entry = chats::entry_of(&s);
        self.changed = false;
        self.edit_index(|i| i.chats = chats::upsert(&i.chats, entry));
    }

    /// The pencil, /new and /clear: a new chat; the old one stays in history
    pub fn new_chat(&mut self) {
        self.stop();
        self.save(false);
        self.rows.clear();
        self.history.clear();
        self.current_id.clear();
        self.changed = false;
        self.write_index();
    }

    /// Open a past chat: an answer still coming is stopped (and kept) first.
    /// Ok(model key to save, when its model comes back too); Err: it's gone
    pub fn open_chat(&mut self, id: &str) -> Result<Option<String>, ()> {
        if id == self.current_id {
            return Ok(None);
        }
        let Some(s) = self.store.as_ref().and_then(|st| st.read_session(id)) else {
            let gone = id.to_string();
            self.edit_index(|i| i.chats = chats::without(&i.chats, &gone));
            self.say("Couldn't open that chat: its file is missing or damaged, so it's been taken off the list.");
            return Err(());
        };
        self.stop();
        self.save(false);
        self.fill(&s);
        self.current_id = id.to_string();
        self.changed = false;
        let model = s["model"].as_str().filter(|k| self.models.iter().any(|m| m.key == *k)).map(String::from);
        let chosen = model.and_then(|k| self.set_model(&k));
        self.write_index();
        Ok(chosen)
    }

    pub fn rename_chat(&mut self, id: &str, title: &str) {
        let t = chats::clean_title(title);
        if !t.is_empty() {
            self.update_chat(id, json!({"title": t}));
        }
    }
    pub fn set_pinned(&mut self, id: &str, pinned: bool) {
        self.update_chat(id, json!({"pinned": pinned}));
    }
    fn update_chat(&mut self, id: &str, fields: Value) {
        if !self.sessions.iter().any(|c| c.id == id) {
            return;
        }
        let apply = |e: &mut Entry| {
            if let Some(t) = fields["title"].as_str() {
                e.title = t.to_string();
            }
            if let Some(p) = fields["pinned"].as_bool() {
                e.pinned = p;
            }
        };
        for e in self.sessions.iter_mut().filter(|e| e.id == id) {
            apply(e);
        }
        if id == self.current_id {
            self.save(true);
        } else if let Some(st) = &self.store {
            st.patch(id, &fields);
        }
        let id = id.to_string();
        self.edit_index(|i| i.chats.iter_mut().filter(|e| e.id == id).for_each(apply));
    }

    /// Delete at once; Undo brings it back until finish_delete (the part's
    /// 8 s timer), and only then is the file removed. The open chat clears.
    pub fn delete_chat(&mut self, id: &str) {
        self.finish_delete();
        let was_current = id == self.current_id;
        if was_current {
            self.stop();
            self.save(false);
        }
        let Some(entry) = self.sessions.iter().find(|c| c.id == id).cloned() else { return };
        if was_current {
            self.discard_current();
        }
        self.trash = vec![id.to_string()];
        self.pending_delete = Some(PendingDelete { entry, was_current });
        self.notice.clear();
        let id = id.to_string();
        self.edit_index(|i| i.chats = chats::without(&i.chats, &id));
    }
    /// Ok(model key to save) when the deleted chat was open and comes back
    pub fn undo_delete(&mut self) -> Option<String> {
        let p = self.pending_delete.take()?;
        self.trash.clear();
        let e = p.entry.clone();
        self.edit_index(|i| i.chats = chats::upsert(&i.chats, e));
        if p.was_current && self.rows.is_empty() && !self.busy() {
            return self.open_chat(&p.entry.id).ok().flatten();
        }
        None
    }
    pub fn finish_delete(&mut self) {
        let Some(p) = self.pending_delete.take() else { return };
        if let Some(st) = &self.store {
            st.remove(&[p.entry.id]);
        }
        self.trash.clear();
        self.write_index();
    }
    fn discard_current(&mut self) {
        self.cancel_request();
        self.rows.clear();
        self.history.clear();
        self.current_id.clear();
        self.changed = false;
    }

    /// Settings' "delete chats older than": never pinned or open ones
    pub fn prune_old(&mut self) {
        let days = self.ai["historyDays"].as_i64().unwrap_or(0);
        let drop = chats::prune(&self.sessions, days, chats::now_ms(), &[self.current_id.as_str()]);
        if drop.is_empty() {
            return;
        }
        if let Some(st) = &self.store {
            st.remove(&drop);
        }
        self.edit_index(|i| i.chats.retain(|c| !drop.contains(&c.id)));
    }

    pub fn say(&mut self, text: &str) {
        self.notice = text.to_string();
    }

    // ── The conversation ────────────────────────────────────────────────

    pub fn note(&mut self, text: &str) {
        self.rows.push(Row::new("note", text, "done"));
    }

    /// attach(picture): one more, while there's room
    pub fn attach(&mut self, p: Picture) {
        if self.attachments.len() < MAX_IMAGES {
            self.attachments.push(p);
        } else {
            self.note_full();
        }
    }
    pub fn note_full(&mut self) {
        self.note(&format!("A message can carry {MAX_IMAGES} pictures. Remove one to add another."));
    }
    pub fn detach(&mut self, i: usize) {
        if i < self.attachments.len() {
            self.attachments.remove(i);
        }
    }

    /// The model can't read pictures: say which of theirs can
    pub fn cant_see(&self) -> String {
        let can: Vec<&str> = self.vision_models().iter().map(|m| m.name.as_str()).collect();
        let rest = if can.is_empty() {
            "Add a model that can (Claude, Gemini or another vision model) in Settings → AI, or remove the pictures.".to_string()
        } else {
            let list = if can.len() == 1 { can[0].to_string() } else { format!("{} or {}", can[..can.len() - 1].join(", "), can[can.len() - 1]) };
            format!("Switch to {list} in the model menu (or type /model {}), then send again.", can[0])
        };
        format!("{} can't read pictures. {rest}", self.model.name)
    }

    /// Circle to search: the circled picture, asked about at once (a model
    /// that can't read pictures keeps it attached for when they switch)
    pub fn search_picture(&mut self, p: Picture) -> Submitted {
        if self.busy() {
            self.stop();
        }
        let path = p.path.clone();
        self.attach(Picture { lens: true, ..p });
        if self.attachments.iter().any(|a| a.path == path) {
            self.submit("")
        } else {
            Submitted::Nothing
        }
    }

    /// The composer's Enter, and the slash commands: /new (or /clear),
    /// /model <id>, /key <key>
    pub fn submit(&mut self, text: &str) -> Submitted {
        let n = self.attachments.len();
        let t = match text.trim() {
            "" if self.attachments.iter().any(|a| a.lens) => CIRCLE_QUESTION.to_string(),
            "" if n == 1 => "What's in this picture?".to_string(),
            "" if n > 1 => "What's in these pictures?".to_string(),
            t => t.to_string(),
        };
        if t.is_empty() || self.busy() {
            return Submitted::Nothing;
        }
        if t == "/new" || t == "/clear" {
            self.new_chat();
            return Submitted::Nothing;
        }
        if let Some(id) = t.strip_prefix("/model ") {
            let id = id.trim();
            let found = self.models.iter().find(|x| x.key == id || x.id == id || x.name.to_lowercase() == id.to_lowercase()).cloned();
            return match found {
                Some(m) => {
                    let key = self.set_model(&m.key);
                    self.note(&format!("Switched to {}.", m.name));
                    key.map_or(Submitted::Nothing, Submitted::Model)
                }
                None => {
                    self.note(&format!("No model called “{id}”. Add it in Settings → AI or pick one from the model menu."));
                    Submitted::Nothing
                }
            };
        }
        if !self.configured() {
            self.note("Add a model and provider in Settings → AI first.");
            return Submitted::Nothing;
        }
        if let Some(key) = t.strip_prefix("/key ") {
            self.save_credential(key);
            let p = self.provider_name();
            self.note(&format!("{p} credential saved."));
            return Submitted::Nothing;
        }
        if self.needs_key() {
            let note = format!("Add your {} {} in Settings → AI, or type /key followed by the credential.", self.provider_name(), self.credential_label().to_lowercase());
            self.note(&note);
            return Submitted::Nothing;
        }
        if !self.attachments.is_empty() && !self.model.vision {
            let note = self.cant_see();
            self.note(&note);
            return Submitted::Nothing;
        }
        self.send(&t)
    }

    fn send(&mut self, text: &str) -> Submitted {
        let imgs = std::mem::take(&mut self.attachments);
        let mut turn = json!({"role": "user", "text": text});
        if !imgs.is_empty() {
            turn["images"] = Value::Array(imgs.iter().map(|i| json!({"data": i.data, "mediaType": i.media_type})).collect());
        }
        self.history.push(turn);
        self.changed = true;
        let mut user = Row::new("user", text, "done");
        user.image = imgs.iter().map(|i| i.path.as_str()).collect::<Vec<_>>().join("\n");
        self.rows.push(user);
        let mut reply = Row::new("assistant", "", "streaming");
        reply.model = self.model.name.clone();
        reply.search = imgs.iter().find(|i| i.lens).map(|i| i.path.clone()).unwrap_or_default();
        self.rows.push(reply);
        let opts = Opts { system: prompt::system(&self.ai, false), ..Default::default() };
        let credential = self.keys.get(&models::credential_provider(&self.model), &self.model.key);
        match models::build_request(&self.model, &self.history, &opts, &models::Settings::from(&self.ai), &credential) {
            Ok(req) => {
                self.next_gen += 1;
                self.gen = self.next_gen;
                self.cancel.store(self.gen, Ordering::SeqCst);
                self.streamed = Default::default();
                Submitted::Request(req, models::protocol(&self.model).to_string(), self.gen, self.cancel.clone())
            }
            Err(e) => {
                self.fail_with(&e);
                Submitted::Nothing
            }
        }
    }

    /// The answer so far (Event::Update); true when it's this request's
    pub fn update(&mut self, gen: u64, text: String, thinking: String) -> bool {
        if gen != self.gen || gen == 0 {
            return false;
        }
        if let Some(r) = self.rows.last_mut().filter(|r| r.role == "assistant") {
            r.text = text.clone();
            r.thinking = thinking.clone();
        }
        self.streamed = (text, thinking);
        true
    }

    /// The request ended (Event::Done)
    pub fn done(&mut self, gen: u64, status: u16, parser: &sse::Parser, body: &str) -> bool {
        if gen != self.gen || gen == 0 {
            return false;
        }
        self.gen = 0;
        self.streamed = (parser.text.clone(), parser.thinking.clone());
        if status == 200 {
            let s = if parser.stop_reason == "refusal" {
                "refused"
            } else if !parser.error.is_empty() {
                "error"
            } else {
                "done"
            };
            self.finish(s, parser);
        } else {
            let t = self.error_text(status, body);
            self.fail_with(&t);
        }
        true
    }

    fn cancel_request(&mut self) {
        self.gen = 0;
        self.cancel.store(0, Ordering::SeqCst);
    }

    /// The stop button: what came so far is kept
    pub fn stop(&mut self) {
        if self.busy() {
            self.cancel_request();
            let mut p = sse::Parser::default();
            (p.text, p.thinking) = self.streamed.clone();
            self.finish("stopped", &p);
        }
    }

    fn finish(&mut self, status: &str, p: &sse::Parser) {
        let Some(i) = self.rows.len().checked_sub(1) else { return };
        self.rows[i].text = p.text.clone();
        self.rows[i].thinking = p.thinking.clone();
        match status {
            "refused" => {
                // (possibly after partial output: the partial and the question go)
                self.rows[i].text.clear();
                self.rows[i].status = "refused".into();
                self.history.pop();
                self.save(false);
            }
            "error" => {
                self.rows[i].status = "error".into();
                self.rows[i].text = p.error.clone();
                self.history.pop();
                self.save(false);
            }
            "stopped" if p.text.is_empty() => {
                self.rows.pop();
                self.history.pop();
            }
            _ => {
                self.rows[i].status = if status == "stopped" { "stopped" } else { "done" }.into();
                let mut turn = json!({"role": "assistant", "text": p.text});
                if p.provider == "anthropic" && status == "done" {
                    turn["blocks"] = Value::Array(p.echo_blocks());
                }
                self.history.push(turn);
                self.save(false);
            }
        }
    }

    fn fail_with(&mut self, text: &str) {
        if let Some(r) = self.rows.last_mut() {
            r.status = "error".into();
            r.text = text.to_string();
        }
        self.history.pop();
        self.save(false);
    }

    /// HTTP errors: what happened and how to fix it
    pub fn error_text(&self, status: u16, body: &str) -> String {
        let j: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let msg = j["error"]["message"].as_str().or_else(|| j[0]["error"]["message"].as_str()).unwrap_or("");
        let p = self.provider_name();
        match status {
            0 => format!("Couldn't reach {p}. Check your internet connection{}.", if self.model.provider == "ollama" { " or that Ollama is running" } else { "" }),
            401 | 403 => format!("Your {p} credential was rejected. Check it in Settings → AI{}.", if self.model.auth == "token" { "; OAuth access tokens expire and must be refreshed" } else { "" }),
            402 => format!("There's a billing problem on your {p} account."),
            404 => format!("The model “{}” isn't available. Check its ID, provider and connection in Settings → AI.", self.model.id),
            429 => format!("Rate limited by {p}. Wait a moment and try again."),
            s if s == 529 || s >= 500 => format!("{p} is having trouble right now. Try again in a moment."),
            _ if !msg.is_empty() => msg.to_string(),
            s => format!("Request failed ({s})."),
        }
    }

    /// Spotlight's "Continue in chat": its question and answer, into the open chat
    pub fn continue_quick(&mut self, question: &str, answer: &str) {
        if answer.is_empty() || self.busy() {
            return;
        }
        self.history.push(json!({"role": "user", "text": question}));
        self.history.push(json!({"role": "assistant", "text": answer}));
        self.rows.push(Row::new("user", question, "done"));
        let mut r = Row::new("assistant", answer, "done");
        r.model = self.model.name.clone();
        self.rows.push(r);
        self.changed = true;
        self.save(false);
    }
}

/// What a submit led to
pub enum Submitted {
    Nothing,
    /// a model was chosen (/model): its key, to save
    Model(String),
    /// a request to run (stream it): the request, its wire format, its number, the cancel switch
    Request(Request, String, u64, Arc<AtomicU64>),
}

fn rand01() -> f64 {
    let mut b = [0u8; 8];
    // (getrandom: no crate for one number)
    if unsafe { libc::getrandom(b.as_mut_ptr() as *mut libc::c_void, 8, 0) } != 8 {
        b = (chats::now_ms() as u64).to_le_bytes();
    }
    (u64::from_le_bytes(b) >> 11) as f64 / (1u64 << 53) as f64
}

/// Run a request on a thread of its own: Update at most every 50 ms while
/// it streams, then Done. Stops early once `cancel` no longer holds `gen`.
pub fn stream(req: Request, protocol: String, gen: u64, cancel: Arc<AtomicU64>, send: impl Fn(Event) + Send + 'static) {
    std::thread::spawn(move || {
        let live = || cancel.load(Ordering::SeqCst) == gen;
        let mut parser = sse::Parser::new(&protocol);
        let (status, body) = match run(&req, &mut parser, &live, &send, gen) {
            Ok(v) => v,
            Err(e) => {
                // (the error's text names the host at most; never a header)
                eprintln!("tessera-ui: AI request failed: {}", short(&e.to_string()));
                (0, String::new())
            }
        };
        if live() {
            send(Event::Done { gen, status, parser: Box::new(parser), body });
        }
    });
}

fn short(s: &str) -> String {
    s.chars().take(200).collect()
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(20)))
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

fn run(req: &Request, parser: &mut sse::Parser, live: &dyn Fn() -> bool, send: &dyn Fn(Event), gen: u64) -> Result<(u16, String), ureq::Error> {
    let mut r = agent().post(&req.url);
    for (k, v) in &req.headers {
        r = r.header(k.as_str(), v.as_str());
    }
    let resp = r.send(serde_json::to_vec(&req.body).unwrap_or_default())?;
    let status = resp.status().as_u16();
    let mut reader = resp.into_body().into_reader();
    let mut buf = [0u8; 8192];
    if status != 200 {
        let mut body = Vec::new();
        let _ = reader.take(256 * 1024).read_to_end(&mut body);
        return Ok((status, String::from_utf8_lossy(&body).into_owned()));
    }
    let mut last = Instant::now();
    let mut dirty = false;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            // (cut off mid-answer: what came is the answer, as XHR's DONE)
            Err(_) => break,
        };
        if !live() {
            return Ok((status, String::new()));
        }
        dirty |= parser.feed(&buf[..n]);
        if dirty && last.elapsed() >= Duration::from_millis(50) {
            send(Event::Update { gen, text: parser.text.clone(), thinking: parser.thinking.clone() });
            last = Instant::now();
            dirty = false;
        }
    }
    parser.feed(b"\n");
    Ok((status, String::new()))
}

/// Start the service: the credentials, read on a thread (secret-tool per
/// slot), arrive as a `keys::Keys` reading
pub fn spawn(tx: Sender<crate::Msg>) {
    std::thread::spawn(move || {
        let cfg = crate::theme::read_json();
        let mut slots: Vec<String> = keys::SLOTS.iter().map(|s| s.to_string()).collect();
        for m in models::read(&cfg["ai"]["models"]) {
            if !slots.contains(&m.key) {
                slots.push(m.key);
            }
        }
        let k = keys::Keys::load(&slots);
        let _ = tx.send(crate::Msg::Data(Box::new(k)));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(dir: Option<&std::path::Path>) -> Chat {
        let mut c = Chat::new(dir.map(|d| Store::new(d.to_path_buf())));
        c.configure(&json!({"ai": {
            "models": [r#"{"id":"atlas-8b","name":"Atlas 8B","provider":"custom","endpoint":"http://127.0.0.1:47311/v1","auth":"none","vision":true}"#,
                       {"id": "text-only", "name": "Text", "provider": "custom", "endpoint": "http://127.0.0.1:1/v1", "auth": "none", "vision": false}],
            "model": "custom|atlas-8b|http%3A%2F%2F127.0.0.1%3A47311%2Fv1|none||"}}));
        c
    }
    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tessera-ui-ai-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_question_streams_and_is_saved() {
        let d = tmp("ask");
        let mut c = chat(Some(&d));
        assert_eq!(c.model.name, "Atlas 8B");
        assert!(!c.needs_key());
        let Submitted::Request(req, proto, gen, _) = c.submit("Plan a weekend in Lisbon") else { panic!("no request") };
        assert_eq!(req.url, "http://127.0.0.1:47311/v1/chat/completions");
        assert_eq!(proto, "custom");
        assert!(req.body["messages"][0]["content"].as_str().unwrap().starts_with("You are the assistant built into Tessera"));
        assert_eq!(req.body["messages"][1], json!({"role": "user", "content": "Plan a weekend in Lisbon"}));
        assert!(c.busy());
        assert!(c.update(gen, "**A weekend".into(), String::new()));
        assert!(!c.update(gen + 1, "late".into(), String::new()));
        assert_eq!(c.rows[1].text, "**A weekend");
        let mut p = sse::Parser::new("custom");
        p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"**A weekend in Lisbon**\"}}]}\n");
        assert!(c.done(gen, 200, &p, ""));
        assert_eq!(c.rows[1].status, "done");
        assert_eq!(c.history.len(), 2);
        // saved: the file and the index, the QML format
        let idx = Store::new(d.clone()).read_index();
        assert_eq!(idx.current, c.current_id);
        assert_eq!(idx.chats[0].title, "Plan a weekend in Lisbon");
        assert_eq!(idx.chats[0].model_name, "Atlas 8B");
        let s = Store::new(d.clone()).read_session(&c.current_id).unwrap();
        assert_eq!(s["rows"][1]["text"], "**A weekend in Lisbon**");
        // a new chat, then the old one back
        let id = c.current_id.clone();
        c.new_chat();
        assert!(c.rows.is_empty() && c.current_id.is_empty());
        assert_eq!(c.open_chat(&id), Ok(None));
        assert_eq!(c.rows.len(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stop_keeps_what_came_and_errors_say_how_to_fix() {
        let mut c = chat(None);
        let Submitted::Request(_, _, gen, cancel) = c.submit("hi") else { panic!() };
        c.update(gen, "partial".into(), String::new());
        c.stop();
        assert_eq!(cancel.load(Ordering::SeqCst), 0);
        assert_eq!((c.rows[1].status.as_str(), c.rows[1].text.as_str()), ("stopped", "partial"));
        let Submitted::Request(_, _, gen, _) = c.submit("again") else { panic!() };
        c.stop(); // (nothing came: the empty answer goes, the question stays on screen, not in history)
        assert_eq!(c.rows.len(), 3);
        assert_eq!(c.history.len(), 2);
        let _ = gen;
        let Submitted::Request(_, _, gen, _) = c.submit("x") else { panic!() };
        c.done(gen, 0, &sse::Parser::default(), "");
        assert_eq!(c.rows.last().unwrap().text, "Couldn't reach OpenAI-compatible endpoint. Check your internet connection.");
        assert_eq!(c.error_text(429, ""), "Rate limited by OpenAI-compatible endpoint. Wait a moment and try again.");
        assert_eq!(c.error_text(400, r#"{"error":{"message":"Bad thing"}}"#), "Bad thing");
        assert_eq!(c.error_text(418, "nope"), "Request failed (418).");
    }

    #[test]
    fn commands_pictures_and_notes() {
        let mut c = chat(None);
        assert!(matches!(c.submit("/model Text"), Submitted::Model(k) if k.contains("text-only")));
        assert_eq!(c.rows.last().unwrap().text, "Switched to Text.");
        c.attach(Picture { path: "/run/a.jpg".into(), data: "AAA".into(), media_type: "image/jpeg".into(), lens: false });
        assert!(matches!(c.submit(""), Submitted::Nothing));
        assert!(c.rows.last().unwrap().text.starts_with("Text can't read pictures. Switch to Atlas 8B"));
        assert_eq!(c.can_see().unwrap().name, "Atlas 8B");
        c.set_model("atlas-8b");
        let Submitted::Request(req, _, _, _) = c.submit("") else { panic!() };
        assert_eq!(req.body["messages"][1]["content"][0]["image_url"]["url"], "data:image/jpeg;base64,AAA");
        assert_eq!(c.rows[c.rows.len() - 2].image, "/run/a.jpg");
        assert_eq!(c.rows[c.rows.len() - 2].text, "What's in this picture?");
        c.stop();
        for i in 0..5 {
            c.attach(Picture { path: format!("/p{i}"), ..Default::default() });
        }
        assert_eq!(c.attachments.len(), MAX_IMAGES);
        assert!(c.rows.last().unwrap().text.contains("can carry 4 pictures"));
        c.attachments.clear();
        let Submitted::Request(_, _, _, _) = c.search_picture(Picture { path: "/run/c.jpg".into(), data: "C".into(), media_type: "image/jpeg".into(), lens: false }) else { panic!() };
        assert_eq!(c.rows.last().unwrap().search, "/run/c.jpg");
        assert_eq!(c.rows[c.rows.len() - 2].text, CIRCLE_QUESTION);
        assert!(matches!(c.submit("/nothing-like-it"), Submitted::Nothing)); // (busy)
    }

    #[test]
    fn delete_undo_rename_pin() {
        let d = tmp("hist");
        let mut c = chat(Some(&d));
        let Submitted::Request(_, _, gen, _) = c.submit("first question") else { panic!() };
        let mut p = sse::Parser::new("custom");
        p.text = "answer".into();
        c.done(gen, 200, &p, "");
        let id = c.current_id.clone();
        c.rename_chat(&id, "  Renamed \n chat ");
        c.set_pinned(&id, true);
        let idx = Store::new(d.clone()).read_index();
        assert_eq!((idx.chats[0].title.as_str(), idx.chats[0].pinned), ("Renamed chat", true));
        c.delete_chat(&id);
        assert!(c.rows.is_empty() && c.sessions.is_empty());
        assert_eq!(Store::new(d.clone()).read_index().trash, vec![id.clone()]);
        assert!(d.join(format!("{id}.json")).exists()); // (until Undo's time is over)
        c.undo_delete();
        assert_eq!(c.current_id, id);
        assert_eq!(c.rows.len(), 2);
        c.delete_chat(&id);
        c.finish_delete();
        assert!(!d.join(format!("{id}.json")).exists());
        assert!(Store::new(d.clone()).read_index().trash.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn continue_from_spotlight() {
        let mut c = chat(None);
        c.continue_quick("What's 2+2?", "4");
        assert_eq!(c.rows.iter().map(|r| r.role.as_str()).collect::<Vec<_>>(), ["user", "assistant"]);
        assert_eq!(c.history[1]["text"], "4");
    }
}
