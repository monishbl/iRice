//! Incremental SSE parser for the Claude, Gemini and OpenAI-compatible stream
//! formats (shell/services/SseParser.qml). `feed` takes the whole response so
//! far and returns true when something new was parsed (the QML's `updated`
//! signal). It is a pure helper: the AI service itself (core/src/services/ai)
//! has its own streaming; this port is here for the host's other services and
//! to keep the QML file accounted for.

use serde_json::Value;

#[derive(Default, Debug)]
pub struct SseParser {
    pub provider: String,
    /// bytes of the response already taken
    pub offset: usize,
    pub text: String,
    pub thinking: String,
    pub blocks: Vec<Value>,
    pub stop_reason: String,
    pub error: String,
}

impl SseParser {
    pub fn reset(&mut self, provider: &str) {
        *self = SseParser { provider: provider.into(), ..Default::default() };
    }

    /// Whole lines only: a half line waits for the rest
    pub fn feed(&mut self, all: &str) -> bool {
        let Some(end) = all.rfind('\n') else { return false };
        if end < self.offset {
            return false;
        }
        let chunk = all[self.offset..end].to_string();
        self.offset = end + 1;
        for line in chunk.split('\n') {
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(data) else { continue };
            self.handle(&ev);
        }
        true
    }

    pub fn handle(&mut self, ev: &Value) {
        if self.provider != "anthropic" && !ev["error"].is_null() {
            self.error = ev["error"]["message"].as_str().unwrap_or("The response was interrupted. Try again.").into();
            return;
        }
        if self.provider == "anthropic" {
            match ev["type"].as_str().unwrap_or("") {
                "content_block_start" => self.blocks.push(ev["content_block"].clone()),
                "content_block_delta" => {
                    let idx = ev["index"].as_u64().map(|i| i as usize).filter(|i| *i < self.blocks.len()).or(self.blocks.len().checked_sub(1));
                    let Some(i) = idx else { return };
                    let d = &ev["delta"];
                    match d["type"].as_str().unwrap_or("") {
                        "text_delta" => {
                            let add = d["text"].as_str().unwrap_or("");
                            let cur = self.blocks[i]["text"].as_str().unwrap_or("").to_string();
                            self.blocks[i]["text"] = Value::String(cur + add);
                            self.text.push_str(add);
                        }
                        "thinking_delta" => {
                            let add = d["thinking"].as_str().unwrap_or("");
                            let cur = self.blocks[i]["thinking"].as_str().unwrap_or("").to_string();
                            self.blocks[i]["thinking"] = Value::String(cur + add);
                            self.thinking.push_str(add);
                        }
                        "signature_delta" => self.blocks[i]["signature"] = d["signature"].clone(),
                        _ => {}
                    }
                }
                "message_delta" => {
                    if let Some(s) = ev["delta"]["stop_reason"].as_str().filter(|s| !s.is_empty()) {
                        self.stop_reason = s.into();
                    }
                }
                "error" => {
                    self.error = if ev["error"]["type"] == "overloaded_error" {
                        "Claude is overloaded right now. Try again in a moment.".into()
                    } else {
                        ev["error"]["message"].as_str().unwrap_or("The response was interrupted. Try again.").into()
                    }
                }
                _ => {}
            }
        } else if self.provider == "gemini" {
            for part in ev["candidates"][0]["content"]["parts"].as_array().cloned().unwrap_or_default() {
                let t = part["text"].as_str().unwrap_or("");
                if part["thought"].as_bool().unwrap_or(false) {
                    self.thinking.push_str(t);
                } else {
                    self.text.push_str(t);
                }
            }
            if ev["candidates"][0]["finishReason"] == "SAFETY" {
                self.stop_reason = "refusal".into();
            }
        } else {
            let d = &ev["choices"][0]["delta"];
            self.text.push_str(d["content"].as_str().unwrap_or(""));
            let r = d["reasoning"].as_str().or_else(|| d["reasoning_content"].as_str()).unwrap_or("");
            self.thinking.push_str(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_half_line_waits_for_the_rest() {
        let mut p = SseParser::default();
        p.reset("openai");
        assert!(!p.feed("data: {\"choices\":[{\"delta\":{\"content\":\"Hel"));
        let whole = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n";
        assert!(p.feed(whole));
        assert_eq!(p.text, "Hello");
        // the same response again, longer: only what's new is taken
        let more = format!("{whole}data: [DONE]\n");
        p.feed(&more);
        assert_eq!(p.text, "Hello");
    }

    #[test]
    fn claude_blocks_collect_text_and_thinking() {
        let mut p = SseParser::default();
        p.reset("anthropic");
        let s = "data: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
                 data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\
                 data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n";
        p.feed(s);
        assert_eq!(p.text, "Hi");
        assert_eq!(p.blocks[0]["text"], "Hi");
        assert_eq!(p.stop_reason, "end_turn");
    }

    #[test]
    fn errors_and_refusals_are_said() {
        let mut p = SseParser::default();
        p.reset("anthropic");
        p.feed("data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n");
        assert!(p.error.contains("overloaded"));
        let mut g = SseParser::default();
        g.reset("gemini");
        g.feed("data: {\"candidates\":[{\"finishReason\":\"SAFETY\",\"content\":{\"parts\":[{\"text\":\"x\",\"thought\":true}]}}]}\n");
        assert_eq!(g.stop_reason, "refusal");
        assert_eq!(g.thinking, "x");
    }
}
