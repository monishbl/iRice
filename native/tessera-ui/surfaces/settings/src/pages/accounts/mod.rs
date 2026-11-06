//! The "accounts" page group: Accounts (Google, iCloud and Microsoft 365 as
//! its parts, with their drives: DriveGroup.qml), Calendar, AI & Sidebar
//! (sidebar.qml its part), Default Apps. What the shell's services know
//! (accounts, calendars, task lists, AI chats, whether a key is set) comes
//! from shell/modules/settings/RustBridgeAccounts.qml as `cx.feed["accounts"]`;
//! signing in and out, syncing and the keyring are asked of it
//! (`cx.ask("accounts", …)`). Its own views: ui/settings-accounts.slint (a
//! text field that follows the page's state, a subscription's controls,
//! the AI note's text area).

pub mod accounts;
pub mod ai;
pub mod calendar;
pub mod defaultapps;

use super::super::model::{Ctx, Page, Row};
use serde_json::Value;

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "accounts" => accounts::page(cx, p),
        "calendar" => calendar::page(cx, p),
        "ai" => ai::page(cx, p),
        "defaultapps" => defaultapps::page(cx, p),
        _ => return false,
    }
    true
}

/// "2 min ago", as the QML pages' ago(t) (t: ms since the epoch; 0 none)
pub fn ago(t: f64, now: i64) -> String {
    if t <= 0.0 {
        return "not yet".into();
    }
    let m = ((now as f64 - t) / 60000.0).round() as i64;
    if m < 1 {
        "just now".into()
    } else if m < 60 {
        format!("{m} min ago")
    } else if m < 1440 {
        format!("{} h ago", (m as f64 / 60.0).round() as i64)
    } else {
        date((t / 1000.0) as i64)
    }
}

/// A day as the shell's locale writes it short ("5 November 2026")
fn date(secs: i64) -> String {
    let t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{} {} {}", tm.tm_mday, MONTHS[tm.tm_mon.clamp(0, 11) as usize], tm.tm_year + 1900)
}

/// The bridge's word for a thing (feed.accounts[…]), Null when it hasn't said
pub fn feed<'a>(cx: &Ctx<'a>) -> &'a Value {
    let f: &'a Value = cx.feed;
    &f["accounts"]
}

pub fn s(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}

pub fn arr(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// An action as a value (a closure with the signature actions have)
pub fn action(f: impl Fn(&mut Ctx, &super::super::model::Arg) + 'static) -> super::super::model::Action {
    std::rc::Rc::new(f)
}

/// A text field kept in the page's own state (`local[key]`), so a button
/// can read and clear it (settings-accounts.slint's "field": it follows the
/// state when the page changes it): typing stores it; Enter runs `enter`
pub fn field(cx: &mut Ctx, key: &str, label: &str, placeholder: &str, width: f32, secret: bool, enter: Option<super::super::model::Action>) -> Row {
    let text = cx.local_s(key);
    let k = key.to_string();
    let id = cx.on(move |cx, a| {
        if a.i == 0 {
            cx.local_set(&k, a.s.clone());
        } else if a.i == 1 {
            cx.local_set(&k, a.s.clone());
            if let Some(f) = &enter {
                f(cx, a);
            }
        }
    });
    let mut r = Row::new(label).ctl("accounts", "field", id).text_value(text).placeholder(placeholder).width(width);
    if secret {
        r = r.secret();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_as_the_pages_say_it() {
        let now = 1_000_000_000_000i64;
        assert_eq!(ago(0.0, now), "not yet");
        assert_eq!(ago((now - 20_000) as f64, now), "just now");
        assert_eq!(ago((now - 5 * 60_000) as f64, now), "5 min ago");
        assert_eq!(ago((now - 3 * 3_600_000) as f64, now), "3 h ago");
        assert!(ago((now - 3 * 86_400_000) as f64, now).contains("2001"));
    }
}
