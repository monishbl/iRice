//! The shell's commands, one JSON object per line on stdin.

use serde_json::Value;

pub const SOURCES: &[&str] = &[
    "login1", "unity", "nm", "net", "avahi", "location", "udev", "pulse", "camera", "clipboard", "copies", "screenshare",
];

// What a logind inhibitor may hold (logind's own names; "a:b" holds both)
const INHIBIT: &[&str] = &[
    "idle", "sleep", "shutdown", "handle-lid-switch", "handle-power-key", "handle-suspend-key", "handle-hibernate-key",
];

#[derive(Debug, PartialEq)]
pub enum Command {
    Watch { src: String, on: bool, args: Value },
    Inhibit { what: String, why: String, on: bool },
}

/// One line: Ok(None) for a blank one, Err(why) for one that's not a command.
pub fn parse(line: &str) -> Result<Option<Command>, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let v: Value = serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))?;
    let o = v.as_object().ok_or("not a JSON object")?;
    let on = match o.get("on") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err("\"on\" must be true or false".into()),
    };
    match o.get("cmd").and_then(Value::as_str) {
        Some("watch") => {
            let src = o.get("src").and_then(Value::as_str).ok_or("watch needs \"src\"")?;
            if !SOURCES.contains(&src) {
                return Err(format!("unknown source {src}"));
            }
            let args = match o.get("args") {
                None | Some(Value::Null) => Value::Object(Default::default()),
                Some(a @ Value::Object(_)) => a.clone(),
                Some(_) => return Err("\"args\" must be an object".into()),
            };
            Ok(Some(Command::Watch { src: src.into(), on, args }))
        }
        Some("inhibit") => {
            let what = o.get("what").and_then(Value::as_str).ok_or("inhibit needs \"what\"")?;
            if what.is_empty() || !what.split(':').all(|w| INHIBIT.contains(&w)) {
                return Err(format!("can't inhibit {what}"));
            }
            let why = o.get("why").and_then(Value::as_str).unwrap_or("");
            let why: String = why.chars().filter(|c| !c.is_control()).take(200).collect();
            Ok(Some(Command::Inhibit { what: what.into(), why, on }))
        }
        Some(c) => Err(format!("unknown command {c}")),
        None => Err("no \"cmd\"".into()),
    }
}

/// A string list in a watch's args (e.g. udev's "subsystems").
pub fn strings(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn watch() {
        assert_eq!(
            parse(r#"{"cmd":"watch","src":"udev","on":true,"args":{"subsystems":["usb","block"]}}"#),
            Ok(Some(Command::Watch { src: "udev".into(), on: true, args: json!({"subsystems": ["usb", "block"]}) }))
        );
        // "on" defaults to true, args to {}
        assert_eq!(
            parse(r#"{"cmd":"watch","src":"pulse"}"#),
            Ok(Some(Command::Watch { src: "pulse".into(), on: true, args: json!({}) }))
        );
        assert_eq!(
            parse(r#" {"cmd":"watch","src":"avahi","on":false} "#),
            Ok(Some(Command::Watch { src: "avahi".into(), on: false, args: json!({}) }))
        );
        assert_eq!(strings(&json!({"subsystems": ["usb", 3, "input"]}), "subsystems"), vec!["usb", "input"]);
        assert!(strings(&json!({}), "subsystems").is_empty());
    }

    #[test]
    fn inhibit() {
        assert_eq!(
            parse(r#"{"cmd":"inhibit","what":"idle","why":"Caffeine\n","on":true}"#),
            Ok(Some(Command::Inhibit { what: "idle".into(), why: "Caffeine".into(), on: true }))
        );
        assert_eq!(
            parse(r#"{"cmd":"inhibit","what":"handle-lid-switch","on":false}"#),
            Ok(Some(Command::Inhibit { what: "handle-lid-switch".into(), why: "".into(), on: false }))
        );
        assert!(parse(r#"{"cmd":"inhibit","what":"idle:sleep"}"#).is_ok());
        assert!(parse(r#"{"cmd":"inhibit","what":"everything"}"#).is_err());
        assert!(parse(r#"{"cmd":"inhibit","what":""}"#).is_err());
        assert!(parse(r#"{"cmd":"inhibit"}"#).is_err());
    }

    #[test]
    fn bad_lines() {
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("   "), Ok(None));
        assert!(parse("watch udev").is_err());
        assert!(parse("[1,2]").is_err());
        assert!(parse(r#"{"src":"udev"}"#).is_err());
        assert!(parse(r#"{"cmd":"launch","src":"udev"}"#).is_err());
        assert!(parse(r#"{"cmd":"watch","src":"keylogger"}"#).is_err());
        assert!(parse(r#"{"cmd":"watch","src":"udev","on":"yes"}"#).is_err());
        assert!(parse(r#"{"cmd":"watch","src":"udev","args":[1]}"#).is_err());
    }
}
