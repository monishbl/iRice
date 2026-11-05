//! A polkit authentication agent (org.freedesktop.PolicyKit1.AuthenticationAgent)
//! and the conversation for one request (polkit-agent-helper-1's protocol),
//! as libpolkit-agent-1 does for Quickshell's PolkitAgent.
//!
//! The agent serves BeginAuthentication and CancelAuthentication at
//! OBJECT on a bus, and registers with the Authority there
//! (RegisterAuthenticationAgent, for this session). BeginAuthentication
//! returns when the part answers: Ok once the helper said SUCCESS, the
//! Cancelled error when you cancel. Which bus is the part's choice (its top
//! comment): the system bus only for the real agent.
//!
//! The conversation: the helper (polkit's own, run as root by polkit: the
//! socket /run/polkit/agent-helper.socket, else the setuid
//! polkit-agent-helper-1; or a fake one for a test) is given the user and
//! the request's cookie, then speaks lines:
//!   PAM_PROMPT_ECHO_OFF <prompt> | PAM_PROMPT_ECHO_ON <prompt>  (answer a line)
//!   PAM_ERROR_MSG <text> | PAM_TEXT_INFO <text> | SUCCESS | FAILURE
//! On SUCCESS the helper has told polkit itself; the agent only answers.

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::io::{BufRead, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use zbus::zvariant::{OwnedValue, Value};

pub const OBJECT: &str = "/org/tessera/PolicyKit1/AuthenticationAgent";
pub const SOCKET: &str = "/run/polkit/agent-helper.socket";
pub const HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";

type To = Arc<Mutex<Box<dyn Fn(Box<dyn Any + Send>) + Send>>>;

// ── A one-shot answer, without an async runtime of our own ──────────────────

#[derive(Default)]
struct Slot {
    value: Option<bool>,
    waker: Option<Waker>,
}

/// The part's answer to one BeginAuthentication: true authenticated, false cancelled
#[derive(Clone)]
pub struct Reply(Arc<Mutex<Slot>>);

impl Reply {
    pub fn send(&self, ok: bool) {
        if let Ok(mut s) = self.0.lock() {
            if s.value.is_none() {
                s.value = Some(ok);
            }
            if let Some(w) = s.waker.take() {
                w.wake();
            }
        }
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reply")
    }
}

struct Wait(Arc<Mutex<Slot>>);

impl Future for Wait {
    type Output = bool;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let mut s = self.0.lock().unwrap();
        match s.value {
            Some(v) => Poll::Ready(v),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub fn oneshot() -> (Reply, impl Future<Output = bool>) {
    let s = Arc::new(Mutex::new(Slot::default()));
    (Reply(s.clone()), Wait(s))
}

// ── The agent ───────────────────────────────────────────────────────────────

/// What the agent hears, for the part
#[derive(Debug)]
pub enum Request {
    Begin { action: String, message: String, cookie: String, user: String, reply: Reply },
    Cancel { cookie: String },
    /// registered with the Authority (or why not)
    Registered(Result<(), String>),
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
pub enum PkError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Cancelled(String),
    Failed(String),
}

struct Agent {
    to: To,
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    #[allow(clippy::too_many_arguments)]
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> Result<(), PkError> {
        let (reply, wait) = oneshot();
        let user = pick_user(&identities);
        if let Ok(to) = self.to.lock() {
            to(Box::new(Request::Begin { action: action_id, message, cookie, user, reply }));
        }
        if wait.await {
            Ok(())
        } else {
            Err(PkError::Cancelled("The authentication was cancelled".into()))
        }
    }

    async fn cancel_authentication(&self, cookie: String) {
        if let Ok(to) = self.to.lock() {
            to(Box::new(Request::Cancel { cookie }));
        }
    }
}

/// Who to ask about: you, when you're among the identities polkit offers;
/// else the first user it names (an admin)
pub fn pick_user(ids: &[(String, HashMap<String, OwnedValue>)]) -> String {
    let me = unsafe { libc::getuid() };
    let uids: Vec<u32> = ids
        .iter()
        .filter(|(kind, _)| kind == "unix-user")
        .filter_map(|(_, d)| d.get("uid").and_then(|v| match &**v {
            Value::U32(u) => Some(*u),
            Value::I32(i) => Some(*i as u32),
            _ => None,
        }))
        .collect();
    let uid = if uids.contains(&me) || uids.is_empty() { me } else { uids[0] };
    user_of(uid)
}

pub fn user_of(uid: u32) -> String {
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() || (*pw).pw_name.is_null() {
            return std::env::var("USER").unwrap_or_default();
        }
        std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
    }
}

/// Serve the agent on `system` (else the session bus) and register it;
/// tries again a few times while another agent still holds this session
pub fn spawn(system: bool, to: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
    let to: To = Arc::new(Mutex::new(Box::new(to)));
    std::thread::spawn(move || {
        let say = |r: Result<(), String>| {
            if let Ok(t) = to.lock() {
                t(Box::new(Request::Registered(r)));
            }
        };
        let conn = match if system { zbus::blocking::Connection::system() } else { zbus::blocking::Connection::session() } {
            Ok(c) => c,
            Err(e) => return say(Err(e.to_string())),
        };
        if let Err(e) = conn.object_server().at(OBJECT, Agent { to: to.clone() }) {
            return say(Err(e.to_string()));
        }
        let subject: (String, HashMap<String, Value>) = match std::env::var("XDG_SESSION_ID").ok().filter(|s| !s.is_empty()) {
            Some(id) => ("unix-session".into(), HashMap::from([("session-id".to_string(), Value::from(id))])),
            None => {
                let pid = std::process::id();
                let start: u64 = std::fs::read_to_string("/proc/self/stat").ok().and_then(|s| s.rsplit(')').next().and_then(|r| r.split_whitespace().nth(19)).and_then(|v| v.parse().ok())).unwrap_or(0);
                ("unix-process".into(), HashMap::from([("pid".to_string(), Value::U32(pid)), ("start-time".to_string(), Value::U64(start))]))
            }
        };
        let locale = std::env::var("LANG").unwrap_or_else(|_| "C".into());
        let mut last = String::new();
        for attempt in 0..10 {
            let r = conn.call_method(
                Some("org.freedesktop.PolicyKit1"),
                "/org/freedesktop/PolicyKit1/Authority",
                Some("org.freedesktop.PolicyKit1.Authority"),
                "RegisterAuthenticationAgent",
                &(&subject, locale.as_str(), OBJECT),
            );
            match r {
                Ok(_) => {
                    say(Ok(()));
                    // (served for as long as this process lives)
                    loop {
                        std::thread::park();
                    }
                }
                Err(e) => last = e.to_string(),
            }
            // (the shell's own agent letting go of this session, a moment)
            std::thread::sleep(std::time::Duration::from_millis(500 * (attempt + 1)));
        }
        say(Err(last));
    });
}

// ── One conversation with the helper ────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Prompt { echo: bool, text: String },
    Info(String),
    Error(String),
    Done(bool),
}

/// A helper's line, parsed
pub fn parse_line(l: &str) -> Option<Line> {
    let l = l.trim_end_matches(['\n', '\r']);
    if l == "SUCCESS" {
        return Some(Line::Done(true));
    }
    if l == "FAILURE" {
        return Some(Line::Done(false));
    }
    let (word, rest) = l.split_once(' ').unwrap_or((l, ""));
    Some(match word {
        "PAM_PROMPT_ECHO_OFF" => Line::Prompt { echo: false, text: rest.to_string() },
        "PAM_PROMPT_ECHO_ON" => Line::Prompt { echo: true, text: rest.to_string() },
        "PAM_ERROR_MSG" => Line::Error(rest.to_string()),
        "PAM_TEXT_INFO" => Line::Info(rest.to_string()),
        _ => return None,
    })
}

/// A conversation's news for the part (`id` tells it from an older one's)
#[derive(Debug)]
pub struct SessionLine(pub u64, pub Line);

/// A running conversation: answer prompts with `respond`, end it with `stop`
pub struct Session {
    pub id: u64,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    child: Option<std::process::Child>,
    stream: Option<std::os::unix::net::UnixStream>,
}

impl Session {
    pub fn respond(&self, text: &str) {
        if let Ok(mut w) = self.writer.lock() {
            if let Some(w) = w.as_mut() {
                let _ = w.write_all(format!("{text}\n").as_bytes());
                let _ = w.flush();
            }
        }
    }
    pub fn stop(&mut self) {
        if let Some(c) = self.child.as_mut() {
            let _ = c.kill();
        }
        if let Some(s) = &self.stream {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        if let Ok(mut w) = self.writer.lock() {
            *w = None;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
        if let Some(mut c) = self.child.take() {
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
    }
}

/// Start the helper for `user` and `cookie`. `helper`: Some(fake) for a
/// test (never the real one then); None for polkit's own (socket, else the
/// setuid helper)
pub fn converse(id: u64, user: &str, cookie: &str, helper: Option<&str>, to: impl Fn(Box<dyn Any + Send>) + Send + 'static) -> std::io::Result<Session> {
    let mut child = None;
    let mut stream = None;
    let (reader, writer): (Box<dyn std::io::Read + Send>, Box<dyn Write + Send>) = match helper {
        None if std::path::Path::new(SOCKET).exists() => match std::os::unix::net::UnixStream::connect(SOCKET) {
            Ok(mut s) => {
                s.write_all(format!("{user}\n{cookie}\n").as_bytes())?;
                let r = s.try_clone()?;
                let w = s.try_clone()?;
                stream = Some(s);
                (Box::new(r), Box::new(w))
            }
            Err(_) => spawn_helper(HELPER, user, cookie, &mut child)?,
        },
        None => spawn_helper(HELPER, user, cookie, &mut child)?,
        Some(fake) => spawn_helper(fake, user, cookie, &mut child)?,
    };
    std::thread::spawn(move || {
        let mut done = false;
        for l in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
            if let Some(line) = parse_line(&l) {
                done |= matches!(line, Line::Done(_));
                to(Box::new(SessionLine(id, line)));
                if done {
                    break;
                }
            }
        }
        if !done {
            to(Box::new(SessionLine(id, Line::Done(false))));
        }
    });
    Ok(Session { id, writer: Arc::new(Mutex::new(Some(writer))), child, stream })
}

type Pipes = (Box<dyn std::io::Read + Send>, Box<dyn Write + Send>);

fn spawn_helper(path: &str, user: &str, cookie: &str, child: &mut Option<std::process::Child>) -> std::io::Result<Pipes> {
    use std::process::{Command, Stdio};
    let mut c = Command::new(path).arg(user).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let mut stdin = c.stdin.take().ok_or_else(|| std::io::Error::other("no stdin"))?;
    let stdout = c.stdout.take().ok_or_else(|| std::io::Error::other("no stdout"))?;
    stdin.write_all(format!("{cookie}\n").as_bytes())?;
    *child = Some(c);
    Ok((Box::new(stdout), Box::new(stdin)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_lines() {
        assert_eq!(parse_line("PAM_PROMPT_ECHO_OFF Password: "), Some(Line::Prompt { echo: false, text: "Password: ".into() }));
        assert_eq!(parse_line("PAM_PROMPT_ECHO_ON Name:"), Some(Line::Prompt { echo: true, text: "Name:".into() }));
        assert_eq!(parse_line("PAM_TEXT_INFO Place your finger on the reader\n"), Some(Line::Info("Place your finger on the reader".into())));
        assert_eq!(parse_line("PAM_ERROR_MSG Sorry"), Some(Line::Error("Sorry".into())));
        assert_eq!(parse_line("SUCCESS"), Some(Line::Done(true)));
        assert_eq!(parse_line("FAILURE"), Some(Line::Done(false)));
        assert_eq!(parse_line("nonsense"), None);
    }

    #[test]
    fn answers_wake_the_waiter() {
        let (reply, wait) = oneshot();
        reply.send(true);
        reply.send(false); // (only the first answer counts)
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut f = Box::pin(wait);
        assert_eq!(f.as_mut().poll(&mut cx), Poll::Ready(true));
    }

    #[test]
    fn a_fake_helper_converses() {
        // (a fake of polkit's helper: never the real one in a test)
        let dir = std::env::temp_dir().join(format!("tessera-pk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("helper.sh");
        std::fs::write(&fake, "#!/bin/sh\nread cookie\necho \"PAM_TEXT_INFO hello $1\"\necho 'PAM_PROMPT_ECHO_OFF Password: '\nread pw\n[ \"$pw\" = secret ] && [ \"$cookie\" = c1 ] && echo SUCCESS || echo FAILURE\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Line>();
        let s = converse(7, "sam", "c1", Some(fake.to_str().unwrap()), move |b| {
            let l = b.downcast::<SessionLine>().unwrap();
            assert_eq!(l.0, 7);
            let _ = tx.send(l.1);
        })
        .unwrap();
        let t = std::time::Duration::from_secs(5);
        assert_eq!(rx.recv_timeout(t).unwrap(), Line::Info("hello sam".into()));
        assert!(matches!(rx.recv_timeout(t).unwrap(), Line::Prompt { echo: false, .. }));
        s.respond("secret");
        assert_eq!(rx.recv_timeout(t).unwrap(), Line::Done(true));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
