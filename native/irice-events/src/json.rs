//! D-Bus values as JSON, for signal arguments: variants unwrapped, dicts as
//! objects, structs and arrays as lists, numbers as numbers.

use serde_json::{Map, Value as Json};
use zbus::zvariant::Value;

pub fn from_dbus(v: &Value) -> Json {
    match v {
        Value::U8(x) => Json::from(*x),
        Value::Bool(x) => Json::from(*x),
        Value::I16(x) => Json::from(*x),
        Value::U16(x) => Json::from(*x),
        Value::I32(x) => Json::from(*x),
        Value::U32(x) => Json::from(*x),
        Value::I64(x) => Json::from(*x),
        Value::U64(x) => Json::from(*x),
        Value::F64(x) => serde_json::Number::from_f64(*x).map(Json::Number).unwrap_or(Json::Null),
        Value::Str(s) => Json::from(s.as_str()),
        Value::Signature(s) => Json::from(s.to_string()),
        Value::ObjectPath(p) => Json::from(p.as_str()),
        Value::Value(inner) => from_dbus(inner),
        Value::Array(a) => Json::Array(a.inner().iter().map(from_dbus).collect()),
        Value::Dict(d) => {
            let mut m = Map::new();
            for (k, v) in d.iter() {
                let key = match from_dbus(k) {
                    Json::String(s) => s,
                    other => other.to_string(),
                };
                m.insert(key, from_dbus(v));
            }
            Json::Object(m)
        }
        Value::Structure(s) => Json::Array(s.fields().iter().map(from_dbus).collect()),
        #[allow(unreachable_patterns)]
        _ => Json::Null, // (file descriptors)
    }
}

/// A message body's arguments, as a list.
pub fn args(msg: &zbus::Message) -> Json {
    let body = msg.body();
    if body.is_empty() {
        return Json::Array(vec![]);
    }
    match body.deserialize::<zbus::zvariant::Structure>() {
        Ok(s) => Json::Array(s.fields().iter().map(from_dbus).collect()),
        Err(_) => Json::Array(vec![]),
    }
}

/// Avahi's TXT records (a list of "key=value" byte strings) as an object.
pub fn txt(records: &[Vec<u8>]) -> Json {
    let mut m = Map::new();
    for r in records {
        let s = String::from_utf8_lossy(r);
        if let Some((k, v)) = s.split_once('=') {
            if !k.is_empty() && !m.contains_key(k) {
                m.insert(k.to_string(), Json::from(v));
            }
        } else if !s.is_empty() {
            m.entry(s.to_string()).or_insert(Json::Bool(true));
        }
    }
    Json::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use zbus::zvariant::{ObjectPath, Structure};

    #[test]
    fn values() {
        assert_eq!(from_dbus(&Value::Bool(true)), json!(true));
        assert_eq!(from_dbus(&Value::I64(3)), json!(3));
        assert_eq!(from_dbus(&Value::U32(7)), json!(7));
        assert_eq!(from_dbus(&Value::F64(f64::NAN)), Json::Null);
        assert_eq!(from_dbus(&Value::from("x")), json!("x"));
        assert_eq!(from_dbus(&Value::from(ObjectPath::try_from("/a/b").unwrap())), json!("/a/b"));
        // a variant inside a variant
        assert_eq!(from_dbus(&Value::Value(Box::new(Value::Value(Box::new(Value::U8(1)))))), json!(1));
        assert_eq!(from_dbus(&Value::from(vec![1u16, 2])), json!([1, 2]));
        let s = Structure::from((1i32, "two"));
        assert_eq!(from_dbus(&Value::Structure(s)), json!([1, "two"]));
    }

    #[test]
    fn launcher_entry_props() {
        // Update's a{sv}: {"count": <int64 3>, "count-visible": <true>}
        let mut props: HashMap<&str, Value> = HashMap::new();
        props.insert("count", Value::Value(Box::new(Value::I64(3))));
        props.insert("count-visible", Value::Value(Box::new(Value::Bool(true))));
        assert_eq!(from_dbus(&Value::from(props)), json!({"count": 3, "count-visible": true}));
        // non-string keys become their JSON text
        let mut m: HashMap<u32, &str> = HashMap::new();
        m.insert(5, "five");
        assert_eq!(from_dbus(&Value::from(m)), json!({"5": "five"}));
    }

    #[test]
    fn txt_records() {
        let recs: Vec<Vec<u8>> = vec![b"id=abc".to_vec(), b"fn=Living Room TV".to_vec(), b"md=Chromecast".to_vec(),
                                      b"flag".to_vec(), b"id=second".to_vec(), b"ve=05=x".to_vec()];
        assert_eq!(txt(&recs), json!({"id": "abc", "fn": "Living Room TV", "md": "Chromecast", "flag": true, "ve": "05=x"}));
    }
}
