//! Nearby devices from both protocols as one list (shell/services/
//! SharingDevices.js): a phone running LocalSend that also has Quick Share
//! open shows up twice, once from each; it is one device to you, so the two
//! are merged by name. Each entry keeps how to reach it both ways: Android
//! phones go over Quick Share (no app to open on the phone), everything else
//! over LocalSend.

use serde_json::{json, Value};

/// "Pixel 8", "pixel-8 " and "PIXEL 8" are the same device (spaces and
/// punctuation out, lower case)
pub fn name_key(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| !c.is_whitespace() && !"-_.,:;'\u{2019}\"()[]".contains(*c)).collect()
}

/// Quick Share says "phone"; LocalSend "mobile". The sheet's icons use LocalSend's words
fn type_of(t: &str) -> String {
    if t == "phone" {
        "mobile".into()
    } else if t.is_empty() {
        "desktop".into()
    } else {
        t.into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub via: Vec<String>,
    pub quickshare: Option<String>,
    pub localsend: Option<String>,
    pub prefer: String,
}

/// merge(LocalSend.peers, QuickShare.devices); each is (name, type, id)
pub fn merge(localsend: &[(String, String, String)], quickshare: &[(String, String, String)]) -> Vec<Device> {
    let mut out: Vec<Device> = Vec::new();
    let mut add = |name: &str, kind: &str, via: &str, id: &str| {
        let k = {
            let k = name_key(name);
            if k.is_empty() {
                format!("{via}:{id}")
            } else {
                k
            }
        };
        let at = match out.iter().position(|d| d.key == k) {
            Some(i) => i,
            None => {
                let n = name.trim();
                out.push(Device {
                    key: k,
                    name: if n.is_empty() { "Unknown device".into() } else { n.into() },
                    kind: type_of(kind),
                    via: vec![],
                    quickshare: None,
                    localsend: None,
                    prefer: String::new(),
                });
                out.len() - 1
            }
        };
        let d = &mut out[at];
        if !d.via.iter().any(|v| v == via) {
            d.via.push(via.into());
        }
        if via == "quickshare" {
            d.quickshare = Some(id.into());
        } else {
            d.localsend = Some(id.into());
        }
        // A more specific kind wins (a phone over a generic desktop)
        if d.kind == "desktop" && !kind.is_empty() {
            d.kind = type_of(kind);
        }
    };
    for (n, t, id) in localsend {
        add(n, t, "localsend", id);
    }
    for (n, t, id) in quickshare {
        add(n, t, "quickshare", id);
    }
    for d in out.iter_mut() {
        d.prefer = if d.quickshare.is_some() && (d.kind == "mobile" || d.kind == "tablet" || d.localsend.is_none()) { "quickshare" } else { "localsend" }.into();
    }
    out
}

/// The send in flight to this device, if any (LocalSend and Quick Share both
/// name an outgoing transfer's peer): the transfer item
pub fn progress_for<'a>(items: &'a [Value], device_name: &str) -> Option<&'a Value> {
    let k = name_key(device_name);
    items.iter().find(|it| it["kind"] == "send" && name_key(it["peer"].as_str().unwrap_or("")) == k)
}

/// The three visibility levels, as one: what both say, or the more open one
/// when they differ (the module then says "Custom")
pub fn combine(a: &str, b: &str) -> String {
    if a == b {
        return a.into();
    }
    if a == "everyone" || b == "everyone" {
        return "everyone".into();
    }
    if a == "10min" || b == "10min" {
        return "10min".into();
    }
    "off".into()
}

/// The merged list as the sheet and the pages read it
pub fn to_json(list: &[Device]) -> Value {
    Value::Array(
        list.iter()
            .map(|d| json!({"key": d.key, "name": d.name, "type": d.kind, "via": d.via, "ids": {"quickshare": d.quickshare, "localsend": d.localsend}, "prefer": d.prefer}))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(n: &str, ty: &str, id: &str) -> (String, String, String) {
        (n.into(), ty.into(), id.into())
    }

    #[test]
    fn names_that_differ_in_punctuation_are_one_device() {
        assert_eq!(name_key("Pixel 8"), name_key("pixel-8 "));
        assert_eq!(name_key("PIXEL 8"), "pixel8");
    }

    #[test]
    fn a_phone_seen_both_ways_is_one_device_on_quick_share() {
        let m = merge(&[t("Kai's Pixel", "mobile", "l1")], &[t("Kai's Pixel", "phone", "q1")]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].prefer, "quickshare");
        assert_eq!(m[0].kind, "mobile");
        assert_eq!(m[0].via, vec!["localsend", "quickshare"]);
    }

    #[test]
    fn computers_go_over_localsend() {
        let m = merge(&[t("Ada's laptop", "laptop", "l1")], &[]);
        assert_eq!(m[0].prefer, "localsend");
        let both = merge(&[t("Desk", "desktop", "l2")], &[t("Desk", "", "q2")]);
        assert_eq!(both[0].prefer, "localsend");
    }

    #[test]
    fn a_quick_share_only_device_prefers_it() {
        let m = merge(&[], &[t("", "tablet", "q3")]);
        assert_eq!(m[0].name, "Unknown device");
        assert_eq!(m[0].prefer, "quickshare");
        assert_eq!(m[0].key, "quickshare:q3");
    }

    #[test]
    fn levels_combine_to_the_more_open() {
        assert_eq!(combine("off", "off"), "off");
        assert_eq!(combine("10min", "everyone"), "everyone");
        assert_eq!(combine("off", "10min"), "10min");
    }

    #[test]
    fn progress_is_found_by_name() {
        let items = vec![json!({"kind": "send", "peer": "Ada's Laptop", "done": 1, "total": 2})];
        assert!(progress_for(&items, "ada's laptop").is_some());
        assert!(progress_for(&items, "other").is_none());
    }
}
