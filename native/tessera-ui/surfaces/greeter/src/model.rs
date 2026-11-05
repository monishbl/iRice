//! The login screen's logic, as GreeterAuth.qml and GreeterWindow.qml: who
//! and which session, the greetd conversation, and what each key and click
//! does, the same in every style. Pure: it changes its state and lists what
//! should happen (`Effect`s: a request for greetd, a timer, a cleared
//! field, a shake…); the part (mod.rs) does them and draws the state.
//!
//! The conversation starts as soon as a person is chosen, not when Enter is
//! pressed (300 ms after the last change), so a fingerprint reader
//! (pam_fprintd) listens straight away: its words are `message`, and a
//! finger logs in by itself. A password typed meanwhile waits (`pending`)
//! for PAM to ask for it. Without greetd (or with TESSERA_GREETER_MOCK=1)
//! it's a preview: the password "tessera" "logs in" (the screen quits), and
//! Escape quits.
//!
//! greetd answers every request once, in order: each request sent is
//! queued with the conversation it belongs to (`gen`, one more after every
//! cancel), so an answer to a conversation already cancelled is dropped.

use super::greetd::{AuthKind, ErrorType, Request, Response};
use super::system;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub full: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub exec: String,
}

/// The fingerprint reader, as PAM's words say
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Finger {
    /// none heard of
    #[default]
    None,
    Waiting,
    Miss,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// a request for greetd
    Send(Request),
    /// (re)start the 300 ms timer that opens the conversation (`begin`)
    BeginLater,
    /// a preview's check: `mock_result` with it in 600 ms
    MockCheck(String),
    /// keep the person and session for next time
    Remember { user: String, session: String },
    /// a preview "logged in": quit in 1.2 s
    PreviewDone,
    /// quit now (the session started, or Escape in a preview)
    Quit,
    /// systemctl suspend | reboot | poweroff
    Power(String),
    /// empty the password field
    ClearField,
    /// a password was refused (the field's already cleared): shake
    Refused,
    /// the keyboard to the password field
    FocusPassword,
    /// the keyboard to the username field, its text `other_name`, selected
    FocusName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Create,
    Respond,
    Start,
    Cancel,
}

#[derive(Default)]
pub struct Model {
    pub mock: bool,
    pub users: Vec<User>,
    pub sessions: Vec<Session>,
    pub user: usize,
    pub session: usize,
    pub busy: bool,
    pub error: String,
    /// what PAM asked ("Password: ")
    pub prompt: String,
    pub success: bool,
    /// What PAM says that isn't a question (the fingerprint reader's words)
    pub message: String,
    /// a username typed in instead of a person chosen (always, with no one listed)
    pub other: bool,
    pub other_name: String,
    pub finger: Finger,
    /// "" | suspend | reboot | poweroff: chosen once, done on a second click or Enter
    pub power_choice: String,
    /// the text style's row: 0 session, 1 login, 2 password
    pub row: i32,
    /// a password waiting for greetd's prompt
    pending: Option<String>,
    /// the conversation: open (a person's chosen), and asked for the password
    open: bool,
    asked: bool,
    /// greetd has a session for us (created, not cancelled)
    live: bool,
    gen: u64,
    queue: VecDeque<(Kind, u64)>,
    out: Vec<Effect>,
}

impl Model {
    pub fn new(mock: bool, users: Vec<User>, sessions: Vec<Session>) -> Self {
        let mut m = Model { mock, users, sessions, prompt: "Password".into(), row: 2, ..Default::default() };
        if m.sessions.is_empty() {
            m.sessions = system::sort_sessions(Vec::new());
        }
        if m.users.is_empty() {
            m.other = true;
        }
        m
    }

    /// What should happen now (and forget it)
    pub fn take(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.out)
    }

    /// Who logs in: the chosen person's username, or the one typed in
    pub fn user_name(&self) -> String {
        if self.other {
            self.other_name.trim().to_string()
        } else {
            self.users.get(self.user).map(|u| u.name.clone()).unwrap_or_default()
        }
    }

    pub fn person(&self) -> Option<&User> {
        self.users.get(self.user)
    }

    pub fn session_name(&self) -> Option<&str> {
        self.sessions.get(self.session).map(|s| s.name.as_str())
    }

    /// The picture's name: AccountsService's, the typed one for a typed username
    pub fn picture_name(&self) -> String {
        if self.other {
            String::new()
        } else {
            self.user_name()
        }
    }

    fn send(&mut self, r: Request) {
        let kind = match r {
            Request::CreateSession { .. } => Kind::Create,
            Request::PostAuthMessageResponse { .. } => Kind::Respond,
            Request::StartSession { .. } => Kind::Start,
            Request::CancelSession => Kind::Cancel,
        };
        self.queue.push_back((kind, self.gen));
        match kind {
            Kind::Create => self.live = true,
            Kind::Cancel => {
                self.live = false;
                self.gen += 1;
            }
            _ => {}
        }
        self.out.push(Effect::Send(r));
    }

    /// Errors shake the field once each time a new one comes (onErrorChanged)
    fn set_error(&mut self, e: &str) {
        if e != self.error {
            self.error = e.to_string();
            if !e.is_empty() {
                self.out.push(Effect::ClearField);
                self.out.push(Effect::Refused);
            }
        }
    }

    // ── The person ──────────────────────────────────────────────────────

    pub fn set_user(&mut self, i: usize) {
        if i != self.user {
            self.user = i;
            self.out.push(Effect::ClearField);
            self.restart();
        }
    }

    fn set_other(&mut self, b: bool) {
        if b != self.other {
            self.other = b;
            self.restart();
            if !b {
                self.out.push(Effect::FocusPassword);
            }
        }
    }

    pub fn set_other_name(&mut self, s: &str) {
        if s != self.other_name {
            self.other_name = s.to_string();
            if self.other {
                self.restart();
            }
        }
    }

    /// Typing a name: starts from the chosen person's
    fn auth_edit_name(&mut self) {
        if self.busy || self.other {
            return;
        }
        self.other_name = self.person().map(|u| u.name.clone()).unwrap_or_default();
        self.set_other(true);
    }

    /// Done typing: a listed name is that person again
    pub fn pick_typed(&mut self) {
        let typed = self.other_name.trim().to_string();
        let Some(i) = self.users.iter().position(|u| u.name == typed) else { return };
        if !self.other {
            return;
        }
        self.set_user(i);
        self.set_other(false);
    }

    /// Back to the list (when there is one)
    fn auth_cancel_name(&mut self) {
        if !self.users.is_empty() {
            self.set_other(false);
        }
    }

    /// The last person and session (the cache file), at the start
    pub fn restore(&mut self, j: &serde_json::Value) {
        let user = j["user"].as_str().unwrap_or("");
        if let Some(u) = self.users.iter().position(|x| x.name == user) {
            self.set_user(u);
        } else if !user.is_empty() {
            // (last time, a username typed in)
            self.other_name = user.to_string();
            self.set_other(true);
        }
        if let Some(s) = self.sessions.iter().position(|x| x.name == j["session"].as_str().unwrap_or("\u{0}")) {
            self.session = s;
        }
    }

    // ── The conversation ────────────────────────────────────────────────

    pub fn login(&mut self, password: &str) {
        if self.busy {
            return;
        }
        if self.user_name().is_empty() {
            self.set_error("Type your username first.");
            return;
        }
        self.set_error("");
        self.busy = true;
        if self.mock {
            self.out.push(Effect::MockCheck(password.to_string()));
            return;
        }
        if self.asked {
            self.asked = false;
            self.send(Request::PostAuthMessageResponse { response: Some(password.to_string()) });
        } else {
            self.pending = Some(password.to_string());
            self.begin();
        }
    }

    /// Opens the conversation for the chosen person (again after a failure,
    /// so the reader listens again)
    pub fn begin(&mut self) {
        if self.mock || self.open || self.user_name().is_empty() {
            return;
        }
        self.open = true;
        self.asked = false;
        let username = self.user_name();
        self.send(Request::CreateSession { username });
    }

    pub fn restart(&mut self) {
        if self.mock {
            return;
        }
        if self.live {
            self.send(Request::CancelSession);
        }
        self.open = false;
        self.asked = false;
        self.message.clear();
        self.finger = Finger::None;
        self.out.push(Effect::BeginLater);
    }

    fn launch(&mut self) {
        let (name, exec) = self.sessions.get(self.session).map(|s| (s.name.clone(), s.exec.clone())).unwrap_or_default();
        let command = match system::parse_exec(&exec, &name) {
            Ok(c) => c,
            Err(e) => {
                // (a field the parser can't expand: say what to do, not the parser's words)
                eprintln!("greeter: {e}");
                self.set_error("This session's launch command isn't supported. Choose another with Tab.");
                self.busy = false;
                self.restart();
                return;
            }
        };
        self.out.push(Effect::Remember { user: self.user_name(), session: name });
        self.success = true;
        if self.mock {
            eprintln!("greeter (preview): would launch {command:?}");
            self.out.push(Effect::PreviewDone);
            return;
        }
        self.send(Request::StartSession { cmd: command, env: Vec::new() });
    }

    /// The preview's answer, 600 ms after Enter
    pub fn mock_result(&mut self, password: &str) {
        self.busy = false;
        if password == "tessera" {
            self.launch();
        } else {
            self.set_error("Incorrect password. Try again.");
        }
    }

    fn failed(&mut self, e: &str) {
        self.busy = false;
        self.pending = None;
        self.set_error(e);
        self.restart();
    }

    /// greetd's answer to the oldest request still waiting for one
    pub fn heard(&mut self, r: Response) {
        let Some((kind, gen)) = self.queue.pop_front() else { return };
        if kind == Kind::Cancel || gen != self.gen {
            return; // (a conversation already cancelled)
        }
        match (kind, r) {
            (Kind::Start, Response::Success) => self.out.push(Effect::Quit),
            (Kind::Start, Response::Error { description, .. }) => {
                self.success = false;
                self.failed(if description.is_empty() { "Login failed. Try again." } else { &description });
            }
            (_, Response::Success) => {
                // (ready to launch)
                self.busy = false;
                self.open = false;
                self.launch();
            }
            (_, Response::AuthMessage { auth_message_type: t, auth_message: msg }) => match t {
                AuthKind::Visible | AuthKind::Secret => match self.pending.take() {
                    Some(p) => self.send(Request::PostAuthMessageResponse { response: Some(p) }),
                    None => {
                        self.asked = true;
                        self.prompt = if msg.is_empty() { "Password".into() } else { msg };
                    }
                },
                AuthKind::Info | AuthKind::Error => {
                    let error = t == AuthKind::Error;
                    let low = msg.to_lowercase();
                    if ["finger", "swipe", "touch"].iter().any(|w| low.contains(w)) {
                        // (pam_fprintd: listening, or the finger didn't match)
                        self.finger = if error { Finger::Miss } else { Finger::Waiting };
                        self.message = msg;
                    } else if error {
                        self.set_error(&msg);
                    } else {
                        self.message = msg;
                    }
                    // (no answer wanted: greetd goes on once it has one)
                    self.send(Request::PostAuthMessageResponse { response: None });
                }
            },
            (_, Response::Error { error_type: ErrorType::AuthError, .. }) => self.failed("Incorrect password. Try again."),
            (_, Response::Error { description, .. }) => self.failed(if description.is_empty() { "Login failed. Try again." } else { &description }),
        }
    }

    /// greetd went away (or never answered the socket)
    pub fn lost(&mut self) {
        self.queue.clear();
        self.live = false;
        self.open = false;
        self.asked = false;
        self.gen += 1;
        self.busy = false;
        self.pending = None;
        self.set_error("The login service isn't answering. Try again in a moment.");
    }

    // ── What the keys and clicks do (GreeterWindow.qml) ──────────────────

    pub fn next_user(&mut self, step: i32) {
        if self.busy || self.users.is_empty() {
            return;
        }
        if self.other {
            // (typing: back to the chosen person)
            self.auth_cancel_name();
            return;
        }
        let n = self.users.len() as i32;
        if n < 2 {
            return;
        }
        let i = (self.user as i32 + n + step).rem_euclid(n) as usize;
        self.set_user(i);
    }

    /// Type a name (a click on the name, or Switch user): the field, the
    /// name in it selected
    pub fn edit_name(&mut self) {
        self.auth_edit_name();
        if self.other {
            self.out.push(Effect::FocusName);
        }
    }

    /// From the username field to the password (Enter or Tab)
    pub fn to_password(&mut self) {
        self.pick_typed();
        self.out.push(Effect::FocusPassword);
    }

    /// Esc in the username field: the chosen person again
    pub fn cancel_name(&mut self) {
        self.auth_cancel_name();
        self.out.push(Effect::FocusPassword);
    }

    pub fn next_session(&mut self, step: i32) {
        if !self.busy && !self.sessions.is_empty() {
            let n = self.sessions.len() as i32;
            self.session = (self.session as i32 + n + step).rem_euclid(n) as usize;
        }
    }

    /// Enter: a power choice to confirm, else log in
    pub fn submit(&mut self, password: &str) {
        if !self.power_choice.is_empty() {
            let action = std::mem::take(&mut self.power_choice);
            self.out.push(Effect::Power(action));
        } else if !self.busy {
            self.login(password);
        }
    }

    /// Sleep, Restart, Shut Down: a first click asks, a second does it
    pub fn choose_power(&mut self, action: &str) {
        if self.busy || !["suspend", "reboot", "poweroff"].contains(&action) {
            return;
        }
        if self.power_choice == action {
            self.submit("");
        } else {
            self.power_choice = action.to_string();
        }
        self.out.push(Effect::FocusPassword);
    }

    /// Escape: drops a power choice, else quits a preview, else clears the field
    pub fn cancel(&mut self) {
        if !self.power_choice.is_empty() {
            self.power_choice.clear();
        } else if self.mock {
            self.out.push(Effect::Quit);
        } else {
            self.out.push(Effect::ClearField);
        }
    }

    /// The text style's rows: leaving the login row takes the name typed
    pub fn set_row(&mut self, r: i32) {
        self.row = r.clamp(0, 2);
        if self.row != 1 {
            self.pick_typed();
        }
    }

    /// The text style's login row, typed as in ly (your name to start
    /// with): a character, or "" for Backspace
    pub fn name_key(&mut self, text: &str) {
        if self.busy {
            return;
        }
        self.auth_edit_name();
        let mut name = self.other_name.clone();
        if text.is_empty() {
            name.pop();
        } else {
            name.push_str(text);
        }
        self.set_other_name(&name);
    }

    /// A demo's state, set at once (`--demo STATE`)
    pub fn force(&mut self, busy: bool, error: &str, message: &str, finger: Finger) {
        self.busy = busy;
        self.error = error.to_string();
        self.message = message.to_string();
        self.finger = finger;
    }
}

#[cfg(test)]
mod tests {
    use super::super::greetd::{tests::Fake, Client, Event};
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn people() -> Vec<User> {
        system::demo_users()
    }
    fn sessions() -> Vec<Session> {
        system::demo_sessions()
    }
    fn sent(e: &[Effect]) -> Vec<Request> {
        e.iter().filter_map(|x| if let Effect::Send(r) = x { Some(r.clone()) } else { None }).collect()
    }
    fn secret() -> Response {
        Response::AuthMessage { auth_message_type: AuthKind::Secret, auth_message: "Password: ".into() }
    }

    #[test]
    fn a_preview_takes_tessera() {
        let mut m = Model::new(true, people(), sessions());
        m.submit("nope");
        assert!(m.busy);
        assert_eq!(m.take(), vec![Effect::MockCheck("nope".into())]);
        m.mock_result("nope");
        assert_eq!(m.error, "Incorrect password. Try again.");
        assert_eq!(m.take(), vec![Effect::ClearField, Effect::Refused]);
        m.submit("tessera");
        m.take();
        m.mock_result("tessera");
        assert!(m.success && m.error.is_empty() && !m.busy);
        let e = m.take();
        assert!(e.contains(&Effect::Remember { user: "ada".into(), session: "Tessera".into() }));
        assert!(e.contains(&Effect::PreviewDone));
        // Escape quits a preview
        m.cancel();
        assert_eq!(m.take(), vec![Effect::Quit]);
        // (never greetd)
        m.restart();
        m.begin();
        assert!(sent(&m.take()).is_empty());
    }

    #[test]
    fn the_conversation_opens_when_a_person_is_chosen() {
        let mut m = Model::new(false, people(), sessions());
        m.begin();
        assert_eq!(sent(&m.take()), vec![Request::CreateSession { username: "ada".into() }]);
        m.heard(secret());
        assert!(m.take().is_empty());
        // typed after PAM asked: answered at once
        m.submit("tessera");
        assert_eq!(sent(&m.take()), vec![Request::PostAuthMessageResponse { response: Some("tessera".into()) }]);
        m.heard(Response::Success);
        assert_eq!(sent(&m.take()), vec![Request::StartSession { cmd: vec!["tessera-session".into()], env: vec![] }]);
        m.heard(Response::Success);
        assert_eq!(m.take(), vec![Effect::Quit]);
    }

    #[test]
    fn a_password_typed_before_pam_asks_waits() {
        let mut m = Model::new(false, people(), sessions());
        // Enter before the 300 ms timer opened it: it opens now, the password waits
        m.submit("tessera");
        assert_eq!(sent(&m.take()), vec![Request::CreateSession { username: "ada".into() }]);
        m.heard(secret());
        assert_eq!(sent(&m.take()), vec![Request::PostAuthMessageResponse { response: Some("tessera".into()) }]);
    }

    #[test]
    fn a_wrong_password_shakes_cancels_and_listens_again() {
        let mut m = Model::new(false, people(), sessions());
        m.begin();
        m.heard(secret());
        m.submit("wrong");
        m.take();
        m.heard(Response::Error { error_type: ErrorType::AuthError, description: "pam".into() });
        assert_eq!(m.error, "Incorrect password. Try again.");
        assert!(!m.busy);
        let e = m.take();
        assert!(e.contains(&Effect::Refused) && e.contains(&Effect::ClearField) && e.contains(&Effect::BeginLater));
        assert_eq!(sent(&e), vec![Request::CancelSession]);
        // the cancel's own answer, then a new conversation
        m.heard(Response::Success);
        m.begin();
        assert_eq!(sent(&m.take()), vec![Request::CreateSession { username: "ada".into() }]);
        // the same error again still shakes (login clears it first)
        m.heard(secret());
        m.submit("wrong");
        m.heard(Response::Error { error_type: ErrorType::AuthError, description: String::new() });
        assert!(m.take().contains(&Effect::Refused));
    }

    #[test]
    fn the_fingerprint_reader_speaks() {
        let mut m = Model::new(false, people(), sessions());
        m.begin();
        m.take();
        m.heard(Response::AuthMessage { auth_message_type: AuthKind::Info, auth_message: "Place your finger on the fingerprint reader".into() });
        assert_eq!(m.finger, Finger::Waiting);
        assert_eq!(m.message, "Place your finger on the fingerprint reader");
        assert_eq!(sent(&m.take()), vec![Request::PostAuthMessageResponse { response: None }]);
        m.heard(Response::AuthMessage { auth_message_type: AuthKind::Error, auth_message: "Failed to match fingerprint".into() });
        assert_eq!(m.finger, Finger::Miss);
        assert!(m.error.is_empty(), "a miss isn't an error line");
        m.take();
        // other words: a message, or an error
        m.heard(Response::AuthMessage { auth_message_type: AuthKind::Info, auth_message: "Hello".into() });
        assert_eq!(m.message, "Hello");
        m.heard(Response::AuthMessage { auth_message_type: AuthKind::Error, auth_message: "Account locked".into() });
        assert_eq!(m.error, "Account locked");
        // a finger that matches: in, without Enter
        m.take();
        m.heard(Response::Success);
        assert!(matches!(sent(&m.take())[..], [Request::StartSession { .. }]));
    }

    #[test]
    fn answers_for_a_cancelled_conversation_are_dropped() {
        let mut m = Model::new(false, people(), sessions());
        m.begin();
        m.take();
        // another person chosen before greetd answered
        m.next_user(1);
        assert_eq!(sent(&m.take()), vec![Request::CancelSession]);
        // the first conversation's prompt, then the cancel's answer: nothing
        m.heard(secret());
        m.heard(Response::Success);
        assert!(m.take().is_empty());
        assert!(!m.success);
        m.begin();
        assert_eq!(sent(&m.take()), vec![Request::CreateSession { username: "grace".into() }]);
    }

    #[test]
    fn people_go_round_and_typed_names() {
        let mut m = Model::new(false, people(), sessions());
        m.next_user(-1);
        assert_eq!(m.user, 2);
        assert!(m.take().contains(&Effect::ClearField));
        m.next_user(1);
        assert_eq!(m.user, 0);
        // a name typed in
        m.edit_name();
        assert!(m.other);
        assert_eq!(m.other_name, "ada");
        assert!(m.take().contains(&Effect::FocusName));
        m.set_other_name("grace ");
        assert_eq!(m.user_name(), "grace");
        // Enter: a listed name is that person again
        m.to_password();
        assert!(!m.other);
        assert_eq!(m.user, 1);
        // typed, not listed: stays typed; Up goes back to the list
        m.edit_name();
        m.set_other_name("someone");
        m.to_password();
        assert!(m.other);
        assert_eq!(m.user_name(), "someone");
        m.next_user(1);
        assert!(!m.other);
        // no one listed: always typed, Esc can't go back
        let mut m = Model::new(false, vec![], sessions());
        assert!(m.other);
        m.cancel_name();
        assert!(m.other);
        m.submit("x");
        assert_eq!(m.error, "Type your username first.");
        assert!(!m.busy);
    }

    #[test]
    fn sessions_and_power() {
        let mut m = Model::new(true, people(), sessions());
        m.next_session(1);
        assert_eq!(m.session_name(), Some("Hyprland"));
        m.next_session(-2);
        assert_eq!(m.session_name(), Some("Sway"));
        // a first click asks, a second does it; Enter does it too; Esc drops it
        m.choose_power("reboot");
        assert_eq!(m.power_choice, "reboot");
        m.choose_power("reboot");
        assert!(m.take().contains(&Effect::Power("reboot".into())));
        assert!(m.power_choice.is_empty());
        m.choose_power("poweroff");
        m.submit("whatever");
        assert!(m.take().contains(&Effect::Power("poweroff".into())));
        assert!(!m.busy, "Enter on a power choice doesn't log in");
        m.choose_power("suspend");
        m.cancel();
        assert!(m.power_choice.is_empty());
        assert!(!m.take().contains(&Effect::Quit), "Esc dropped the choice, didn't quit");
        m.choose_power("rm -rf");
        assert!(m.power_choice.is_empty());
        // nothing while busy
        m.busy = true;
        m.choose_power("reboot");
        m.next_session(1);
        assert!(m.power_choice.is_empty() && m.session_name() == Some("Sway"));
    }

    #[test]
    fn the_text_styles_login_row() {
        let mut m = Model::new(false, people(), sessions());
        m.set_row(1);
        m.name_key("x");
        assert!(m.other);
        assert_eq!(m.other_name, "adax");
        m.name_key("");
        m.name_key("");
        assert_eq!(m.other_name, "ad");
        m.name_key("a");
        // leaving the row: a listed name is that person
        m.set_row(2);
        assert!(!m.other && m.user == 0);
    }

    #[test]
    fn the_last_person_and_session_come_back() {
        let mut m = Model::new(false, people(), sessions());
        m.restore(&serde_json::json!({"user": "grace", "session": "Sway"}));
        assert_eq!((m.user, m.session_name()), (1, Some("Sway")));
        let mut m = Model::new(false, people(), sessions());
        m.restore(&serde_json::json!({"user": "someone"}));
        assert!(m.other && m.user_name() == "someone");
        let mut m = Model::new(false, people(), sessions());
        m.restore(&serde_json::json!(null));
        assert!(!m.other && m.user == 0);
    }

    #[test]
    fn a_session_command_it_cant_run() {
        let mut m = Model::new(true, people(), vec![Session { name: "Odd".into(), exec: "odd %x".into() }]);
        m.submit("tessera");
        m.mock_result("tessera");
        assert!(!m.success);
        assert!(m.error.contains("Choose another with Tab"));
    }

    /// The model and the client against the fake greetd, as the part drives them
    fn drive(user: usize, typed: &[&str]) -> (Model, Vec<Vec<String>>, Vec<Finger>) {
        let fake = Fake::start(&format!("drive{user}"));
        let (tx, rx) = mpsc::channel();
        let mut c = Client::connect(&fake.path, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let mut m = Model::new(false, people(), sessions());
        m.set_user(user);
        m.begin();
        let mut typed = typed.iter();
        let mut fingers = Vec::new();
        for _ in 0..40 {
            for e in m.take() {
                match e {
                    Effect::Send(r) => c.send(&r).unwrap(),
                    Effect::BeginLater => m.begin(),
                    Effect::Quit => return (m, fake.started.try_iter().collect(), fingers),
                    _ => {}
                }
            }
            // (each prompt answered with the next password)
            if m.asked && !m.busy {
                match typed.next() {
                    Some(p) => {
                        m.submit(p);
                        continue;
                    }
                    None => break,
                }
            }
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Event::Response(r)) => {
                    m.heard(r);
                    fingers.push(m.finger);
                }
                _ => break,
            }
        }
        let started = fake.started.try_iter().collect();
        (m, started, fingers)
    }

    #[test]
    fn logs_in_through_a_fake_greetd() {
        let (m, started, _) = drive(0, &["tessera"]);
        assert!(m.success);
        assert_eq!(started, vec![vec!["tessera-session".to_string()]]);
    }

    #[test]
    fn a_wrong_password_then_the_right_one_through_a_fake_greetd() {
        let (m, started, _) = drive(0, &["wrong", "tessera"]);
        assert!(m.success);
        assert_eq!(started.len(), 1);
        let (m, started, _) = drive(0, &["wrong", "bad"]);
        assert!(!m.success && started.is_empty());
        assert_eq!(m.error, "Incorrect password. Try again.");
    }

    #[test]
    fn a_fingerprint_that_misses_then_the_password_through_a_fake_greetd() {
        let (m, started, fingers) = drive(1, &["tessera"]);
        assert!(fingers.contains(&Finger::Waiting) && fingers.contains(&Finger::Miss));
        assert!(m.success);
        assert_eq!(started.len(), 1);
    }
}
