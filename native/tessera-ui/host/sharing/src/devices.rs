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

/// merge(LocalSend.peers, QuickShare.devices); each is (name, type, id).
/// Merged by name only across the two protocols: an entry holds at most one
/// device of each, so two LocalSend peers (or two phones) with the same name
/// stay two entries, the later one keyed by its id and named "Pixel 8 (2)"
/// (a send never goes to whichever came last)
pub fn merge(localsend: &[(String, String, String)], quickshare: &[(String, String, String)]) -> Vec<Device> {
    let mut out: Vec<Device> = Vec::new();
    // each entry's name key (its `key` unless it is a same-name second)
    let mut bases: Vec<String> = Vec::new();
    let mut add = |name: &str, kind: &str, via: &str, id: &str| {
        let k = {
            let k = name_key(name);
            if k.is_empty() {
                format!("{via}:{id}")
            } else {
                k
            }
        };
        // (this protocol's place in the entry is free, or this very device's)
        let fits = |d: &Device| {
            let slot = if via == "quickshare" { &d.quickshare } else { &d.localsend };
            slot.as_deref().is_none_or(|x| x == id)
        };
        let at = match (0..out.len()).find(|&i| bases[i] == k && fits(&out[i])) {
            Some(i) => i,
            None => {
                let n = name.trim();
                let n = if n.is_empty() { "Unknown device".to_string() } else { n.to_string() };
                let same = bases.iter().filter(|b| **b == k).count();
                let (key, n) = if same == 0 { (k.clone(), n) } else { (format!("{via}:{id}"), format!("{n} ({})", same + 1)) };
                bases.push(k);
                out.push(Device {
                    key,
                    name: n,
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
    fn phones_and_computers_show_together_in_one_list() {
        // what the send window gets: LocalSend's computers, Quick Share's
        // phones, and one phone that runs both (shown once, sent to over Quick Share)
        let ls = [t("Ada's laptop", "laptop", "fp1"), t("Studio desktop", "desktop", "fp2"), t("Kai's Pixel", "mobile", "fp3")];
        let qs = [t("Kai's Pixel", "phone", "Q1"), t("Ines's tablet", "tablet", "Q2")];
        let m = merge(&ls, &qs);
        let names: Vec<(&str, &str)> = m.iter().map(|d| (d.name.as_str(), d.prefer.as_str())).collect();
        assert_eq!(names, [("Ada's laptop", "localsend"), ("Studio desktop", "localsend"), ("Kai's Pixel", "quickshare"), ("Ines's tablet", "quickshare")]);
        assert_eq!(m[2].localsend.as_deref(), Some("fp3"));
        assert_eq!(m[2].quickshare.as_deref(), Some("Q1"));
        // a device found later is added live, the others keep their place
        let more = merge(&ls, &[t("Kai's Pixel", "phone", "Q1"), t("Ines's tablet", "tablet", "Q2"), t("Moto edge", "phone", "Q3")]);
        assert_eq!(more.len(), 5);
        assert_eq!(more[4].name, "Moto edge");
        assert_eq!(more[4].kind, "mobile");
        // (JSON for the pages keeps both ways to reach each)
        assert_eq!(to_json(&m)[2]["ids"]["quickshare"], "Q1");
    }

    #[test]
    fn two_devices_of_one_protocol_with_the_same_name_stay_two() {
        let m = merge(&[t("Pixel 8", "mobile", "a"), t("Pixel 8", "mobile", "b")], &[]);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].localsend.as_deref(), Some("a"));
        assert_eq!(m[0].key, "pixel8");
        assert_eq!(m[1].localsend.as_deref(), Some("b"));
        assert_eq!(m[1].key, "localsend:b");
        assert_eq!(m[1].name, "Pixel 8 (2)");
        // the same peer announced twice is still one
        assert_eq!(merge(&[t("Desk", "desktop", "a"), t("Desk", "desktop", "a")], &[]).len(), 1);
        // across protocols they still merge, at most one of each per entry
        let m = merge(&[t("Pixel 8", "mobile", "a"), t("Pixel 8", "mobile", "b")], &[t("Pixel 8", "phone", "Q1"), t("Pixel 8", "phone", "Q2"), t("Pixel 8", "phone", "Q3")]);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].quickshare.as_deref(), Some("Q1"));
        assert_eq!(m[1].quickshare.as_deref(), Some("Q2"));
        assert_eq!(m[2].key, "quickshare:Q3");
        assert_eq!(m[2].name, "Pixel 8 (3)");
        assert_eq!(m[2].localsend, None);
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
