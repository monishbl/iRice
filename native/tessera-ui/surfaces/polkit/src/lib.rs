//! The authentication dialog (rust.surfaces: polkit), as
//! shell/modules/polkit/Polkit.qml: when an app needs admin rights, a modal
//! dialog over a scrim asks for your password, or for a finger first when
//! fprintd is in the PAM stack (then the password once that times out).
//! A wrong password shakes it and says so; Cancel or Esc refuses.
//!
//! ── Roles: which real protocol is taken when ──────────────────────────────
//!   The polkit agent on the SYSTEM bus (agent.rs: register with polkit's
//!     Authority for this session, polkit's own helper for the
//!     conversation): only when real: not `--demo` (nor TESSERA_UI_DEMO=1)
//!     and not a sandbox (TESSERA_SANDBOX / IRICE_SANDBOX ≠ 1), and only
//!     when the shell started it naming "polkit" in `--surfaces` (stdin its
//!     pipe: tessera-ui run by hand never registers). Polkit.qml
//!     then doesn't make its own agent (RustUi.drawing("polkit")).
//!   A test agent on the SESSION bus: only when not real and
//!     TESSERA_POLKIT_TEST=1, against a fake Authority there (src/bin/
//!     fake-polkit.rs) and a fake helper (TESSERA_POLKIT_HELPER, required:
//!     polkit's own helper is never run then).
//!   Otherwise no agent at all: the dialog shows only Polkit.qml's preview
//!     (`qs ipc call polkit preview | previewFinger`, forwarded as
//!     `{"to":"polkit","preview":true,"finger":…}`), with its fake action.
//! Not like the QML: Material's hero shape changes without morphing; Glass's
//! fingerprint turns red on a miss but doesn't pulse or shake (FingerGlyph);
//! the real agent is untested here (its socket handshake follows polkit
//! 126+'s, read from libpolkit-agent-1's strings, not its source).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod agent;

use tessera_core::services::hypr::Glass;
use tessera_core::shell::Part;
use tessera_core::surfaces::polar as shape;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use agent::{Line, Reply, Request, SessionLine};
use slint::ComponentHandle;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "polkit";
const NS: &str = "tessera:polkit";
const PREVIEW: &str = "Authentication is required to install software";

enum M {
    Submit(String),
    Cancel,
    MissOver,
    Gone,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::PolkitDialog,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

struct Req {
    message: String,
    cookie: String,
    user: String,
    reply: Reply,
}

/// What the dialog shows, from a request's conversation
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ask {
    pub prompt: String,
    pub secret: bool,
    /// the helper waits for an answer (the field shows)
    pub required: bool,
    pub supplementary: String,
}

/// The message without its full stop, as Polkit.qml shows it
pub fn message_of(m: &str) -> String {
    m.strip_suffix('.').unwrap_or(m).to_string()
}

/// The prompt without its colon ("Password: " → "Password")
pub fn prompt_of(p: &str) -> String {
    let p = p.trim();
    let p = p.strip_suffix(':').unwrap_or(p).trim();
    if p.is_empty() { "Password".into() } else { p.to_string() }
}

/// A finger first: PAM asks for no text while the reader waits
pub fn finger_wait(ask: &Ask, waiting: bool) -> bool {
    let s = ask.supplementary.to_lowercase();
    !waiting && !ask.required && ["finger", "reader", "swipe"].iter().any(|w| s.contains(w))
}

/// `--surfaces` names it (not the default "everything")
pub fn explicitly_asked(args: &[String], name: &str) -> bool {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let list = if let Some(v) = a.strip_prefix("--surfaces=") { Some(v.to_string()) } else if a == "--surfaces" { it.next().cloned() } else { None };
        if list.is_some_and(|l| l.split(',').any(|s| s.trim() == name)) {
            return true;
        }
    }
    false
}

/// Started by the shell: stdin is its pipe
fn stdin_is_pipe() -> bool {
    unsafe {
        let mut s: libc::stat = std::mem::zeroed();
        libc::fstat(0, &mut s) == 0 && matches!(s.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK)
    }
}

pub struct Polkit {
    win: Option<Win>,
    real: bool,
    queue: Vec<Req>,
    session: Option<agent::Session>,
    conv: u64,
    ask: Ask,
    preview: bool,
    preview_finger: bool,
    error: String,
    error_count: i32,
    waiting: bool,
    finger_asked_at: i64,
    finger_missed: bool,
    miss: slint::Timer,
    gone: slint::Timer,
}

impl Default for Polkit {
    fn default() -> Self {
        let real = !crate::env::demo_mode() && !crate::env::sandboxed();
        Self {
            win: None,
            real,
            queue: Vec::new(),
            session: None,
            conv: 0,
            ask: Ask::default(),
            preview: false,
            preview_finger: false,
            error: String::new(),
            error_count: 0,
            waiting: false,
            finger_asked_at: 0,
            finger_missed: false,
            miss: slint::Timer::default(),
            gone: slint::Timer::default(),
        }
    }
}

impl Polkit {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + Send + 'static {
        st.shell.sender(NAME)
    }

    fn shown(&self) -> bool {
        self.preview || !self.queue.is_empty()
    }

    fn fake_helper() -> Option<String> {
        crate::env::var("POLKIT_HELPER").filter(|h| !h.is_empty())
    }

    /// The window, on the focused screen (Hyprland's), else the first
    fn window(&mut self, st: &mut State) -> bool {
        if self.win.is_some() {
            return true;
        }
        let focused = crate::services::hypr::request("j/monitors").ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v.as_array()?.iter().find(|m| m["focused"].as_bool() == Some(true))?["name"].as_str().map(str::to_string));
        let outputs = st.outputs_now();
        let Some((wl, name)) = outputs.iter().find(|(_, n)| Some(n) == focused.as_ref()).or(outputs.first()).cloned() else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl, &name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::Exclusive,
            margin_left: 0,
        });
        let ui = ui::PolkitDialog::new().expect("PolkitDialog");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        ui.set_user(agent::user_of(unsafe { libc::getuid() }).into());
        let to = Self::send(st);
        {
            let to = to.clone();
            ui.on_submit(move |t| to(Box::new(M::Submit(t.to_string()))));
        }
        ui.on_cancel(move || to(Box::new(M::Cancel)));
        ui.show().expect("show polkit");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: name, surface, ui, a11y, glass_sent: String::new() });
        true
    }

    fn push(&mut self, st: &mut State) {
        let shown = self.shown();
        if shown {
            self.gone.stop();
            if !self.window(st) {
                return;
            }
        }
        let Some(w) = &self.win else { return };
        let fw = self.preview_finger || (!self.preview && finger_wait(&self.ask, self.waiting));
        let message = if self.preview { PREVIEW.to_string() } else { self.queue.first().map(|r| message_of(&r.message)).unwrap_or_default() };
        let hero = if fw { if self.finger_missed { "softBurst" } else { "clover8" } } else if !self.error.is_empty() { "softBurst" } else { "cookie9" };
        let ui = &w.ui;
        ui.set_message(message.into());
        ui.set_prompt(if self.preview { "Password".into() } else { prompt_of(&self.ask.prompt).into() });
        ui.set_secret(self.preview || self.ask.secret);
        ui.set_error(self.error.clone().into());
        ui.set_error_count(self.error_count);
        ui.set_waiting(self.waiting);
        ui.set_finger_wait(fw);
        ui.set_finger_missed(self.finger_missed && fw);
        ui.set_hero(shape::outline(hero, hero, 0.0, 0.0).into());
        if shown && !ui.get_shown() {
            ui.set_text("".into());
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            w.surface.show(0, 0);
            ui.set_shown(true);
            ui.invoke_focus_field();
        } else if !shown && ui.get_shown() {
            ui.set_shown(false);
            ui.set_text("".into());
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
            let to = Self::send(st);
            self.gone.start(slint::TimerMode::SingleShot, Duration::from_millis(600), move || to(Box::new(M::Gone)));
        }
    }

    /// A conversation for the request in front
    fn start_session(&mut self, st: &State) {
        self.session = None;
        self.ask = Ask::default();
        let Some(r) = self.queue.first() else { return };
        self.conv += 1;
        // (a test runs only its fake helper; the real agent only polkit's)
        let fake = if self.real { None } else { Self::fake_helper() };
        if !self.real && fake.is_none() {
            return;
        }
        match agent::converse(self.conv, &r.user, &r.cookie, fake.as_deref(), Self::send(st)) {
            Ok(s) => self.session = Some(s),
            Err(e) => {
                eprintln!("tessera-ui: polkit: can't start the helper ({e})");
                self.error = "Couldn't ask for the password. Try again.".into();
            }
        }
    }

    /// The request in front is over (answered or cancelled)
    fn finish(&mut self, st: &mut State, ok: bool) {
        if let Some(mut s) = self.session.take() {
            s.stop();
        }
        if !self.queue.is_empty() {
            let r = self.queue.remove(0);
            r.reply.send(ok);
        }
        self.error.clear();
        self.waiting = false;
        self.finger_missed = false;
        if !self.queue.is_empty() {
            self.start_session(st);
        }
        self.push(st);
    }

    fn cancel(&mut self, st: &mut State) {
        if self.preview {
            self.preview = false;
            self.preview_finger = false;
            self.error.clear();
            self.waiting = false;
            self.push(st);
            return;
        }
        self.finish(st, false);
    }

    fn submit(&mut self, st: &mut State, text: String) {
        if self.preview {
            self.error = "Incorrect password. Try again.".into();
            self.error_count += 1;
            self.push(st);
            return;
        }
        if text.is_empty() || self.queue.is_empty() || self.waiting {
            return;
        }
        let Some(s) = &self.session else { return };
        self.waiting = true;
        self.error.clear();
        s.respond(&text);
        self.ask.required = false;
        self.push(st);
    }

    fn line(&mut self, st: &mut State, id: u64, line: Line) {
        if self.session.as_ref().map(|s| s.id) != Some(id) {
            return;
        }
        let now = crate::util::now_ms();
        match line {
            Line::Prompt { echo, text } => {
                self.ask.prompt = text;
                self.ask.secret = !echo;
                self.ask.required = true;
                self.waiting = false;
            }
            Line::Info(t) | Line::Error(t) => {
                let l = t.to_lowercase();
                if ["fail", "no match", "not match"].iter().any(|w| l.contains(w)) {
                    if now - self.finger_asked_at >= 700 {
                        self.finger_missed = true;
                        let to = Self::send(st);
                        self.miss.start(slint::TimerMode::SingleShot, Duration::from_millis(1600), move || to(Box::new(M::MissOver)));
                    }
                } else if ["place", "swipe", "finger"].iter().any(|w| l.contains(w)) {
                    self.finger_asked_at = now;
                }
                self.ask.supplementary = t;
            }
            Line::Done(true) => return self.finish(st, true),
            Line::Done(false) => {
                // (only a typed password is "incorrect"; then PAM asks again: a new conversation)
                let typed = self.waiting;
                self.waiting = false;
                self.error = if typed { "Incorrect password. Try again.".into() } else { String::new() };
                if typed {
                    self.error_count += 1;
                }
                self.start_session(st);
            }
        }
        self.push(st);
    }
}

impl Part for Polkit {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        // (the real agent only for the shell that named it: tessera-ui run by
        // hand draws every surface, and mustn't become the session's agent)
        if self.real && !(explicitly_asked(&std::env::args().collect::<Vec<_>>(), NAME) && stdin_is_pipe()) {
            self.real = false;
        }
        let test = !self.real && crate::env::var("POLKIT_TEST").as_deref() == Some("1") && Self::fake_helper().is_some();
        if self.real || test {
            agent::spawn(self.real, Self::send(st));
        }
        eprintln!(
            "tessera-ui: polkit: {}",
            if self.real { "the agent, on the system bus" } else if test { "a test agent, on the session bus, with a fake helper" } else { "no agent (previews only)" }
        );
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Request>() {
            Ok(r) => {
                match *r {
                    Request::Begin { message, cookie, user, reply, .. } => {
                        let first = self.queue.is_empty();
                        self.queue.push(Req { message, cookie, user, reply });
                        if first {
                            self.error.clear();
                            self.start_session(st);
                        }
                        self.push(st);
                    }
                    Request::Cancel { cookie } => {
                        if self.queue.first().is_some_and(|r| r.cookie == cookie) {
                            self.finish(st, false);
                        } else if let Some(i) = self.queue.iter().position(|r| r.cookie == cookie) {
                            self.queue.remove(i).reply.send(false);
                        }
                    }
                    Request::Registered(Ok(())) => eprintln!("tessera-ui: polkit: agent registered"),
                    Request::Registered(Err(e)) => eprintln!("tessera-ui: polkit: agent not registered ({e})"),
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<SessionLine>() {
            Ok(l) => return self.line(st, l.0, l.1),
            Err(m) => m,
        };
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["preview"].as_bool() == Some(true) {
                    self.preview = true;
                    self.preview_finger = v["finger"].as_bool().unwrap_or(false);
                    self.error.clear();
                    self.push(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Submit(t) => self.submit(st, t),
            M::Cancel => self.cancel(st),
            M::MissOver => {
                self.finger_missed = false;
                self.push(st);
            }
            M::Gone => {
                if !self.shown() {
                    if let Some(w) = self.win.take() {
                        Glass::clear_now(&w.output, NS);
                        st.drop_surface(&w.surface);
                    }
                }
            }
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
            if self.shown() {
                self.push(st);
            }
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let s = ui.get_body_scale();
        let (bw, bh) = (ui.get_body_w(), ui.get_body_h());
        let x = ui.get_body_x() + bw * (1.0 - s) / 2.0;
        let y = ui.get_body_y() + bh * (1.0 - s) / 2.0;
        let o = ui.get_body_opacity();
        let shape = theme::glass_shape(&st.shell.look, x, y, bw * s, bh * s, 28.0 * s, o, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn quit(&mut self, _: &mut State) {
        // (requests still open are refused: polkit asks the next agent)
        for r in self.queue.drain(..) {
            r.reply.send(false);
        }
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_as_the_qml_shows_them() {
        assert_eq!(message_of("Authentication is required to install software."), "Authentication is required to install software");
        assert_eq!(prompt_of("Password: "), "Password");
        assert_eq!(prompt_of("  "), "Password");
        assert_eq!(prompt_of("PIN:"), "PIN");
    }

    #[test]
    fn a_finger_first() {
        let a = Ask { supplementary: "Place your finger on the fingerprint reader".into(), ..Default::default() };
        assert!(finger_wait(&a, false));
        // (not while a typed password is checked, nor once the field is asked for)
        assert!(!finger_wait(&a, true));
        assert!(!finger_wait(&Ask { required: true, ..a.clone() }, false));
        assert!(!finger_wait(&Ask::default(), false));
    }

    #[test]
    fn only_when_named() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(explicitly_asked(&a(&["ui", "--surfaces", "lock,polkit"]), "polkit"));
        assert!(explicitly_asked(&a(&["ui", "--surfaces=polkit"]), "polkit"));
        assert!(!explicitly_asked(&a(&["ui"]), "polkit"));
        assert!(!explicitly_asked(&a(&["ui", "--surfaces", "lock"]), "polkit"));
    }

    #[test]
    fn never_real_in_a_test() {
        // (the test binary is neither --demo nor told it's a sandbox; what
        // makes it real is only those two, never the environment's buses)
        let p = Polkit::default();
        assert_eq!(p.real, !crate::env::demo_mode() && !crate::env::sandboxed());
    }
}
