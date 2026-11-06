//! The standalone shell's IPC (what `qs ipc call <target> <fn> …` is to the QML
//! shell): a Unix socket the host listens on, and the client
//! `tessera-ui ipc call <target> <fn> [args…]` that keybinds and scripts run.
//!
//! The socket is $XDG_RUNTIME_DIR/tessera/ipc.sock (the folder 0700, the
//! socket 0600). A request is one line, a JSON array of the strings after
//! `ipc` (`["call","panels","toggle","spotlight"]`, the form shell-ipc.sh
//! passes to `qs ipc call`); it is routed with
//! tessera_core::host::route_ipc(args, Some(reply)) to the service that owns
//! the target, and what that service answers is written back (then the
//! connection closes). The client prints the answer (like `qs ipc call`
//! prints a function's return value) and exits 0; 1 when no shell answers.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tessera_core::host::{Host, Service};

/// $XDG_RUNTIME_DIR/tessera/ipc.sock (TESSERA_IPC_SOCKET names another, for tests and sandboxes)
pub fn socket_path() -> PathBuf {
    if let Some(p) = tessera_core::env::var("IPC_SOCKET").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    crate::common::runtime_dir().join("tessera/ipc.sock")
}

/// What a request carries: the strings after `ipc`, with a leading `call` dropped
/// (`qs ipc call target fn args…`; `target fn args…` also works)
pub fn parse_request(line: &str) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let mut args: Vec<String> = v.as_array()?.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect();
    if args.first().is_some_and(|a| a == "call") {
        args.remove(0);
    }
    (!args.is_empty()).then_some(args)
}

/// The request line for the strings after `ipc`
pub fn request_line(args: &[String]) -> String {
    format!("{}\n", serde_json::to_string(args).unwrap_or_else(|_| "[]".into()))
}

/// The IPC targets this shell answers (`ipc show`)
pub const TARGETS: &[&str] = &["panels", "shell", "rustui", "island", "cooldown", "conflicts", "lock", "loginIntro", "osd", "polkit", "welcome", "shortcut"];

/// One connection: read the request, route it, wait for the answer, write it
fn serve(mut s: UnixStream) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut line = String::new();
    if BufReader::new(&s).take(1 << 20).read_line(&mut line).is_err() {
        return;
    }
    let reply = match parse_request(&line) {
        None => "tessera-ui ipc: expected a JSON array of arguments\n".to_string(),
        Some(args) if args[0] == "show" => TARGETS.iter().map(|t| format!("target {t}\n")).collect(),
        Some(args) => {
            let (tx, rx) = std::sync::mpsc::channel::<String>();
            if !tessera_core::host::route_ipc(&args, Some(tx)) {
                "tessera-ui ipc: the shell is not running standalone\n".to_string()
            } else {
                // (a service answers within the loop's turn; no answer for 10 s: the call is done anyway)
                match rx.recv_timeout(Duration::from_secs(10)) {
                    Ok(a) if a.is_empty() => String::new(),
                    Ok(a) if a.ends_with('\n') => a,
                    Ok(a) => a + "\n",
                    Err(_) => String::new(),
                }
            }
        }
    };
    let _ = s.write_all(reply.as_bytes());
}

/// Listen on the socket (a stale one from a shell that died is replaced; one
/// that answers is not: a second shell would fight the first)
pub fn listen(path: &Path) -> std::io::Result<UnixListener> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "another tessera-ui answers on the IPC socket"));
        }
        let _ = std::fs::remove_file(path);
    }
    let l = UnixListener::bind(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    Ok(l)
}

#[derive(Default)]
pub struct IpcServer {
    path: Option<PathBuf>,
}

impl Service for IpcServer {
    fn name(&self) -> &'static str {
        "Ipc"
    }
    fn start(&mut self, _h: &mut Host) {
        let path = socket_path();
        match listen(&path) {
            Ok(l) => {
                self.path = Some(path);
                std::thread::spawn(move || {
                    for s in l.incoming().flatten() {
                        // (a thread each: a slow client never holds up the next)
                        std::thread::spawn(move || serve(s));
                    }
                });
            }
            Err(e) => eprintln!("tessera-ui: ipc: no socket at {}: {e}", path.display()),
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Ask a socket: the request, the whole answer
pub fn ask(path: &Path, args: &[String]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_secs(15)))?;
    s.write_all(request_line(args).as_bytes())?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    Ok(out)
}

/// `tessera-ui ipc …` (args after "ipc"): the exit code
pub fn client(args: &[String]) -> i32 {
    let mut args: Vec<String> = args.to_vec();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        eprintln!("usage: tessera-ui ipc call <target> <function> [arguments…]\n       tessera-ui ipc show");
        return 2;
    }
    if args[0] != "call" && args[0] != "show" {
        args.insert(0, "call".into());
    }
    if args[0] == "call" && args.len() < 3 {
        eprintln!("tessera-ui ipc: call needs a target and a function");
        return 2;
    }
    match ask(&socket_path(), &args) {
        Ok(out) => {
            print!("{out}");
            let _ = std::io::stdout().flush();
            0
        }
        Err(e) => {
            eprintln!("tessera-ui ipc: no Tessera shell is running ({e})");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_json_arrays_with_or_without_call() {
        assert_eq!(parse_request(r#"["call","panels","toggle","spotlight"]"#), Some(vec!["panels".into(), "toggle".into(), "spotlight".into()]));
        assert_eq!(parse_request(r#" ["lock","lock"] "#), Some(vec!["lock".into(), "lock".into()]));
        // (numbers and booleans arrive as their text)
        assert_eq!(parse_request(r#"["island","hot",true]"#), Some(vec!["island".into(), "hot".into(), "true".into()]));
        for bad in ["", "panels toggle", "{}", "[]", r#"["call"]"#] {
            assert_eq!(parse_request(bad), None, "{bad}");
        }
    }

    #[test]
    fn request_lines_round_trip_awkward_arguments() {
        let args: Vec<String> = ["call", "panels", "shareFiles", "/a b\n/c\"d"].iter().map(|s| s.to_string()).collect();
        let line = request_line(&args);
        assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
        assert_eq!(parse_request(&line).unwrap(), args[1..].to_vec());
    }

    #[test]
    fn the_client_gets_the_servers_answer() {
        let dir = std::env::temp_dir().join(format!("tessera-ipc-test-{}", std::process::id()));
        let path = dir.join("ipc.sock");
        let l = listen(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let t = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&s).read_line(&mut line).unwrap();
            let args = parse_request(&line).unwrap();
            s.write_all(format!("got {}\n", args.join(" ")).as_bytes()).unwrap();
        });
        let out = ask(&path, &["call".into(), "shell".into(), "ready".into()]).unwrap();
        assert_eq!(out, "got shell ready\n");
        t.join().unwrap();
        // a second shell does not take the socket of one that answers
        let l2 = listen(&path);
        assert!(l2.is_ok(), "(the first is gone: the stale socket is replaced)");
        drop(l2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nobody_listening_is_an_error() {
        let p = std::env::temp_dir().join("tessera-ipc-nobody-here.sock");
        assert!(ask(&p, &["call".into(), "a".into(), "b".into()]).is_err());
    }
}
