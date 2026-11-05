//! PAM for the lock: the password, the fingerprint reader and the face
//! camera, each its own conversation (Lock.qml's three PamContexts).
//!
//! A conversation runs in a child process, this binary again as
//! `tessera-ui --pam-helper SERVICE [CONFDIR]` (`helper_main`): PAM then runs
//! single-threaded, away from the UI, and stopping one (the reader, when
//! the lock goes or the computer sleeps) is a kill, as Quickshell's
//! PamContext.abort is. The helper reads one line on stdin (the answer to
//! any prompt: the password, or nothing for the reader and the camera) and
//! writes one line per PAM message, then its result:
//!   INFO <text> | ERROR <text> | PROMPT <text> | RESULT <code> <text>
//!
//! Which PAM, by `Backend` (the lock's top comment says when each is used):
//!   System       the system's `login` service (/etc/pam.d), as Quickshell's
//!                PamContext defaults to; tessera-fingerprint and tessera-face
//!                from shell/assets/pam
//!   TestService  a service of the test's own from its own folder
//!                (pam_start_confdir: TESSERA_LOCK_PAM_DIR and
//!                TESSERA_LOCK_PAM_SERVICE), never the system's files
//!   Mock         no PAM at all: accepts TESSERA_LOCK_TEST_PASSWORD (when
//!                set) after the moment a real check takes

use std::any::Any;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::io::{BufRead, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum Backend {
    System,
    TestService { dir: String, service: String },
    Mock { password: Option<String> },
}

/// The backend: the system's PAM only for the real lock; a test service when
/// one is named; else the mock
pub fn backend(real: bool) -> Backend {
    if real {
        return Backend::System;
    }
    match (crate::env::var("LOCK_PAM_DIR"), crate::env::var("LOCK_PAM_SERVICE")) {
        (Some(dir), Some(service)) if !dir.is_empty() && !service.is_empty() => Backend::TestService { dir, service },
        _ => Backend::Mock { password: crate::env::var("LOCK_TEST_PASSWORD").filter(|p| !p.is_empty()) },
    }
}

/// What a conversation is for
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Password,
    Finger,
    Face,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Success,
    /// PAM said no (wrong password, a finger that didn't match, a timeout)
    Failure(String),
    /// it couldn't ask (PAM itself failed, the helper couldn't start)
    Error(String),
}

/// A conversation's news, to the lock part (its `message`)
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Info(Kind, u64, String),
    Error(Kind, u64, String),
    Prompt(Kind, u64, String),
    Done(Kind, u64, Outcome),
}

/// One running conversation (`id` tells its events from a stopped one's)
pub struct Conv {
    pub kind: Kind,
    pub id: u64,
    child: Arc<Mutex<Option<Child>>>,
}

impl Conv {
    /// Stop it (its Done still comes, as an Error, or not at all)
    pub fn abort(&self) {
        if let Ok(mut c) = self.child.lock() {
            if let Some(ch) = c.as_mut() {
                let _ = ch.kill();
            }
        }
    }
}

impl Drop for Conv {
    fn drop(&mut self) {
        self.abort();
    }
}

/// The service and folder a conversation of this kind uses (None: the
/// mock answers)
pub fn service(b: &Backend, kind: Kind) -> Option<(String, Option<String>)> {
    let shell_pam = || crate::util::asset("shell/assets/pam").map(|p| p.to_string_lossy().into_owned());
    match (b, kind) {
        (Backend::System, Kind::Password) => Some(("login".into(), None)),
        (Backend::System, Kind::Finger) => Some(("tessera-fingerprint".into(), shell_pam())),
        (Backend::System, Kind::Face) => Some(("tessera-face".into(), shell_pam())),
        (Backend::TestService { dir, service }, Kind::Password) => Some((service.clone(), Some(dir.clone()))),
        _ => None,
    }
}

/// Start a conversation; its events go to `to`
pub fn start(b: &Backend, kind: Kind, id: u64, secret: String, to: impl Fn(Box<dyn Any + Send>) + Send + Clone + 'static) -> Conv {
    let child = Arc::new(Mutex::new(None));
    let conv = Conv { kind, id, child: child.clone() };
    let Some((service, dir)) = service(b, kind) else {
        // The mock: a moment, then yes for the test password, no otherwise
        let ok = matches!(b, Backend::Mock { password: Some(p) } if *p == secret) && kind == Kind::Password;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(700));
            let out = if ok { Outcome::Success } else if kind == Kind::Password { Outcome::Failure("Authentication failure".into()) } else { Outcome::Error("no reader here".into()) };
            to(Box::new(Event::Done(kind, id, out)));
        });
        return conv;
    };
    let exe = std::env::current_exe().unwrap_or_else(|_| "/proc/self/exe".into());
    let mut cmd = Command::new(exe);
    cmd.arg("--pam-helper").arg(&service);
    if let Some(d) = &dir {
        cmd.arg(d);
    }
    let spawned = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    let mut ch = match spawned {
        Ok(c) => c,
        Err(e) => {
            let msg = e.to_string();
            std::thread::spawn(move || to(Box::new(Event::Done(kind, id, Outcome::Error(msg)))));
            return conv;
        }
    };
    if let Some(mut stdin) = ch.stdin.take() {
        let mut line = secret.into_bytes();
        line.push(b'\n');
        let _ = stdin.write_all(&line);
        line.iter_mut().for_each(|b| *b = 0);
    }
    let stdout = ch.stdout.take();
    *child.lock().unwrap() = Some(ch);
    std::thread::spawn(move || {
        let mut result = None;
        if let Some(out) = stdout {
            for l in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                let (word, rest) = l.split_once(' ').unwrap_or((l.as_str(), ""));
                match word {
                    "INFO" => to(Box::new(Event::Info(kind, id, rest.to_string()))),
                    "ERROR" => to(Box::new(Event::Error(kind, id, rest.to_string()))),
                    "PROMPT" => to(Box::new(Event::Prompt(kind, id, rest.to_string()))),
                    "RESULT" => result = Some(parse_result(rest)),
                    _ => {}
                }
            }
        }
        if let Ok(mut c) = child.lock() {
            if let Some(mut ch) = c.take() {
                let _ = ch.wait();
            }
        }
        to(Box::new(Event::Done(kind, id, result.unwrap_or_else(|| Outcome::Error("stopped".into())))));
    });
    conv
}

/// "<code> <text>" from the helper
pub fn parse_result(rest: &str) -> Outcome {
    let (code, text) = rest.split_once(' ').unwrap_or((rest, ""));
    match code.parse::<i32>() {
        Ok(PAM_SUCCESS) => Outcome::Success,
        // (asking failed: PAM's own trouble, not a wrong answer)
        Ok(PAM_ABORT) | Ok(PAM_SYSTEM_ERR) | Ok(PAM_BUF_ERR) | Ok(PAM_CONV_ERR) | Ok(PAM_SERVICE_ERR) | Ok(PAM_AUTHINFO_UNAVAIL) => Outcome::Error(text.to_string()),
        Ok(_) => Outcome::Failure(text.to_string()),
        Err(_) => Outcome::Error(rest.to_string()),
    }
}

// ── The helper: PAM itself ──────────────────────────────────────────────────

const PAM_SUCCESS: i32 = 0;
const PAM_SERVICE_ERR: i32 = 3;
const PAM_SYSTEM_ERR: i32 = 4;
const PAM_BUF_ERR: i32 = 5;
const PAM_AUTHINFO_UNAVAIL: i32 = 9;
const PAM_CONV_ERR: i32 = 19;
const PAM_ABORT: i32 = 26;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

#[repr(C)]
struct PamConv {
    conv: ConvFn,
    appdata_ptr: *mut c_void,
}

#[link(name = "pam")]
extern "C" {
    fn pam_start(service: *const c_char, user: *const c_char, conv: *const PamConv, pamh: *mut *mut c_void) -> c_int;
    fn pam_start_confdir(service: *const c_char, user: *const c_char, conv: *const PamConv, confdir: *const c_char, pamh: *mut *mut c_void) -> c_int;
    fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut c_void, status: c_int) -> c_int;
    fn pam_strerror(pamh: *mut c_void, errnum: c_int) -> *const c_char;
}

fn say(word: &str, text: &str) {
    let mut out = std::io::stdout().lock();
    // (one line each: a message with newlines is joined)
    let _ = writeln!(out, "{word} {}", text.replace('\n', " "));
    let _ = out.flush();
}

extern "C" fn conversation(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
    if n <= 0 || msgs.is_null() || resp.is_null() {
        return PAM_CONV_ERR;
    }
    let secret = unsafe { &*(data as *const CString) };
    let out = unsafe { libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse };
    if out.is_null() {
        return PAM_BUF_ERR;
    }
    for i in 0..n as usize {
        let m = unsafe { &**msgs.add(i) };
        let text = if m.msg.is_null() { String::new() } else { unsafe { CStr::from_ptr(m.msg) }.to_string_lossy().into_owned() };
        match m.msg_style {
            PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => {
                say("PROMPT", &text);
                unsafe { (*out.add(i)).resp = libc::strdup(secret.as_ptr()) };
            }
            PAM_ERROR_MSG => say("ERROR", &text),
            PAM_TEXT_INFO => say("INFO", &text),
            _ => {}
        }
    }
    unsafe { *resp = out };
    PAM_SUCCESS
}

/// The current user's name (the one the lock unlocks)
pub fn user_name() -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_name.is_null() {
            return std::env::var("USER").unwrap_or_default();
        }
        CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
    }
}

/// `tessera-ui --pam-helper SERVICE [CONFDIR]`: one conversation for the
/// current user (stdin: the answer to prompts; stdout: the lines above)
pub fn helper_main(args: &[String]) -> i32 {
    let Some(service) = args.get(2) else { return 2 };
    let dir = args.get(3);
    let mut line = Vec::new();
    let _ = std::io::stdin().lock().take(4096).read_until(b'\n', &mut line);
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    let secret = CString::new(std::mem::take(&mut line)).unwrap_or_default();
    let (Ok(svc), Ok(user)) = (CString::new(service.as_str()), CString::new(user_name())) else { return 2 };
    let conv = PamConv { conv: conversation, appdata_ptr: &secret as *const CString as *mut c_void };
    let mut h: *mut c_void = std::ptr::null_mut();
    let r = unsafe {
        match dir {
            Some(d) => {
                let d = CString::new(d.as_str()).unwrap_or_default();
                pam_start_confdir(svc.as_ptr(), user.as_ptr(), &conv, d.as_ptr(), &mut h)
            }
            None => pam_start(svc.as_ptr(), user.as_ptr(), &conv, &mut h),
        }
    };
    if r != PAM_SUCCESS || h.is_null() {
        say("RESULT", &format!("{r} pam_start failed"));
        return 1;
    }
    let mut r = unsafe { pam_authenticate(h, 0) };
    if r == PAM_SUCCESS {
        r = unsafe { pam_acct_mgmt(h, 0) };
    }
    let text = unsafe {
        let p = pam_strerror(h, r);
        if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
    };
    unsafe { pam_end(h, r) };
    // (the answer, overwritten before it goes back to the allocator)
    let mut bytes = secret.into_bytes();
    bytes.iter_mut().for_each(|b| *b = 0);
    say("RESULT", &format!("{r} {text}"));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_from_the_helper() {
        assert_eq!(parse_result("0 Success"), Outcome::Success);
        assert_eq!(parse_result("7 Authentication failure"), Outcome::Failure("Authentication failure".into()));
        assert_eq!(parse_result("9 Authentication service cannot retrieve authentication info"), Outcome::Error("Authentication service cannot retrieve authentication info".into()));
        assert!(matches!(parse_result("nonsense"), Outcome::Error(_)));
    }

    #[test]
    fn services_by_backend() {
        assert_eq!(service(&Backend::System, Kind::Password), Some(("login".into(), None)));
        let t = Backend::TestService { dir: "/tmp/pam".into(), service: "tessera-test".into() };
        assert_eq!(service(&t, Kind::Password), Some(("tessera-test".into(), Some("/tmp/pam".into()))));
        // (a test never reaches the reader or the camera)
        assert_eq!(service(&t, Kind::Finger), None);
        assert_eq!(service(&Backend::Mock { password: None }, Kind::Password), None);
    }

    #[test]
    fn the_mock_says_yes_only_to_its_password() {
        let (tx, rx) = std::sync::mpsc::channel::<Event>();
        let to = move |b: Box<dyn Any + Send>| {
            let _ = tx.send(*b.downcast::<Event>().unwrap());
        };
        let m = Backend::Mock { password: Some("tessera".into()) };
        let _a = start(&m, Kind::Password, 1, "tessera".into(), to.clone());
        let _b = start(&m, Kind::Password, 2, "wrong".into(), to.clone());
        let _c = start(&Backend::Mock { password: None }, Kind::Password, 3, String::new(), to);
        let mut got: Vec<Event> = (0..3).map(|_| rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()).collect();
        got.sort_by_key(|e| if let Event::Done(_, id, _) = e { *id } else { 0 });
        assert_eq!(got[0], Event::Done(Kind::Password, 1, Outcome::Success));
        assert!(matches!(got[1], Event::Done(Kind::Password, 2, Outcome::Failure(_))));
        assert!(matches!(got[2], Event::Done(Kind::Password, 3, Outcome::Failure(_))));
    }

    #[test]
    fn never_the_system_unless_real() {
        // (whatever the environment says, only the real lock uses the system's PAM)
        assert_eq!(backend(true), Backend::System);
        assert_ne!(backend(false), Backend::System);
    }
}
