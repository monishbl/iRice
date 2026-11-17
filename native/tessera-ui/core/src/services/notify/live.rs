//! Live entries: one entry per thing that's happening, updated in place,
//! instead of a new alert each time it changes. The one mechanism both lists
//! use: the notification list (the server, model.rs's Store, and the
//! banners) and the island's alerts and their history (host-shell's
//! island.rs, shown as Notification Center's Tessera group).
//!
//!   - An entry is keyed by an event id. Tessera's own alerts carry one (the
//!     island event's `key`, or its kind for the stateful kinds: `island_key`;
//!     a notification's `x-tessera-key` hint: util::notify_live). An app's
//!     notification joins an earlier one only through the spec's replaces_id,
//!     or when the same app sends the same title again within THREAD_MS (one
//!     thread: `thread_key`, `merges`).
//!   - A change to an entry updates it quietly in place. It pops up again,
//!     with its sound, only when it got more urgent (`realert`): a step up in
//!     severity (low → critical), a failure or a disconnect, or when the
//!     entry had gone quiet for STALE_MS (something new).
//!   - The entry keeps every change with its time (`Change`, at most
//!     CHANGES_MAX, the oldest go first): it shows the latest, and expands to
//!     the whole list ("30% left · 4:10 pm", then "10% left · 4:52 pm").
//!   - When the thing is over the producer resolves it (`Resolve`): the entry
//!     goes away (a battery that's plugged in), or stays as its final "done"
//!     state, quietly (a finished transfer).

use serde::{Deserialize, Serialize};

/// Changes an entry keeps (its history), newest last
pub const CHANGES_MAX: usize = 20;
/// An app's same title again within this long: the same thread
pub const THREAD_MS: i64 = 120_000;
/// A keyed entry quiet for this long: its next change is news again
pub const STALE_MS: i64 = 600_000;

/// One state an entry had: when, and what it said then
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Change {
    /// ms since the epoch
    pub at: i64,
    /// the title, then the detail ("Low battery · 10% left · plug in soon")
    pub text: String,
}

/// How urgent a state is: 0 news, 1 worth a look (orange, yellow), 2 urgent
/// (red), 3 critical (it's about to act: sleep, shut down)
pub type Severity = u8;

/// One state of an entry, as the rules see it
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub severity: Severity,
    /// a failure, a disconnect, something lost
    pub failure: bool,
    pub text: String,
    pub at: i64,
}

/// A change adds its line; the same text again only moves its time (a
/// reading that comes twice isn't a change), and the oldest go past CHANGES_MAX
pub fn push_change(changes: &mut Vec<Change>, c: Change) {
    if let Some(last) = changes.last_mut() {
        if last.text == c.text {
            last.at = c.at;
            return;
        }
    }
    changes.push(c);
    let over = changes.len().saturating_sub(CHANGES_MAX);
    changes.drain(..over);
}

/// Should the new state pop up again (a banner or the island's card, its
/// sound), or update the entry quietly where it is?
pub fn realert(old: &State, new: &State) -> bool {
    new.severity > old.severity
        || (new.failure && (!old.failure || new.text != old.text))
        // (back after a failure or a disconnect: the news that it's over)
        || (old.failure && !new.failure)
        || new.at - old.at >= STALE_MS
}

/// What a producer says about an entry that's over
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolve {
    /// still going
    No,
    /// over, and the entry stays as this final state (quietly)
    Done,
    /// over: the entry goes away
    Remove,
}

impl Resolve {
    pub fn from_str(s: &str) -> Self {
        match s {
            "done" | "true" => Self::Done,
            "remove" | "gone" => Self::Remove,
            _ => Self::No,
        }
    }
}

/// Words that say something failed or went away (titles and details are
/// short and plain: the island's, and the shell's own notifications)
pub fn says_failure(text: &str) -> bool {
    let t = text.to_lowercase();
    ["disconnected", "failed", "didn't work", "couldn't", "connection lost", "not saved", "removed unexpectedly"].iter().any(|w| t.contains(w))
}

/// A notification's category that says so (the spec's names, and theirs
/// with a failure: "transfer.error", "network.disconnected", "device.removed")
pub fn category_failure(category: &str) -> bool {
    let c = category.to_lowercase();
    c.ends_with(".error") || c.ends_with(".failed") || c.ends_with("disconnected") || c == "device.removed"
}

/// An island event's severity: its `severity`, else its colour (red 2,
/// orange or yellow 1, else 0)
pub fn severity_of_color(color: &str) -> Severity {
    let c = color.to_lowercase();
    if ["danger", "red", "#ff453a", "#ff3b30", "error"].contains(&c.as_str()) {
        2
    } else if ["orange", "yellow", "warning", "#ff9f0a", "#ffd60a", "#ff9500"].contains(&c.as_str()) {
        1
    } else {
        0
    }
}

/// A notification's severity: its x-tessera-severity, else its urgency
/// (low and normal 0, critical 2)
pub fn severity_of_urgency(hint: Option<u8>, urgency: u8) -> Severity {
    hint.unwrap_or(if urgency >= 2 { 2 } else { 0 }).min(3)
}

/// The island's kinds that are one ongoing thing each: their alerts are one
/// live entry, keyed by the kind when the event has no key of its own
/// (not the ones that answer something you just did, which always show:
/// plugging in the charger, switching the output, Focus, recording)
pub const STATEFUL_KINDS: &[&str] = &["battery", "temperature", "resources", "network", "sysupdate", "storage"];

/// An island event's entry key: its own `key`, else its kind when that's a
/// stateful one, else none ("": the old rule, the same title within a minute)
pub fn island_key(kind: &str, key: &str) -> String {
    if !key.is_empty() {
        key.to_string()
    } else if STATEFUL_KINDS.contains(&kind) {
        kind.to_string()
    } else {
        String::new()
    }
}

/// A notification's thread: its key from the app (x-tessera-key), else
/// the app and its title
pub fn thread_key(app: &str, key: &str, summary: &str) -> String {
    if !key.is_empty() {
        format!("k\u{1f}{app}\u{1f}{key}")
    } else {
        format!("s\u{1f}{app}\u{1f}{summary}")
    }
}

/// Does a new notification join a thread whose latest came at `last_at`?
/// A keyed one always (while its entry is there), a title only within
/// THREAD_MS of the last time it came
pub fn merges(thread: &str, last_at: i64, now: i64) -> bool {
    thread.starts_with('k') || (now - last_at >= 0 && now - last_at < THREAD_MS)
}

/// The history as an entry shows it expanded, newest first: "<text> · <time>"
pub fn history_lines(changes: &[Change], clock: impl Fn(i64) -> String) -> Vec<String> {
    changes.iter().rev().map(|c| format!("{} · {}", c.text, clock(c.at))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(severity: Severity, failure: bool, text: &str, at: i64) -> State {
        State { severity, failure, text: text.into(), at }
    }

    #[test]
    fn the_history_keeps_the_last_twenty() {
        let mut c = Vec::new();
        for i in 0..30 {
            push_change(&mut c, Change { at: i, text: format!("{i}%") });
        }
        assert_eq!(c.len(), CHANGES_MAX);
        assert_eq!(c[0].text, "10%"); // the oldest went first
        assert_eq!(c.last().unwrap().text, "29%");
        // the same text again: only its time moves
        push_change(&mut c, Change { at: 99, text: "29%".into() });
        assert_eq!(c.len(), CHANGES_MAX);
        assert_eq!(c.last().unwrap().at, 99);
    }

    #[test]
    fn it_pops_up_again_only_when_more_urgent() {
        let low = st(1, false, "30% left", 0);
        // the same or a lower severity: quiet
        assert!(!realert(&low, &st(1, false, "25% left", 60_000)));
        assert!(!realert(&low, &st(0, false, "Charging", 60_000)));
        // a step up: low → critical
        assert!(realert(&low, &st(2, false, "10% left", 60_000)));
        assert!(realert(&st(2, false, "10%", 0), &st(3, false, "3%", 1)));
        // a failure or a disconnect, once (the same failure again stays quiet)
        let up = st(0, false, "Connected to ACT", 0);
        let down = st(0, true, "Disconnected from ACT", 1000);
        assert!(realert(&up, &down));
        assert!(!realert(&down, &st(0, true, "Disconnected from ACT", 2000)));
        assert!(realert(&down, &st(0, true, "Transfer failed", 3000)));
        // back after a failure: that's news too
        assert!(realert(&down, &st(0, false, "Connected to ACT", 4000)));
        // connected to another network, no failure between: quiet
        assert!(!realert(&up, &st(0, false, "Connected to Home", 4000)));
        // quiet for ten minutes: news again
        assert!(realert(&up, &st(0, false, "Connected to Home", STALE_MS)));
    }

    #[test]
    fn keys_and_threads() {
        assert_eq!(island_key("battery", ""), "battery");
        assert_eq!(island_key("device", "bt:AA"), "bt:AA");
        assert_eq!(island_key("custom", ""), "");
        assert_eq!(island_key("network", "network:wlan0"), "network:wlan0");
        let keyed = thread_key("Tessera", "battery", "Low battery");
        let titled = thread_key("Chat", "", "Ann");
        assert_ne!(thread_key("Chat", "", "Ann"), thread_key("Mail", "", "Ann")); // another app: another thread
        // a keyed thread joins whenever it's there; a title within two minutes
        assert!(merges(&keyed, 0, 10 * THREAD_MS));
        assert!(merges(&titled, 1000, 1000 + THREAD_MS - 1));
        assert!(!merges(&titled, 1000, 1000 + THREAD_MS));
        assert!(!merges(&titled, 5000, 1000)); // (a clock that went back)
    }

    #[test]
    fn severity_and_failure_words() {
        assert_eq!(severity_of_color("#FF453A"), 2);
        assert_eq!(severity_of_color("danger"), 2);
        assert_eq!(severity_of_color("#FFD60A"), 1);
        assert_eq!(severity_of_color("#0A84FF"), 0);
        assert_eq!(severity_of_urgency(None, 2), 2);
        assert_eq!(severity_of_urgency(None, 1), 0);
        assert_eq!(severity_of_urgency(Some(3), 1), 3);
        assert!(says_failure("Recording didn't work"));
        assert!(says_failure("Disconnected"));
        assert!(!says_failure("Connected"));
        assert!(category_failure("transfer.error"));
        assert!(category_failure("network.disconnected"));
        assert!(!category_failure("network.connected"));
        assert_eq!(Resolve::from_str("remove"), Resolve::Remove);
        assert_eq!(Resolve::from_str("done"), Resolve::Done);
        assert_eq!(Resolve::from_str(""), Resolve::No);
    }

    #[test]
    fn the_expanded_history_reads_newest_first() {
        let c = vec![Change { at: 1, text: "Low battery · 30% left".into() }, Change { at: 2, text: "Low battery · 10% left".into() }];
        let l = history_lines(&c, |t| if t == 1 { "4:10 pm".into() } else { "4:52 pm".into() });
        assert_eq!(l, vec!["Low battery · 10% left · 4:52 pm", "Low battery · 30% left · 4:10 pm"]);
    }
}
