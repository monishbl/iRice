//! greetd's IPC (greetd-ipc(7)), the login screen's half: a Unix socket at
//! $GREETD_SOCK, one JSON object each way per message, each preceded by its
//! length as a 32-bit unsigned integer in the machine's byte order. Every
//! request gets exactly one response, in order:
//!   requests   {"type":"create_session","username":…}
//!              {"type":"post_auth_message_response","response":… | null}
//!              {"type":"start_session","cmd":[…],"env":[…]}
//!              {"type":"cancel_session"}
//!   responses  {"type":"success"}
//!              {"type":"error","error_type":"auth_error"|"error","description":…}
//!              {"type":"auth_message","auth_message_type":"visible"|"secret"|"info"|"error","auth_message":…}
//! Quickshell's Greetd (what the QML greeter uses) is the same conversation:
//! an info or error message is answered with a null response at once, a
//! visible or secret one waits for the person; a failure is followed by
//! cancel_session.
//!
//! `Client` writes from the UI thread (requests are small) and reads on a
//! thread of its own, so a response that takes a while (PAM waiting for a
//! finger) never holds the screen up.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

#[derive(Serialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    CreateSession { username: String },
    PostAuthMessageResponse { response: Option<String> },
    StartSession { cmd: Vec<String>, env: Vec<String> },
    CancelSession,
}

// (by hand: a response may carry a password, never printed)
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::CreateSession { username } => write!(f, "CreateSession({username})"),
            Request::PostAuthMessageResponse { response } => write!(f, "PostAuthMessageResponse({})", if response.is_some() { "…" } else { "null" }),
            Request::StartSession { cmd, .. } => write!(f, "StartSession({cmd:?})"),
            Request::CancelSession => write!(f, "CancelSession"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    AuthError,
    Error,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    Visible,
    Secret,
    Info,
    Error,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Success,
    Error {
        error_type: ErrorType,
        #[serde(default)]
        description: String,
    },
    AuthMessage {
        auth_message_type: AuthKind,
        #[serde(default)]
        auth_message: String,
    },
}

/// What the reading thread hears
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Response(Response),
    /// the socket closed or spoke nonsense: greetd is gone
    Closed,
}

/// A request as it goes on the wire
pub fn encode(r: &Request) -> Vec<u8> {
    let body = serde_json::to_vec(r).unwrap_or_default();
    let mut out = (body.len() as u32).to_ne_bytes().to_vec();
    out.extend_from_slice(&body);
    out
}

/// The longest message read (greetd's are a few hundred bytes)
const MAX: usize = 1 << 16;

/// One response off the wire
pub fn read_response(r: &mut impl Read) -> std::io::Result<Response> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_ne_bytes(len) as usize;
    if n > MAX {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "message too long"));
    }
    let mut body = vec![0u8; n];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Connects, and reads responses on a thread of its own, each to `heard`
    /// (then `Event::Closed` once, when the socket ends)
    pub fn connect(path: &Path, heard: impl Fn(Event) + Send + 'static) -> std::io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        let mut reader = stream.try_clone()?;
        std::thread::Builder::new().name("greetd".into()).spawn(move || loop {
            match read_response(&mut reader) {
                Ok(r) => heard(Event::Response(r)),
                Err(_) => {
                    heard(Event::Closed);
                    return;
                }
            }
        })?;
        Ok(Self { stream })
    }

    pub fn send(&mut self, r: &Request) -> std::io::Result<()> {
        self.stream.write_all(&encode(r))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // (the reading thread ends with the socket)
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A fake greetd for tests: one connection, accepts `ada` with the
    /// password `tessera`; `grace` has a fingerprint reader that misses once
    /// before PAM asks for the password. It never runs anything: a
    /// start_session is recorded and answered.
    pub struct Fake {
        pub path: std::path::PathBuf,
        pub started: mpsc::Receiver<Vec<String>>,
        _dir: TempDir,
    }

    pub struct TempDir(pub std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    pub fn temp_dir(tag: &str) -> TempDir {
        let d = std::env::temp_dir().join(format!("tessera-greeter-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }

    // (the fake's own framing, not the client's: the two must agree)
    fn read_req(s: &mut UnixStream) -> Option<Value> {
        let mut len = [0u8; 4];
        s.read_exact(&mut len).ok()?;
        let mut body = vec![0u8; u32::from_ne_bytes(len) as usize];
        s.read_exact(&mut body).ok()?;
        serde_json::from_slice(&body).ok()
    }
    fn reply(s: &mut UnixStream, v: Value) {
        let body = v.to_string().into_bytes();
        let mut out = (body.len() as u32).to_ne_bytes().to_vec();
        out.extend(body);
        s.write_all(&out).unwrap();
    }
    fn auth(kind: &str, msg: &str) -> Value {
        json!({"type": "auth_message", "auth_message_type": kind, "auth_message": msg})
    }

    impl Fake {
        pub fn start(tag: &str) -> Fake {
            let dir = temp_dir(tag);
            let path = dir.0.join("greetd.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let (tx, started) = mpsc::channel();
            std::thread::spawn(move || {
                let Ok((mut s, _)) = listener.accept() else { return };
                // (user, step): the conversation's place
                let mut who: Option<(String, u8)> = None;
                while let Some(req) = read_req(&mut s) {
                    match req["type"].as_str().unwrap_or("") {
                        "create_session" => {
                            if who.is_some() {
                                reply(&mut s, json!({"type": "error", "error_type": "error", "description": "a session is already being configured"}));
                                continue;
                            }
                            let u = req["username"].as_str().unwrap_or("").to_string();
                            match u.as_str() {
                                "ada" => reply(&mut s, auth("secret", "Password: ")),
                                "grace" => reply(&mut s, auth("info", "Place your finger on the fingerprint reader")),
                                "nobody" => {
                                    // (no password at all: ready at once)
                                    reply(&mut s, json!({"type": "success"}));
                                }
                                _ => reply(&mut s, auth("secret", "Password: ")),
                            }
                            who = Some((u, 0));
                        }
                        "post_auth_message_response" => {
                            let Some((u, step)) = who.as_mut() else {
                                reply(&mut s, json!({"type": "error", "error_type": "error", "description": "no session"}));
                                continue;
                            };
                            let resp = req["response"].as_str();
                            if u == "grace" && *step == 0 {
                                *step = 1;
                                reply(&mut s, auth("error", "Failed to match fingerprint"));
                            } else if u == "grace" && *step == 1 {
                                *step = 2;
                                reply(&mut s, auth("secret", "Password: "));
                            } else if (u == "ada" || u == "grace") && resp == Some("tessera") {
                                reply(&mut s, json!({"type": "success"}));
                            } else {
                                reply(&mut s, json!({"type": "error", "error_type": "auth_error", "description": "pam_authenticate: AUTH_ERR"}));
                            }
                        }
                        "start_session" => {
                            let cmd: Vec<String> = req["cmd"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                            let _ = tx.send(cmd);
                            reply(&mut s, json!({"type": "success"}));
                        }
                        "cancel_session" => {
                            who = None;
                            reply(&mut s, json!({"type": "success"}));
                        }
                        _ => reply(&mut s, json!({"type": "error", "error_type": "error", "description": "unknown request"})),
                    }
                }
            });
            Fake { path, started, _dir: dir }
        }
    }

    #[test]
    fn requests_are_greetds_json() {
        let v = |r: Request| -> Value { serde_json::from_slice(&encode(&r)[4..]).unwrap() };
        assert_eq!(v(Request::CreateSession { username: "ada".into() }), json!({"type": "create_session", "username": "ada"}));
        assert_eq!(v(Request::PostAuthMessageResponse { response: None }), json!({"type": "post_auth_message_response", "response": null}));
        assert_eq!(v(Request::PostAuthMessageResponse { response: Some("pw".into()) }), json!({"type": "post_auth_message_response", "response": "pw"}));
        assert_eq!(v(Request::StartSession { cmd: vec!["Hyprland".into()], env: vec![] }), json!({"type": "start_session", "cmd": ["Hyprland"], "env": []}));
        assert_eq!(v(Request::CancelSession), json!({"type": "cancel_session"}));
        // the length leads, in the machine's byte order
        let e = encode(&Request::CancelSession);
        assert_eq!(u32::from_ne_bytes(e[..4].try_into().unwrap()) as usize, e.len() - 4);
        // (a password never shows in a debug print)
        assert!(!format!("{:?}", Request::PostAuthMessageResponse { response: Some("hunter2".into()) }).contains("hunter2"));
    }

    #[test]
    fn responses_are_read_whole() {
        let mut bytes = Vec::new();
        for v in [
            json!({"type": "success"}),
            json!({"type": "error", "error_type": "auth_error", "description": "nope"}),
            json!({"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password: "}),
            json!({"type": "error", "error_type": "error"}),
        ] {
            let b = v.to_string().into_bytes();
            bytes.extend((b.len() as u32).to_ne_bytes());
            bytes.extend(b);
        }
        let mut r = &bytes[..];
        assert_eq!(read_response(&mut r).unwrap(), Response::Success);
        assert_eq!(read_response(&mut r).unwrap(), Response::Error { error_type: ErrorType::AuthError, description: "nope".into() });
        assert_eq!(read_response(&mut r).unwrap(), Response::AuthMessage { auth_message_type: AuthKind::Secret, auth_message: "Password: ".into() });
        assert_eq!(read_response(&mut r).unwrap(), Response::Error { error_type: ErrorType::Error, description: String::new() });
        assert!(read_response(&mut r).is_err());
        // nonsense, or a length past what greetd sends, is an error, never a panic
        let mut bad: &[u8] = &[0xff, 0xff, 0xff, 0x7f, 1, 2];
        assert!(read_response(&mut bad).is_err());
        let mut junk = 5u32.to_ne_bytes().to_vec();
        junk.extend(b"{oops");
        assert!(read_response(&mut &junk[..]).is_err());
    }

    #[test]
    fn the_client_talks_to_a_fake_greetd() {
        let fake = Fake::start("client");
        let (tx, rx) = mpsc::channel();
        let mut c = Client::connect(&fake.path, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let next = || rx.recv_timeout(Duration::from_secs(5)).unwrap();
        c.send(&Request::CreateSession { username: "ada".into() }).unwrap();
        assert_eq!(next(), Event::Response(Response::AuthMessage { auth_message_type: AuthKind::Secret, auth_message: "Password: ".into() }));
        c.send(&Request::PostAuthMessageResponse { response: Some("wrong".into()) }).unwrap();
        assert!(matches!(next(), Event::Response(Response::Error { error_type: ErrorType::AuthError, .. })));
        c.send(&Request::CancelSession).unwrap();
        assert_eq!(next(), Event::Response(Response::Success));
        c.send(&Request::CreateSession { username: "ada".into() }).unwrap();
        next();
        c.send(&Request::PostAuthMessageResponse { response: Some("tessera".into()) }).unwrap();
        assert_eq!(next(), Event::Response(Response::Success));
        c.send(&Request::StartSession { cmd: vec!["Hyprland".into()], env: vec![] }).unwrap();
        assert_eq!(next(), Event::Response(Response::Success));
        assert_eq!(fake.started.recv_timeout(Duration::from_secs(5)).unwrap(), vec!["Hyprland".to_string()]);
        drop(c);
        assert_eq!(next(), Event::Closed);
    }
}
