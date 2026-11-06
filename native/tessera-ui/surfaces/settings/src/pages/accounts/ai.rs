//! AI & Sidebar (pages/ai.qml, with sidebar.qml as its part "Sidebar"):
//! your models (the records are config.json's `ai.models`, read with
//! services/ai/models.rs as the shell's AiModels.js reads them), the form
//! that adds or edits one, the conversation's style and your note, the chat
//! history. Saving a model, removing it (with 8 s to undo) and its
//! credential are the shell's (Ai, AiKeys: the keyring): a typed credential
//! goes to it once, and only whether one is set ever comes back.

use super::super::super::model::{btn, Ctx, Page, Row};
use super::{feed, field, s};
use crate::services::ai::models::{self, Model};
use serde_json::{json, Value};

/// AiPrompt.js's MAX_NOTE
pub const MAX_NOTE: usize = 600;

/// Your note as it's kept and sent (AiPrompt.js's clean): no control or
/// direction characters, no section tags, at most two newlines in a row,
/// 600 characters at most
pub fn clean(text: &str) -> String {
    let t: String = text
        .chars()
        .filter(|&c| {
            let n = c as u32;
            !((n <= 0x08) || (0x0b..=0x1f).contains(&n) || n == 0x7f || (0x200b..=0x200f).contains(&n) || (0x202a..=0x202e).contains(&n) || (0x2066..=0x2069).contains(&n))
        })
        .collect();
    // </?\s*(person_preferences|system|instructions?)\b[^>]*>, any case
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let after_slash = tail.strip_prefix('/').unwrap_or(tail).trim_start();
        let lower = after_slash.to_lowercase();
        let word = ["person_preferences", "instructions", "instruction", "system"].into_iter().find(|w| {
            lower.starts_with(w) && !lower[w.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
        });
        match (word, tail.find('>')) {
            (Some(_), Some(end)) => rest = &tail[end + 1..],
            _ => {
                out.push('<');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    // \n{3,} -> \n\n
    let mut collapsed = String::new();
    let mut run = 0;
    for c in out.chars() {
        if c == '\n' {
            run += 1;
            if run <= 2 {
                collapsed.push(c);
            }
        } else {
            run = 0;
            collapsed.push(c);
        }
    }
    let t = collapsed.trim();
    if t.chars().count() > MAX_NOTE {
        t.chars().take(MAX_NOTE).collect::<String>().trim().to_string()
    } else {
        t.to_string()
    }
}

/// The form's fields (page-local state) with their starting values
fn fld(cx: &Ctx, k: &str) -> String {
    match cx.local.get(&format!("ai.{k}")) {
        Some(Value::String(s)) => s.clone(),
        _ => match k {
            "provider" => "aistudio".into(),
            "auth" => "apiKey".into(),
            "location" => "global".into(),
            "tokens" => "4096".into(),
            _ => String::new(),
        },
    }
}

/// The record the form describes (ai.qml's draft)
fn draft(cx: &Ctx) -> Value {
    let provider = fld(cx, "provider");
    let id = fld(cx, "id");
    let vision = match cx.local.get("ai.vision") {
        Some(Value::Bool(b)) => *b,
        _ => models::normalize(&json!({"provider": provider, "id": id})).vision,
    };
    json!({
        "id": id,
        "name": fld(cx, "name"),
        "provider": provider,
        "auth": fld(cx, "auth"),
        "endpoint": fld(cx, "endpoint"),
        "project": fld(cx, "project"),
        "location": fld(cx, "location"),
        "thinking": cx.local_b("ai.thinking"),
        "maxTokens": fld(cx, "tokens"),
        "vision": vision,
    })
}

/// Load a model into the form (None: a new one)
fn edit(cx: &mut Ctx, m: Option<&Model>) {
    let m = m.cloned();
    let d = Model::default();
    let x = m.as_ref().unwrap_or(&d);
    cx.local_set("ai.editing", x.key.clone());
    cx.local_set("ai.provider", if m.is_some() { x.provider.clone() } else { "aistudio".into() });
    cx.local_set("ai.auth", if m.is_some() { x.auth.clone() } else { "apiKey".into() });
    cx.local_set("ai.thinking", x.thinking);
    if m.is_some() {
        cx.local_set("ai.vision", x.vision);
    } else {
        cx.local.remove("ai.vision");
    }
    cx.local_set("ai.name", x.name.clone());
    cx.local_set("ai.id", x.id.clone());
    cx.local_set("ai.endpoint", x.endpoint.clone());
    cx.local_set("ai.project", x.project.clone());
    cx.local_set("ai.location", if x.location.is_empty() { "global".to_string() } else { x.location.clone() });
    cx.local_set("ai.tokens", if x.max_tokens > 0.0 && x.max_tokens.is_finite() { format!("{}", x.max_tokens) } else { "4096".into() });
    cx.local_set("ai.cred", "");
    cx.local_set("ai.msg", "");
    cx.local_set("ai.msgErr", false);
}

/// What saving the form says first ("" when it can go to the shell), as
/// Ai.saveModel checks
pub fn check(raw: &Value, saved: &[Model], editing: &str) -> String {
    let m = models::normalize(raw);
    let e = models::validate(&m);
    if !e.is_empty() {
        return e.to_string();
    }
    if saved.iter().any(|x| x.key == m.key && x.key != editing) {
        return "That model and connection are already in your list. Edit the existing entry.".into();
    }
    String::new()
}

fn save(cx: &mut Ctx) {
    let raw = draft(cx);
    let saved = models::read(&cx.get("ai.models"));
    let editing = cx.local_s("ai.editing");
    let error = check(&raw, &saved, &editing);
    if !error.is_empty() {
        cx.local_set("ai.msg", error);
        cx.local_set("ai.msgErr", true);
        return;
    }
    let cred = cx.local_s("ai.cred");
    cx.ask("accounts", "aiSave", json!([raw, editing, cred]));
    let m = models::normalize(&raw);
    edit(cx, Some(&m));
    cx.local_set("ai.msg", "Model saved and selected.");
}

/// A field of the form, its text in `ai.<k>`
fn form_field(cx: &mut Ctx, k: &str, label: &str, placeholder: &str, width: f32) -> Row {
    let key = format!("ai.{k}");
    if !cx.local.contains_key(&key) {
        let v = fld(cx, k);
        cx.local_set(&key, v);
    }
    field(cx, &key, label, placeholder, width, false, None)
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let ai = &f["ai"];
    let saved = models::read(&cx.get("ai.models"));
    let selected = cx.s("ai.model");
    let model = saved.iter().find(|m| m.key == selected).or(saved.first()).cloned().unwrap_or_else(Model::none);

    // ── Your models ──
    let (labels, keys): (Vec<String>, Vec<String>) = if saved.is_empty() {
        (vec!["No models added".into()], vec![String::new()])
    } else {
        saved.iter().map(|m| (format!("{} · {}", m.name, models::provider_name(&m.provider)), m.key.clone())).unzip()
    };
    let cur = keys.iter().position(|k| *k == model.key).map_or(-1, |i| i as i32);
    let k2 = keys.clone();
    let pick = cx.on(move |cx, a| {
        if let Some(k) = k2.get(a.i.max(0) as usize) {
            if !k.is_empty() {
                cx.ask("accounts", "aiSetModel", json!([k]));
            }
        }
    });
    let mut rows = vec![Row::new("Selected model")
        .detail(if model.configured() { models::provider_name(&model.provider) } else { "Add your first model below".into() })
        .choice(&labels, cur, pick)
        .enabled(!saved.is_empty())
        .width(270.0)];
    for m in &saved {
        let m2 = m.clone();
        let act = cx.on(move |cx, a| {
            if a.i == 0 {
                edit(cx, Some(&m2));
            } else {
                cx.ask("accounts", "aiRemove", json!([m2.key]));
                if cx.local_s("ai.editing") == m2.key {
                    edit(cx, None);
                }
            }
        });
        rows.push(Row::new(m.name.clone()).detail(format!("{} · {}", models::provider_name(&m.provider), m.id)).buttons(vec![btn("Edit"), btn("Remove").danger()], act));
    }
    let g = p.group("Your models");
    g.footer("Add the model IDs available on your provider account. Only your saved models appear in the sidebar and Search.");
    for r in rows {
        g.row(r);
    }

    // A removed model can come back for a few seconds
    let undo = s(&ai["undo"]);
    if !undo.is_empty() {
        let u = cx.on(|cx, _| cx.ask("accounts", "aiUndo", Value::Null));
        p.group("").row(Row::new(undo).buttons(vec![btn("Undo").icon("arrow-counter-clockwise")], u));
    }

    // ── Add or edit a model ──
    let editing = cx.local_s("ai.editing");
    let provider = fld(cx, "provider");
    let auth = fld(cx, "auth");
    let thinking = cx.local_b("ai.thinking");
    let raw = draft(cx);
    let vision_now = raw["vision"].as_bool().unwrap_or(false);
    let names: Vec<String> = models::PROVIDERS.iter().map(|p| p.1.to_string()).collect();
    let ids: Vec<&'static str> = models::PROVIDERS.iter().map(|p| p.0).collect();
    let pcur = ids.iter().position(|x| *x == provider).map_or(-1, |i| i as i32);
    let ppick = cx.on(move |cx, a| {
        if let Some(v) = ids.get(a.i.max(0) as usize) {
            cx.local_set("ai.provider", *v);
            cx.local_set("ai.auth", if *v == "ollama" { "none" } else { "apiKey" });
            cx.local_set("ai.msg", "");
        }
    });
    let auth_opts: Vec<(&str, &str)> = if provider == "vertex" { vec![("apiKey", "API key (Vertex Express)"), ("token", "OAuth access token")] } else { vec![("apiKey", "API key"), ("none", "No authentication")] };
    let alabels: Vec<String> = auth_opts.iter().map(|x| x.1.to_string()).collect();
    let acur = auth_opts.iter().position(|x| x.0 == auth).map_or(-1, |i| i as i32);
    let avals: Vec<String> = auth_opts.iter().map(|x| x.0.to_string()).collect();
    let apick = cx.on(move |cx, a| {
        if let Some(v) = avals.get(a.i.max(0) as usize) {
            cx.local_set("ai.auth", v.clone());
        }
    });
    // Whether a credential is there for this connection (never the credential)
    let m = models::normalize(&raw);
    let has_key = m.auth == "none"
        || m.provider == "ollama"
        || ai["hasKey"][&m.key].as_bool().unwrap_or(false)
        || ai["envKeys"].as_array().is_some_and(|e| e.iter().any(|x| x.as_str() == Some(models::credential_provider(&m).as_str())));
    let ollama_url = cx.s("ai.ollamaUrl");
    let vt = cx.on(move |cx, _| cx.local_set("ai.vision", !vision_now));
    let tt = cx.on(move |cx, _| cx.local_set("ai.thinking", !thinking));
    let msg = cx.local_s("ai.msg");
    let msg_err = cx.local_b("ai.msgErr");
    let act = cx.on(|cx, a| {
        if a.i == 0 {
            save(cx);
        } else {
            edit(cx, None);
        }
    });
    let rows = vec![
        form_field(cx, "name", "Display name", "Name shown in Tessera", 270.0).detail("Optional; defaults to the model ID"),
        Row::new("Provider").detail(if provider == "vertex" { "Gemini models on Vertex AI" } else { "" }).choice(&names, pcur, ppick).width(270.0),
        form_field(cx, "id", "Model ID", "Model ID", 270.0).detail(if provider == "openrouter" { "Use the full provider/model ID from OpenRouter" } else { "Copy the exact model ID from your provider" }),
        Row::new("Authentication").visible(provider == "vertex" || provider == "custom").choice(&alabels, acur, apick).width(270.0),
        form_field(cx, "endpoint", "API base URL", &if provider == "ollama" { format!("{ollama_url}/v1") } else { "https://your-server.example/v1".to_string() }, 270.0)
            .visible(provider == "custom" || provider == "ollama")
            .detail("Include /v1 if your server uses it; omit /chat/completions"),
        form_field(cx, "project", "Google Cloud project", "Project ID or number", 270.0).visible(provider == "vertex" && auth == "token"),
        form_field(cx, "location", "Location", "", 270.0).visible(provider == "vertex" && auth == "token").detail("A supported region, or global"),
        field(cx, "ai.cred", if auth == "token" { "OAuth access token" } else { "API key" }, "Paste or replace credential", 270.0, true, None)
            .visible(provider != "ollama" && auth != "none")
            .detail(if auth == "token" { "Short-lived; replace it when it expires" } else if has_key { "A saved credential or environment variable is available" } else { "Paste a credential for this connection" }),
        form_field(cx, "tokens", "Max output tokens", "", 120.0).visible(provider == "anthropic").detail("Must fit the selected model's output limit"),
        Row::new("Reads pictures").detail("Screenshots, photos and circle to search need a model that can see. Guessed from the model ID; change it if that's wrong.").switch(vision_now, vt),
        Row::new("Adaptive thinking").visible(provider == "anthropic").detail("Enable only if this model supports adaptive thinking").switch(thinking, tt),
        // Status of the last save: an error says how to fix it
        Row::new(msg).tone(if msg_err { 2 } else { 1 }).buttons(vec![btn(if editing.is_empty() { "Add model" } else { "Save changes" }).primary(), btn("New model")], act),
    ];
    let g = p.group(if editing.is_empty() { "Add model" } else { "Edit model" });
    g.footer("Each model keeps its own credential, so a second model on the same provider needs its key pasted again. Credentials are stored in your system keyring, or a private file when no keyring is available. Provider environment variables take priority. Leave the credential field empty to keep an existing credential.");
    for r in rows {
        g.row(r);
    }

    // ── Conversation ──
    let efforts = ["low", "medium", "high", "xhigh", "max"];
    let effort = cx.s("ai.effort");
    let ep = cx.pick("ai.effort", &efforts);
    let styles = ["balanced", "concise", "friendly", "professional", "playful"];
    let style = cx.s("ai.style");
    let slabels: Vec<String> = ["Balanced", "Concise", "Friendly", "Professional", "Playful"].iter().map(|x| x.to_string()).collect();
    let sp = cx.pick("ai.style", &styles);
    // (the note as typed, else as kept)
    let note = match cx.local.get("ai.note") {
        Some(Value::String(t)) => t.clone(),
        _ => cx.s("ai.customInstructions"),
    };
    let note_act = cx.on(|cx, a| {
        let t: String = a.s.chars().take(MAX_NOTE).collect();
        if a.i == 1 {
            cx.set("ai.customInstructions", clean(&t));
            cx.local.remove("ai.note");
        } else {
            cx.local_set("ai.note", t);
        }
    });
    let g = p.group("Conversation");
    g.row(Row::new("Effort")
        .visible(model.provider == "anthropic" && model.thinking)
        .detail("For models with adaptive thinking enabled")
        .segmented(&["Low", "Medium", "High", "X-High", "Max"], efforts.iter().position(|x| *x == effort).unwrap_or(0) as i32, ep)
        .width(300.0));
    g.row(Row::new("Style").detail("How the assistant sounds").choice(&slabels, styles.iter().position(|x| *x == style).map_or(-1, |i| i as i32), sp).width(200.0));
    g.row(Row::new("Your note").detail(format!("How it should answer: tone, length, what to call you. Tessera's own rules stay in place. {} of {MAX_NOTE}", note.chars().count())));
    // (a few lines, in the group: settings-accounts.slint's "note")
    // (num: its place in the group, for Material's tile: 2 the last)
    g.row(Row::custom("accounts", "note").id(note_act).text_value(note).placeholder("For example: answer in British English, and keep it short.").num(2.0));

    // ── Chat history: deleting them all asks first, here ──
    let keep = cx.b("ai.keepHistory");
    let kt = cx.toggle("ai.keepHistory");
    let days = cx.i("ai.historyDays");
    let dvals = [0i64, 30, 90];
    let dcur = dvals.iter().position(|v| *v == days).map_or(-1, |i| i as i32);
    let dp = cx.pick_values("ai.historyDays", dvals.iter().map(|v| Value::from(*v)).collect());
    let count = ai["sessions"].as_i64().unwrap_or(0);
    let pinned = ai["pinned"].as_i64().unwrap_or(0);
    let confirming = cx.local_b("ai.confirm");
    let detail = saved_chats(count, pinned, confirming);
    let mut opts = Vec::new();
    if confirming {
        opts.push(btn("Keep chats"));
    }
    opts.push(btn(if confirming { if count == 1 { "Delete 1 chat".to_string() } else { format!("Delete {count} chats") } } else { "Delete all chats".into() }).danger().off(count == 0));
    let del = cx.on(move |cx, a| {
        let keep_index = if confirming { 0 } else { -1 };
        if a.i == keep_index {
            cx.local_set("ai.confirm", false);
        } else if !confirming {
            cx.local_set("ai.confirm", true);
        } else {
            cx.local_set("ai.confirm", false);
            cx.ask("accounts", "aiDeleteAll", Value::Null);
        }
    });
    p.group("Chat history")
        .footer("Chats are saved on this computer only, readable by you alone. Pictures are never saved.")
        .row(Row::new("Keep chat history").detail(if keep { "Past chats are in the sidebar, under the clock" } else { "Chats aren't saved: each is forgotten a minute after the sidebar closes" }).switch(keep, kt))
        .row(Row::new("Delete chats older than").detail("Pinned chats are always kept").choice(&["Never".to_string(), "30 days".into(), "90 days".into()], dcur, dp).width(200.0))
        .row(Row::new("Saved chats").detail(detail).buttons(opts, del));

    // ── More options ──
    p.more(cx);
    let st = cx.b("ai.showThinking");
    let stt = cx.toggle("ai.showThinking");
    p.group("Conversation").row(Row::new("Show reasoning summary").switch(st, stt));
    p.end_more();

    p.part(cx, "Sidebar", sidebar);
}

/// "Saved chats"' line, and its question while it asks (ai.qml)
pub fn saved_chats(count: i64, pinned: i64, confirming: bool) -> String {
    if !confirming {
        return match count {
            0 => "None yet".into(),
            1 => format!("1 chat{}", if pinned > 0 { ", pinned" } else { "" }),
            n => format!("{n} chats{}", if pinned > 0 { format!(", {pinned} pinned") } else { String::new() }),
        };
    }
    let q = if count == 1 { "Delete 1 chat? ".to_string() } else { format!("Delete {count} chats? ") };
    let tail = if pinned == 0 {
        String::new()
    } else if pinned == count {
        if count == 1 { " It's pinned, and goes too.".into() } else { " They're all pinned, and go too.".into() }
    } else if pinned == 1 {
        " Your pinned chat goes too.".into()
    } else {
        format!(" Your {pinned} pinned chats go too.")
    };
    format!("{q}This can't be undone.{tail}")
}

/// The sidebar's options (sidebar.qml)
pub fn sidebar(cx: &mut Ctx, p: &mut Page) {
    // (only whether a key is there: it's never shown)
    let has = !cx.get("emoji.klipyKey").as_str().unwrap_or("").is_empty();
    let mut key = field(cx, "ai.klipy", "KLIPY app key", if has { "Replace key" } else { "Paste key" }, 240.0, true, None).detail(if has { "Key saved" } else { "No key" });
    let kid = cx.on(|cx, a| {
        if a.i == 1 {
            let t = a.s.trim().to_string();
            if !t.is_empty() {
                cx.set("emoji.klipyKey", t);
                cx.local_set("ai.klipy", "");
                return;
            }
        }
        cx.local_set("ai.klipy", a.s.clone());
    });
    key.id = kid;
    key.on = true; // (committed when it loses focus too)
    let filters = ["off", "low", "medium", "high"];
    let cur = cx.s("emoji.gifFilter");
    let fp = cx.pick("emoji.gifFilter", &filters);
    p.group("GIFs")
        .footer("The emoji picker's GIF tab (Super+.) uses KLIPY. Get a free app key at partner.klipy.com/api-keys and paste it here.")
        .row(key)
        .row(Row::new("Content filter").segmented(&["Off", "Low", "Medium", "High"], filters.iter().position(|x| *x == cur).unwrap_or(0) as i32, fp).width(260.0));

    p.more(cx);
    let booru = cx.b("sidebar.booru");
    let bt = cx.toggle("sidebar.booru");
    let nsfw = cx.b("sidebar.booruNsfw");
    let nt = cx.toggle("sidebar.booruNsfw");
    let gaming = cx.b("overlays.gaming");
    let gt = cx.toggle("overlays.gaming");
    p.group("Anime")
        .footer("Images come from yande.re, Konachan or Danbooru. Explicit results are off unless you switch them on.")
        .row(Row::new("Show Anime tab").switch(booru, bt))
        .row(Row::new("Allow explicit results").switch(nsfw, nt));
    p.group("Overlay")
        .footer("The overlay (Super+G) always has Notes, Resources, Recorder and the volume mixer.")
        .row(Row::new("Gaming overlays").detail("Adds a floating image, a crosshair and an FPS limit (through MangoHud) to the overlay").switch(gaming, gt));
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_is_cleaned_as_ai_prompt_js() {
        assert_eq!(clean("  hi\u{200b} there \n\n\n\nok "), "hi there \n\nok");
        assert_eq!(clean("a <System>x</system> b"), "a x b");
        assert_eq!(clean("<instructions foo>keep</Instructions>"), "keep");
        assert_eq!(clean("1 < 2 and <systems>"), "1 < 2 and <systems>");
        assert_eq!(clean(&"x".repeat(700)).chars().count(), MAX_NOTE);
    }

    #[test]
    fn saved_chats_say_how_many() {
        assert_eq!(saved_chats(0, 0, false), "None yet");
        assert_eq!(saved_chats(1, 1, false), "1 chat, pinned");
        assert_eq!(saved_chats(5, 2, false), "5 chats, 2 pinned");
        assert_eq!(saved_chats(3, 3, true), "Delete 3 chats? This can't be undone. They're all pinned, and go too.");
        assert_eq!(saved_chats(3, 1, true), "Delete 3 chats? This can't be undone. Your pinned chat goes too.");
    }

    #[test]
    fn saving_checks_first() {
        let saved = models::read(&json!([{"provider": "openai", "id": "gpt-4o"}]));
        assert!(check(&json!({"provider": "openai", "id": ""}), &saved, "").starts_with("Enter the model ID"));
        assert!(check(&json!({"provider": "openai", "id": "gpt-4o"}), &saved, "").starts_with("That model and connection"));
        assert_eq!(check(&json!({"provider": "openai", "id": "gpt-4o"}), &saved, &saved[0].key), "");
    }
}
