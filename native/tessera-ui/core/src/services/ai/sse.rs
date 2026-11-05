//! The streamed answer, as services/SseParser.qml reads it: server-sent
//! events in the Claude, Gemini and OpenAI-compatible formats. Bytes go in as
//! they arrive; complete lines are read, the rest waits for the next chunk.

use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct Parser {
    /// anthropic, gemini, or any other (OpenAI-compatible)
    pub provider: String,
    pub text: String,
    pub thinking: String,
    /// Claude's content blocks (echoed back in the next request)
    pub blocks: Vec<Value>,
    pub stop_reason: String,
    pub error: String,
    pending: Vec<u8>,
}

impl Parser {
    pub fn new(provider: &str) -> Self {
        Parser { provider: provider.to_string(), ..Default::default() }
    }

    /// A chunk of the response (any size, lines may be split); true when
    /// it held a complete line
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.pending.extend_from_slice(bytes);
        let Some(end) = self.pending.iter().rposition(|&b| b == b'\n') else { return false };
        let chunk: Vec<u8> = self.pending.drain(..=end).collect();
        for line in String::from_utf8_lossy(&chunk).split('\n') {
            self.line(line);
        }
        true
    }

    /// One line of the stream
    pub fn line(&mut self, line: &str) {
        let Some(data) = line.strip_prefix("data:") else { return };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            return;
        }
        if let Ok(ev) = serde_json::from_str::<Value>(data) {
            self.handle(&ev);
        }
    }

    fn handle(&mut self, ev: &Value) {
        if self.provider != "anthropic" && truthy(&ev["error"]) {
            self.error = ev["error"]["message"].as_str().unwrap_or("The response was interrupted. Try again.").to_string();
            return;
        }
        match self.provider.as_str() {
            "anthropic" => match ev["type"].as_str().unwrap_or("") {
                "content_block_start" => self.blocks.push(ev["content_block"].clone()),
                "content_block_delta" => {
                    let i = ev["index"].as_u64().map(|i| i as usize).filter(|&i| i < self.blocks.len()).or(self.blocks.len().checked_sub(1));
                    let d = &ev["delta"];
                    let add = |b: Option<&mut Value>, key: &str, s: &str| {
                        if let Some(Value::Object(o)) = b {
                            let old = o.get(key).and_then(Value::as_str).unwrap_or("").to_string();
                            o.insert(key.into(), Value::String(old + s));
                        }
                    };
                    match d["type"].as_str().unwrap_or("") {
                        "text_delta" => {
                            let s = d["text"].as_str().unwrap_or("");
                            add(i.and_then(|i| self.blocks.get_mut(i)), "text", s);
                            self.text += s;
                        }
                        "thinking_delta" => {
                            let s = d["thinking"].as_str().unwrap_or("");
                            add(i.and_then(|i| self.blocks.get_mut(i)), "thinking", s);
                            self.thinking += s;
                        }
                        "signature_delta" => {
                            if let Some(Value::Object(o)) = i.and_then(|i| self.blocks.get_mut(i)) {
                                o.insert("signature".into(), d["signature"].clone());
                            }
                        }
                        _ => {}
                    }
                }
                "message_delta" => {
                    if let Some(r) = ev["delta"]["stop_reason"].as_str().filter(|r| !r.is_empty()) {
                        self.stop_reason = r.to_string();
                    }
                }
                "error" => {
                    self.error = if ev["error"]["type"] == "overloaded_error" {
                        "Claude is overloaded right now. Try again in a moment.".into()
                    } else {
                        ev["error"]["message"].as_str().unwrap_or("The response was interrupted. Try again.").to_string()
                    }
                }
                _ => {}
            },
            "gemini" => {
                let c = &ev["candidates"][0];
                for part in c["content"]["parts"].as_array().into_iter().flatten() {
                    let s = part["text"].as_str().unwrap_or("");
                    if truthy(&part["thought"]) {
                        self.thinking += s;
                    } else {
                        self.text += s;
                    }
                }
                if c["finishReason"] == "SAFETY" {
                    self.stop_reason = "refusal".into();
                }
            }
            _ => {
                let d = &ev["choices"][0]["delta"];
                self.text += d["content"].as_str().unwrap_or("");
                self.thinking += d["reasoning"].as_str().or_else(|| d["reasoning_content"].as_str()).unwrap_or("");
            }
        }
    }

    /// Claude: after a mid-output fallback, blocks other than text before the
    /// last fallback marker aren't echoed back (Ai.qml's echoBlocks)
    pub fn echo_blocks(&self) -> Vec<Value> {
        let last = self.blocks.iter().rposition(|b| b["type"] == "fallback");
        self.blocks
            .iter()
            .enumerate()
            .filter(|(i, b)| last.is_none_or(|l| *i >= l) || b["type"] == "text" || b["type"] == "fallback")
            .map(|(_, b)| b.clone())
            .collect()
    }
}

/// JS truthiness for a JSON value
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stream(provider: &str, text: &str, chunk: usize) -> Parser {
        let mut p = Parser::new(provider);
        for c in text.as_bytes().chunks(chunk) {
            p.feed(c);
        }
        p
    }

    #[test]
    fn openai_compatible_deltas_any_chunking() {
        let body = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n: keep-alive\n\ndata: {\"choices\":[{\"delta\":{\"reasoning\":\"hmm \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo ✓\",\"reasoning_content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
        for chunk in [1, 3, 7, 1000] {
            let p = stream("custom", body, chunk);
            assert_eq!(p.text, "Hello ✓", "chunk {chunk}");
            assert_eq!(p.thinking, "hmm ok");
            assert_eq!(p.error, "");
        }
        // (a line still coming waits)
        let mut p = Parser::new("openai");
        assert!(!p.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}"));
        assert_eq!(p.text, "");
        p.feed(b"\n");
        assert_eq!(p.text, "x");
        let p = stream("openrouter", "data: {\"error\":{\"message\":\"Upstream died\"}}\n", 64);
        assert_eq!(p.error, "Upstream died");
    }

    #[test]
    fn claude_blocks_thinking_and_refusals() {
        let events = [
            json!({"type": "message_start", "message": {}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "Let me see"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Hi "}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "there"}}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}}),
        ];
        let body: String = events.iter().map(|e| format!("event: x\ndata: {e}\n\n")).collect();
        let p = stream("anthropic", &body, 5);
        assert_eq!(p.text, "Hi there");
        assert_eq!(p.thinking, "Let me see");
        assert_eq!(p.stop_reason, "end_turn");
        assert_eq!(p.blocks, vec![json!({"type": "thinking", "thinking": "Let me see", "signature": "sig"}), json!({"type": "text", "text": "Hi there"})]);
        let p = stream("anthropic", "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"x\"}}\n", 9);
        assert_eq!(p.error, "Claude is overloaded right now. Try again in a moment.");
        let p = stream("anthropic", "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\"}}\n", 9);
        assert_eq!(p.stop_reason, "refusal");
    }

    #[test]
    fn claude_echoes_blocks_after_the_last_fallback() {
        let mut p = Parser::new("anthropic");
        p.blocks = vec![json!({"type": "thinking"}), json!({"type": "text", "text": "a"}), json!({"type": "fallback"}), json!({"type": "thinking"}), json!({"type": "text", "text": "b"})];
        let kinds: Vec<_> = p.echo_blocks().iter().map(|b| b["type"].as_str().unwrap().to_string()).collect();
        assert_eq!(kinds, ["text", "fallback", "thinking", "text"]);
    }

    #[test]
    fn gemini_text_thoughts_and_safety() {
        let body = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"plan\",\"thought\":true},{\"text\":\"Lis\"}]}}]}\r\n\r\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"bon\"}]},\"finishReason\":\"STOP\"}]}\r\n\r\n";
        let p = stream("gemini", body, 11);
        assert_eq!(p.text, "Lisbon");
        assert_eq!(p.thinking, "plan");
        assert_eq!(p.stop_reason, "");
        let p = stream("gemini", "data: {\"candidates\":[{\"finishReason\":\"SAFETY\"}]}\n", 4);
        assert_eq!(p.stop_reason, "refusal");
    }
}
