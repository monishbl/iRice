//! What one service of this crate lets the others read (the QML services
//! read each other as singletons: Power.percent, Brightness.value…). A
//! service `put`s its latest state as JSON under its name; another `get`s it.
//! Actions go the other way through `Host::ask("<prefix>.<fn>", args)` to the
//! service whose `calls()` has that prefix (internal prefixes are fine: the
//! host logs a call nobody answers).

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn map() -> &'static Mutex<HashMap<&'static str, Value>> {
    static M: OnceLock<Mutex<HashMap<&'static str, Value>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Publish a service's state (whole value replaced)
pub fn put(name: &'static str, v: Value) {
    if let Ok(mut m) = map().lock() {
        if m.get(name) != Some(&v) {
            m.insert(name, v);
            GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Bumped by every `put` that changed something (a reader that mirrors the
/// bus can skip a turn in which nothing changed)
pub fn generation() -> u64 {
    GEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// A service's last published state (Null if none yet)
pub fn get(name: &str) -> Value {
    map().lock().ok().and_then(|m| m.get(name).cloned()).unwrap_or(Value::Null)
}

/// One field of it: `field("power", "percent")`
pub fn field(name: &str, key: &str) -> Value {
    // (the field alone: cloning the whole state for it was most of the
    // shell's idle time, read after every turn)
    map().lock().ok().and_then(|m| m.get(name).map(|v| v[key].clone())).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn put_then_get() {
        put("t", serde_json::json!({"a": 1}));
        assert_eq!(field("t", "a"), 1);
        assert!(get("nothing").is_null());
    }
}
