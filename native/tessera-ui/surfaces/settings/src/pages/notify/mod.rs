//! The "notify" page group: Notifications & Island (with its Island and
//! Sound when you log in subpages), Focus, and Windows & Spaces (with the
//! Windows, Stages and Overview part). Its own views: ui/settings-notify.slint.
//! What only the QML shell knows comes from
//! shell/modules/settings/RustBridgeNotify.qml (`cx.feed["notify"]`: Focus
//! and its modes, the notification count, sounds' files, the decorations
//! plugin, the CPU's temperature…); what only it does goes there
//! (`cx.ask("notify", …)`: play a sound, Do Not Disturb, Focus modes, clear
//! notifications, install hyprbars).

pub mod focus;
pub mod notifications;
pub mod windowrules;

use super::super::model::{Ctx, Page, Row};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "notifications" => notifications::notifications(cx, p),
        "notifications/island" => notifications::island(cx, p),
        "notifications/startup" => notifications::startup(cx, p),
        "focus" => focus::focus(cx, p),
        "windowrules" => windowrules::windowrules(cx, p),
        _ => return false,
    }
    true
}

/// The shell's side for this group (RustBridgeNotify.qml's feed)
pub fn feed<'a>(cx: &'a Ctx) -> &'a Value {
    &cx.feed["notify"]
}

/// Where a value is in a list (0 when it isn't: Math.max(0, indexOf))
pub fn idx(values: &[&str], v: &str) -> i32 {
    values.iter().position(|x| *x == v).unwrap_or(0) as i32
}

/// A switch row for a boolean setting
pub fn sw(cx: &mut Ctx, path: &str, label: &str, detail: &str) -> Row {
    let on = cx.b(path);
    Row::new(label).detail(detail).switch(on, cx.toggle(path))
}

/// A Material custom row's place in its group (settings-notify.slint's
/// `r.index`): 1 first, 2 last, 3 both
pub fn place(first: bool, last: bool) -> i32 {
    first as i32 | (last as i32) << 1
}

/// Play a sound once a slider rests (QML plays it as the slider is let go):
/// each move starts the wait again
pub fn play_when_still(what: &'static str) {
    static GEN: AtomicU64 = AtomicU64::new(0);
    let mine = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(450));
        if GEN.load(Ordering::SeqCst) == mine {
            crate::shell::ask("settings", serde_json::json!(["act", "notify", "play", what]));
        }
    });
}

thread_local! {
    static APP_ICONS: std::cell::RefCell<std::collections::HashMap<String, Option<slint::Image>>> = Default::default();
}

/// An app's icon from the icon theme (Apps.iconFor: its desktop entry's
/// icon, else the class itself), read once
pub fn app_icon(cx: &mut Ctx, class: &str) -> Option<slint::Image> {
    cx.want("apps");
    cx.want("icons");
    let shell = cx.shell;
    let icons = shell.get::<crate::services::icons::Icons>()?;
    let name = shell
        .get::<crate::services::apps::Apps>()
        .and_then(|a| a.entry_for(class).or_else(|| a.by_id(class)))
        .map(|e| e.icon.clone())
        .unwrap_or_default();
    let key = format!("{}|{name}|{class}", icons.theme);
    if let Some(hit) = APP_ICONS.with(|m| m.borrow().get(&key).cloned()) {
        return hit;
    }
    let img = icons.path(&[name.as_str(), class], &[]).and_then(|p| slint::Image::load_from_path(&p).ok());
    APP_ICONS.with(|m| m.borrow_mut().insert(key, img.clone()));
    img
}

/// "22:30" as the shell shows a time (Focus.fmt: TimeFormat.hm, "10:30 PM"
/// or "22:30")
pub fn fmt_time(hhmm: &str, h24: bool) -> String {
    let mut it = hhmm.split(':');
    let h: u32 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0) % 24;
    let m: u32 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0) % 60;
    if h24 {
        format!("{h:02}:{m:02}")
    } else {
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{h12}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_as_the_shell_writes_them() {
        assert_eq!(fmt_time("22:30", true), "22:30");
        assert_eq!(fmt_time("22:30", false), "10:30 PM");
        assert_eq!(fmt_time("0:05", false), "12:05 AM");
        assert_eq!(fmt_time("12:00", false), "12:00 PM");
        assert_eq!(fmt_time("7:00", true), "07:00");
    }

    #[test]
    fn places_and_indexes() {
        assert_eq!(place(true, false), 1);
        assert_eq!(place(false, true), 2);
        assert_eq!(place(true, true), 3);
        assert_eq!(idx(&["a", "b"], "b"), 1);
        assert_eq!(idx(&["a", "b"], "x"), 0);
    }
}
