//! The person's models (Settings → AI) and the request each provider takes:
//! shell/services/AiModels.js in Rust, the same normalising, checks, keys and
//! wire formats (tests/ai_models.test.cjs holds the JS side; the tests below
//! the same cases). Provider defaults describe API formats, never a catalogue
//! of model IDs; no credential is ever part of a record.

use serde_json::{json, Map, Value};

pub const PROVIDERS: &[(&str, &str)] = &[
    ("aistudio", "Google AI Studio"),
    ("vertex", "Google Vertex AI"),
    ("openrouter", "OpenRouter"),
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("ollama", "Ollama"),
    ("custom", "OpenAI-compatible endpoint"),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub key: String,
    pub id: String,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub project: String,
    pub location: String,
    /// apiKey | token | none
    pub auth: String,
    pub thinking: bool,
    /// (Anthropic's max output tokens; NaN when the record's isn't a number)
    pub max_tokens: f64,
    pub vision: bool,
}

impl Model {
    /// No model chosen yet (Ai.qml's placeholder)
    pub fn none() -> Self {
        Model { name: "Add a model".into(), auth: "none".into(), ..Default::default() }
    }
    pub fn configured(&self) -> bool {
        !self.id.is_empty()
    }
}

pub fn provider_name(provider: &str) -> String {
    PROVIDERS.iter().find(|p| p.0 == provider).map_or(provider, |p| p.1).to_string()
}

/// JS's String(value ?? "").trim()
fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// encodeURIComponent
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether a model reads pictures, when its record doesn't say: Claude and
/// Gemini do; elsewhere, IDs that name a vision model (AiModels.js's pattern)
pub fn guess_vision(provider: &str, id: &str) -> bool {
    if matches!(provider, "anthropic" | "aistudio" | "vertex") {
        return true;
    }
    let s = id.to_lowercase();
    let plain = ["gpt-4o", "gpt-4.1", "gpt-5", "o3-", "o4-", "claude", "gemini", "vision", "-vl-", "llava", "pixtral", "gemma3", "gemma-3", "minicpm-v", "moondream", "bakllava"];
    if plain.iter().any(|p| s.contains(p)) {
        return true;
    }
    let chars: Vec<char> = s.chars().collect();
    let at = |i: isize| if i < 0 { None } else { chars.get(i as usize).copied() };
    for i in 0..chars.len() as isize {
        // \bo[34]\b
        if at(i) == Some('o') && matches!(at(i + 1), Some('3' | '4')) && !word(at(i - 1)) && !word(at(i + 2)) {
            return true;
        }
        // [-_.]vl\b
        if matches!(at(i), Some('-' | '_' | '.')) && at(i + 1) == Some('v') && at(i + 2) == Some('l') && !word(at(i + 3)) {
            return true;
        }
    }
    // qwen.*vl
    s.find("qwen").is_some_and(|q| s[q + 4..].contains("vl"))
}

/// A record from config.json (`ai.models`: a JSON string or an object), as
/// AiModels.js's normalize
pub fn normalize(raw: &Value) -> Model {
    let p = text(&raw["provider"]);
    let provider = if p == "gemini" { "aistudio".to_string() } else { p };
    let mut m = Model {
        id: text(&raw["id"]),
        name: {
            let n = text(&raw["name"]);
            if n.is_empty() { text(&raw["id"]) } else { n }
        },
        endpoint: if provider == "custom" || provider == "ollama" { text(&raw["endpoint"]).trim_end_matches('/').to_string() } else { String::new() },
        project: if provider == "vertex" { text(&raw["project"]) } else { String::new() },
        location: if provider == "vertex" {
            let l = text(&raw["location"]);
            if l.is_empty() { "global".into() } else { l }
        } else {
            String::new()
        },
        auth: if provider == "ollama" {
            "none"
        } else if provider == "vertex" && raw["auth"] == "token" {
            "token"
        } else if provider == "custom" && raw["auth"] == "none" {
            "none"
        } else {
            "apiKey"
        }
        .into(),
        thinking: provider == "anthropic" && raw["thinking"] == true,
        max_tokens: if provider == "anthropic" {
            match &raw["maxTokens"] {
                Value::Null => 4096.0,
                Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
                Value::String(s) if s.trim().is_empty() => 0.0,
                Value::String(s) => s.trim().parse().unwrap_or(f64::NAN),
                Value::Bool(b) => *b as u8 as f64,
                _ => f64::NAN,
            }
        } else {
            0.0
        },
        vision: false,
        provider,
        key: String::new(),
    };
    m.vision = raw["vision"].as_bool().unwrap_or_else(|| guess_vision(&m.provider, &m.id));
    if m.provider == "aistudio" {
        m.id = m.id.strip_prefix("models/").unwrap_or(&m.id).to_string();
    }
    if m.provider == "vertex" {
        m.id = m.id.strip_prefix("publishers/google/models/").unwrap_or(&m.id).to_string();
    }
    if m.provider == "vertex" && m.auth == "apiKey" {
        m.project.clear();
        m.location = "global".into();
    }
    m.key = [&m.provider, &m.id, &m.endpoint, &m.auth, &m.project, &m.location].map(|s| encode_uri_component(s)).join("|");
    m
}

/// Loopback hosts: localhost, 127.x.x.x, [::1]
pub fn is_local(url: &str) -> bool {
    let Some(rest) = url.split_once("://").map(|(_, r)| r) else { return false };
    let host = if rest.starts_with('[') {
        rest.find(']').map_or("", |e| &rest[..=e])
    } else {
        rest.split(['/', ':']).next().unwrap_or("")
    }
    .to_lowercase();
    let octets: Vec<&str> = host.split('.').collect();
    host == "localhost"
        || host == "[::1]"
        || (octets.len() == 4 && octets[0] == "127" && octets[1..].iter().all(|o| (1..=3).contains(&o.len()) && o.chars().all(|c| c.is_ascii_digit())))
}

/// ^https?://[^\s/?#@]+(?:/[^\s?#]*)?$
fn endpoint_ok(e: &str) -> bool {
    let rest = e.strip_prefix("https://").or_else(|| e.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    let (host, path) = rest.find('/').map_or((rest, ""), |i| (&rest[..i], &rest[i..]));
    !host.is_empty() && !host.chars().any(|c| c.is_whitespace() || "/?#@".contains(c)) && !path.chars().any(|c| c.is_whitespace() || "?#".contains(c))
}

/// What's wrong with a record ("" when nothing)
pub fn validate(m: &Model) -> &'static str {
    let only = |s: &str, extra: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(c));
    if !PROVIDERS.iter().any(|p| p.0 == m.provider) {
        return "Choose a supported provider.";
    }
    if m.id.is_empty() || m.id.chars().any(|c| c.is_whitespace() || c == '?' || c == '#') {
        return "Enter the model ID supplied by your provider, without spaces or a URL.";
    }
    if (m.provider == "aistudio" || m.provider == "vertex") && !only(&m.id, "._-") {
        return "Enter a Google model ID, not a full endpoint URL.";
    }
    if m.provider == "custom" && m.endpoint.is_empty() {
        return "Enter the API base URL for this endpoint.";
    }
    if !m.endpoint.is_empty() && !endpoint_ok(&m.endpoint) {
        return "Use an http:// or https:// API base URL without credentials, query parameters or a fragment.";
    }
    // A key sent over plain HTTP can be read by anyone on the network
    if m.endpoint.starts_with("http:") && m.auth != "none" && !is_local(&m.endpoint) {
        return "Use https:// for remote endpoints. Plain http:// is only for this computer (localhost).";
    }
    if m.provider == "vertex" && m.auth == "token" {
        if !only(&m.project, "._:-") {
            return "Enter your Google Cloud project ID or project number.";
        }
        if !m.location.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') || m.location.is_empty() {
            return "Enter a Vertex AI region or global.";
        }
    }
    if m.provider == "anthropic" && !(m.max_tokens.fract() == 0.0 && m.max_tokens >= 1.0) {
        return "Max output tokens must be a positive whole number.";
    }
    ""
}

/// config.json's `ai.models` (JSON strings or objects): the valid ones, once each
pub fn read(entries: &Value) -> Vec<Model> {
    let mut out: Vec<Model> = Vec::new();
    for e in entries.as_array().into_iter().flatten() {
        let raw = match e {
            Value::String(s) => match serde_json::from_str::<Value>(s) {
                Ok(v) => v,
                Err(_) => continue,
            },
            Value::Object(_) => e.clone(),
            _ => continue,
        };
        if !raw.is_object() {
            continue;
        }
        let m = normalize(&raw);
        if validate(&m).is_empty() && !out.iter().any(|x| x.key == m.key) {
            out.push(m);
        }
    }
    out
}

/// The wire format: gemini, anthropic, or an OpenAI-compatible one
pub fn protocol(m: &Model) -> &str {
    if m.provider == "aistudio" || m.provider == "vertex" {
        "gemini"
    } else {
        &m.provider
    }
}

/// The keyring slot family a model's credential is looked up under
pub fn credential_provider(m: &Model) -> String {
    if m.provider == "vertex" && m.auth == "token" { "vertex-oauth".into() } else { m.provider.clone() }
}

pub fn credential_label(m: &Model) -> &'static str {
    if m.auth == "token" { "OAuth access token" } else { "API key" }
}

/// Options for one request: Tessera's system prompt (AiPrompt), and for
/// Spotlight's quick answers a lower effort, fewer tokens, no thinking
#[derive(Clone, Debug, Default)]
pub struct Opts {
    pub system: String,
    pub max_tokens: Option<i64>,
    pub thinking: Option<bool>,
    pub effort: Option<String>,
}

/// The `ai` settings a request reads
#[derive(Clone, Debug)]
pub struct Settings {
    pub show_thinking: bool,
    pub effort: String,
    pub ollama_url: String,
}

impl Settings {
    pub fn from(ai: &Value) -> Self {
        Settings {
            show_thinking: ai["showThinking"].as_bool().unwrap_or(true),
            effort: ai["effort"].as_str().unwrap_or("medium").to_string(),
            ollama_url: ai["ollamaUrl"].as_str().unwrap_or("http://localhost:11434").to_string(),
        }
    }
}

/// A request ready to send. (Debug leaves the headers out: they carry the key.)
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request").field("url", &self.url).field("headers", &self.headers.iter().map(|h| &h.0).collect::<Vec<_>>()).finish()
    }
}

#[cfg(test)]
impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|h| h.0 == name).map(|h| h.1.as_str())
    }
}

/// A turn's pictures: `images` (up to 4), or one `image` (saved before)
fn images_of(h: &Value) -> Vec<Value> {
    if let Some(a) = h["images"].as_array() {
        a.clone()
    } else if h["image"].is_object() {
        vec![h["image"].clone()]
    } else {
        Vec::new()
    }
}

/// One turn's content in a provider's format (`provider`: anthropic, gemini or another)
fn content(h: &Value, provider: &str) -> Value {
    let imgs = images_of(h);
    let t = h["text"].clone();
    let text = if t.is_null() { json!("") } else { t };
    match provider {
        "anthropic" => {
            if !h["blocks"].is_null() {
                return h["blocks"].clone();
            }
            if imgs.is_empty() {
                return text;
            }
            let mut v: Vec<Value> = imgs.iter().map(|i| json!({"type": "image", "source": {"type": "base64", "media_type": i["mediaType"], "data": i["data"]}})).collect();
            v.push(json!({"type": "text", "text": text}));
            Value::Array(v)
        }
        "gemini" => {
            let mut v: Vec<Value> = imgs.iter().map(|i| json!({"inline_data": {"mime_type": i["mediaType"], "data": i["data"]}})).collect();
            v.push(json!({"text": text}));
            Value::Array(v)
        }
        _ => {
            if imgs.is_empty() {
                return text;
            }
            let mut v: Vec<Value> = imgs.iter().map(|i| json!({"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", i["mediaType"].as_str().unwrap_or(""), i["data"].as_str().unwrap_or(""))}})).collect();
            v.push(json!({"type": "text", "text": text}));
            Value::Array(v)
        }
    }
}

/// The request for these turns ({role, text, images?, blocks?}), as
/// AiModels.js's buildRequest. Err: the record isn't valid.
pub fn build_request(m: &Model, turns: &[Value], opts: &Opts, settings: &Settings, credential: &str) -> Result<Request, String> {
    let error = validate(m);
    if !error.is_empty() {
        return Err(error.into());
    }
    let sys = opts.system.clone();
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if m.provider == "anthropic" {
        let mut body = Map::new();
        body.insert("model".into(), json!(m.id));
        body.insert("max_tokens".into(), opts.max_tokens.map_or(json!(m.max_tokens as i64), |t| json!(t)));
        body.insert("stream".into(), json!(true));
        body.insert("system".into(), json!(sys));
        body.insert("messages".into(), Value::Array(turns.iter().map(|h| json!({"role": h["role"], "content": content(h, "anthropic")})).collect()));
        // Only what the model was set up for: a model that doesn't support
        // adaptive thinking or effort rejects the whole request
        if m.thinking && opts.thinking != Some(false) {
            body.insert("thinking".into(), json!({"type": "adaptive", "display": if settings.show_thinking { "summarized" } else { "omitted" }}));
            body.insert("output_config".into(), json!({"effort": opts.effort.clone().unwrap_or_else(|| settings.effort.clone())}));
        }
        headers.push(("x-api-key".into(), credential.into()));
        headers.push(("anthropic-version".into(), "2026-10-21".into()));
        return Ok(Request { url: "https://api.anthropic.com/v1/messages".into(), headers, body: Value::Object(body) });
    }
    if m.provider == "aistudio" || m.provider == "vertex" {
        let base = if m.provider == "aistudio" {
            headers.push(("x-goog-api-key".into(), credential.into()));
            "https://generativelanguage.googleapis.com/v1beta/models/".to_string()
        } else if m.auth == "apiKey" {
            headers.push(("x-goog-api-key".into(), credential.into()));
            "https://aiplatform.googleapis.com/v1/publishers/google/models/".to_string()
        } else {
            let host = if m.location == "global" { "aiplatform.googleapis.com".to_string() } else { format!("{}-aiplatform.googleapis.com", m.location) };
            headers.push(("authorization".into(), format!("Bearer {credential}")));
            format!("https://{host}/v1/projects/{}/locations/{}/publishers/google/models/", encode_uri_component(&m.project), encode_uri_component(&m.location))
        };
        let mut body = Map::new();
        body.insert("systemInstruction".into(), json!({"parts": [{"text": sys}]}));
        body.insert("contents".into(), Value::Array(turns.iter().map(|h| json!({"role": if h["role"] == "assistant" { "model" } else { "user" }, "parts": content(h, "gemini")})).collect()));
        if let Some(t) = opts.max_tokens.filter(|t| *t != 0) {
            body.insert("generationConfig".into(), json!({"maxOutputTokens": t}));
        }
        return Ok(Request { url: format!("{base}{}:streamGenerateContent?alt=sse", encode_uri_component(&m.id)), headers, body: Value::Object(body) });
    }
    let base = if !m.endpoint.is_empty() {
        m.endpoint.clone()
    } else if m.provider == "ollama" {
        format!("{}/v1", settings.ollama_url.trim_end_matches('/'))
    } else if m.provider == "openrouter" {
        "https://openrouter.ai/api/v1".into()
    } else {
        "https://api.openai.com/v1".into()
    };
    if m.auth != "none" {
        headers.push(("authorization".into(), format!("Bearer {credential}")));
    }
    let mut messages = vec![json!({"role": "system", "content": sys})];
    messages.extend(turns.iter().map(|h| json!({"role": h["role"], "content": content(h, &m.provider)})));
    Ok(Request { url: format!("{base}/chat/completions"), headers, body: json!({"model": m.id, "stream": true, "messages": messages}) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings { show_thinking: true, effort: "medium".into(), ollama_url: "http://localhost:11434".into() }
    }
    fn turns() -> Vec<Value> {
        vec![json!({"role": "user", "text": "Test prompt"}), json!({"role": "assistant", "text": "Test reply"})]
    }
    fn model(provider: &str, extra: Value) -> Model {
        let mut raw = json!({"id": "user-chosen-model", "provider": provider});
        for (k, v) in extra.as_object().unwrap() {
            raw[k] = v.clone();
        }
        normalize(&raw)
    }
    fn request(m: &Model, opts: Opts, history: &[Value]) -> Result<Request, String> {
        build_request(m, history, &opts, &settings(), "fixture-secret")
    }

    #[test]
    fn keys_match_the_shells() {
        // (the seed's key, and the shell's own encoding: ':' and '/' escaped)
        let m = normalize(&json!({"id": "atlas-8b", "name": "Atlas 8B", "provider": "custom", "endpoint": "http://127.0.0.1:47311/v1", "auth": "none"}));
        assert_eq!(m.key, "custom|atlas-8b|http%3A%2F%2F127.0.0.1%3A47311%2Fv1|none||");
        assert_eq!(validate(&m), "");
        let a = model("aistudio", json!({}));
        let b = model("vertex", json!({}));
        let c = model("custom", json!({"endpoint": "https://one.example/v1"}));
        let d = model("custom", json!({"endpoint": "https://two.example/v1"}));
        let keys: std::collections::HashSet<_> = [&a.key, &b.key, &c.key, &d.key].into_iter().collect();
        assert_eq!(keys.len(), 4);
        assert_eq!(model("aistudio", json!({"name": "New label"})).key, a.key);
        assert_eq!(read(&json!([serde_json::to_string(&json!({"id": "x", "provider": "openai"})).unwrap(), {"id": "x", "provider": "openai"}, "bad json", null])).len(), 1);
    }

    #[test]
    fn ai_studio_streams_with_its_key_header_and_gemini_roles() {
        let m = model("aistudio", json!({"id": "models/user-chosen-model"}));
        let req = request(&m, Opts { max_tokens: Some(2048), ..Default::default() }, &turns()).unwrap();
        assert_eq!(req.url, "https://generativelanguage.googleapis.com/v1beta/models/user-chosen-model:streamGenerateContent?alt=sse");
        assert_eq!(req.header("x-goog-api-key"), Some("fixture-secret"));
        assert_eq!(req.body["contents"][1]["role"], "model");
        assert_eq!(req.body["generationConfig"]["maxOutputTokens"], 2048);
        assert_eq!(protocol(&m), "gemini");
    }

    #[test]
    fn vertex_express_and_oauth() {
        let m = model("vertex", json!({"project": "ignored-project", "location": "ignored-location"}));
        assert_eq!(validate(&m), "");
        assert_eq!(m.project, "");
        let req = request(&m, Opts::default(), &turns()).unwrap();
        assert_eq!(req.url, "https://aiplatform.googleapis.com/v1/publishers/google/models/user-chosen-model:streamGenerateContent?alt=sse");
        assert_eq!(req.header("x-goog-api-key"), Some("fixture-secret"));
        assert_eq!(req.header("authorization"), None);
        for location in ["global", "us-central1"] {
            let m = model("vertex", json!({"auth": "token", "project": "example-project", "location": location}));
            let req = request(&m, Opts::default(), &turns()).unwrap();
            let host = if location == "global" { String::new() } else { format!("{location}-") };
            assert_eq!(req.url, format!("https://{host}aiplatform.googleapis.com/v1/projects/example-project/locations/{location}/publishers/google/models/user-chosen-model:streamGenerateContent?alt=sse"));
            assert_eq!(req.header("authorization"), Some("Bearer fixture-secret"));
            assert_eq!(req.header("x-goog-api-key"), None);
            assert_eq!(credential_provider(&m), "vertex-oauth");
        }
        assert!(validate(&model("vertex", json!({"auth": "token"}))).contains("project"));
    }

    #[test]
    fn openai_compatible_endpoints() {
        let req = request(&model("openrouter", json!({"id": "publisher/model-selected-by-user"})), Opts::default(), &turns()).unwrap();
        assert_eq!(req.url, "https://openrouter.ai/api/v1/chat/completions");
        assert_eq!(req.body["model"], "publisher/model-selected-by-user");
        assert_eq!(req.header("authorization"), Some("Bearer fixture-secret"));
        assert_eq!(request(&model("openai", json!({})), Opts::default(), &turns()).unwrap().url, "https://api.openai.com/v1/chat/completions");
        let local = request(&model("ollama", json!({})), Opts::default(), &turns()).unwrap();
        assert_eq!(local.url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(local.header("authorization"), None);
        let custom = request(&model("custom", json!({"endpoint": "http://localhost:8000/v1/", "auth": "none"})), Opts { system: "Sys".into(), ..Default::default() }, &turns()).unwrap();
        assert_eq!(custom.url, "http://localhost:8000/v1/chat/completions");
        assert_eq!(custom.header("authorization"), None);
        // the whole body: system first, then the turns, streamed
        assert_eq!(
            custom.body,
            json!({"model": "user-chosen-model", "stream": true, "messages": [
                {"role": "system", "content": "Sys"}, {"role": "user", "content": "Test prompt"}, {"role": "assistant", "content": "Test reply"}]})
        );
    }

    #[test]
    fn anthropic_only_what_the_model_was_set_up_for() {
        let req = request(&model("anthropic", json!({})), Opts::default(), &turns()).unwrap();
        assert_eq!(req.body["max_tokens"], 4096);
        assert!(req.body.get("thinking").is_none() && req.body.get("output_config").is_none());
        assert_eq!(req.header("anthropic-beta"), None);
        assert_eq!(req.header("x-api-key"), Some("fixture-secret"));
        assert_eq!(req.header("anthropic-version"), Some("2026-10-21"));
        assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(req.body["stream"], true);
        let thinking = request(&model("anthropic", json!({"thinking": true, "maxTokens": 8192})), Opts::default(), &turns()).unwrap();
        assert_eq!(thinking.body["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(thinking.body["output_config"]["effort"], "medium");
        assert_eq!(thinking.body["max_tokens"], 8192);
        let quick = request(&model("anthropic", json!({"thinking": true})), Opts { thinking: Some(false), max_tokens: Some(2048), ..Default::default() }, &turns()).unwrap();
        assert!(quick.body.get("thinking").is_none());
        assert_eq!(quick.body["max_tokens"], 2048);
        // (Claude's blocks go back unchanged)
        let blocks = json!([{"type": "thinking", "thinking": "t", "signature": "s"}, {"type": "text", "text": "a"}]);
        let h = vec![json!({"role": "user", "text": "q"}), json!({"role": "assistant", "text": "a", "blocks": blocks})];
        assert_eq!(request(&model("anthropic", json!({})), Opts::default(), &h).unwrap().body["messages"][1]["content"], blocks);
    }

    #[test]
    fn pictures_in_every_wire_format() {
        let history = vec![json!({"role": "user", "text": "Describe", "image": {"mediaType": "image/png", "data": "fixture-base64"}})];
        assert_eq!(request(&model("aistudio", json!({})), Opts::default(), &history).unwrap().body["contents"][0]["parts"][0]["inline_data"]["data"], "fixture-base64");
        assert_eq!(request(&model("anthropic", json!({})), Opts::default(), &history).unwrap().body["messages"][0]["content"][0]["source"]["data"], "fixture-base64");
        assert_eq!(request(&model("openrouter", json!({})), Opts::default(), &history).unwrap().body["messages"][1]["content"][0]["image_url"]["url"], "data:image/png;base64,fixture-base64");
        let pics = json!([{"data": "AAA", "mediaType": "image/jpeg"}, {"data": "BBB", "mediaType": "image/png"}]);
        let h = vec![json!({"role": "user", "text": "Compare these", "images": pics})];
        let claude = request(&model("anthropic", json!({})), Opts::default(), &h).unwrap().body["messages"][0]["content"].clone();
        assert_eq!(claude.as_array().unwrap().iter().map(|c| c["type"].as_str().unwrap()).collect::<Vec<_>>(), ["image", "image", "text"]);
        let openai = request(&model("openai", json!({})), Opts::default(), &h).unwrap().body["messages"][1]["content"].clone();
        assert_eq!(openai.as_array().unwrap().iter().map(|c| c["type"].as_str().unwrap()).collect::<Vec<_>>(), ["image_url", "image_url", "text"]);
        let gemini = request(&model("aistudio", json!({})), Opts::default(), &h).unwrap().body["contents"][0]["parts"].clone();
        assert_eq!(gemini.as_array().unwrap().iter().filter(|p| p.get("inline_data").is_some()).count(), 2);
    }

    #[test]
    fn keys_only_travel_over_plain_http_to_this_computer() {
        for ep in ["http://localhost:8000/v1", "http://127.0.0.1:8000/v1", "http://[::1]:8000/v1"] {
            assert_eq!(validate(&model("custom", json!({"endpoint": ep}))), "", "{ep}");
        }
        assert!(validate(&model("custom", json!({"endpoint": "http://192.168.1.5:8000/v1"}))).contains("https://"));
        assert!(validate(&model("custom", json!({"endpoint": "http://localhost.evil.example/v1"}))).contains("https://"));
        assert_eq!(validate(&model("custom", json!({"endpoint": "http://192.168.1.5:8000/v1", "auth": "none"}))), "");
    }

    #[test]
    fn invalid_records_are_rejected_before_a_request() {
        for raw in [
            json!({"provider": "aistudio", "id": ""}),
            json!({"provider": "not-supported", "id": "test"}),
            json!({"provider": "vertex", "id": "bad/path"}),
            json!({"provider": "custom", "id": "test", "endpoint": ""}),
            json!({"provider": "custom", "id": "test", "endpoint": "https://user:pass@host/v1"}),
            json!({"provider": "custom", "id": "test", "endpoint": "https://host/v1?key=secret"}),
            json!({"provider": "custom", "id": "test", "endpoint": "file:///tmp/model"}),
            json!({"provider": "custom", "id": "test", "endpoint": "http://models.example/v1"}),
            json!({"provider": "anthropic", "id": "test", "maxTokens": "NaN"}),
            json!({"provider": "vertex", "id": "test", "auth": "token", "project": "p", "location": "../bad"}),
        ] {
            let m = normalize(&raw);
            assert_ne!(validate(&m), "", "{raw}");
            assert!(request(&m, Opts::default(), &turns()).is_err());
        }
        assert!(read(&json!(["bad json", null, {"provider": "unknown", "id": "test"}])).is_empty());
    }

    #[test]
    fn which_models_read_pictures() {
        assert!(guess_vision("anthropic", "anything"));
        assert!(guess_vision("aistudio", "gemini-2.5-flash"));
        assert!(guess_vision("openai", "gpt-4o-mini"));
        assert!(!guess_vision("openai", "gpt-3.5-turbo"));
        assert!(!guess_vision("ollama", "llama3.1:8b"));
        assert!(guess_vision("ollama", "qwen2.5vl:7b"));
        assert!(guess_vision("ollama", "llava:13b"));
        assert!(guess_vision("openai", "o3"));
        assert!(!guess_vision("openai", "foo3"));
        assert!(guess_vision("custom", "some_vl"));
        assert!(model("ollama", json!({"vision": true})).vision);
        assert!(!model("anthropic", json!({"vision": false})).vision);
    }

    #[test]
    fn debug_never_shows_the_key() {
        let req = request(&model("openai", json!({})), Opts::default(), &turns()).unwrap();
        assert!(!format!("{req:?}").contains("fixture-secret"));
    }
}
