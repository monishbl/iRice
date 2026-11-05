"""The security surfaces in Rust (native/tessera-ui: lock, session, polkit):
which real role each takes, and when. Their parts' top comments say it in
words; these tests hold the code to it:
  - one session lock at a time, decided as a lock begins and never handed
    over mid-lock; this shell locks again if tessera-ui ends or is refused;
  - the system's PAM (`login`), the reader and the camera only for the real
    shell: never --demo, never a sandbox; a test PAM service only from the
    test's own folder; else a mock;
  - the polkit agent on the system bus only for the real shell that named it;
    a test agent only on the session bus, with a fake helper; the fake
    Authority only on the session bus;
  - a PAM conversation against a test service of our own (pam_exec), never
    against anyone's password (skipped when the binary or pam_exec is missing;
    never with a display: the incident rule).
"""
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "native/tessera-ui"
LOCK = CRATE / "surfaces/lock/src"
POLKIT = CRATE / "surfaces/polkit/src"


def read(p):
    return Path(p).read_text()


class SessionLock(unittest.TestCase):
    def test_one_lock_never_handed_over(self):
        qml = read(ROOT / "shell/modules/lock/Lock.qml")
        # this shell's lock steps aside only for a lock tessera-ui began
        self.assertIn("locked: root.held && !root.rustLock", qml)
        self.assertIn('rustLock = held && RustUi.drawing("lock")', qml)
        # tessera-ui ends, or the compositor refuses it: this shell locks
        self.assertRegex(qml, r"if \(!RustUi\.running\)\s*root\.rustLock = false;")
        self.assertRegex(qml, r"e\.refused === true && root\.rustLock\)[\s\S]{0,120}root\.rustLock = false;")
        # only tessera-ui's own unlock, for its own lock, unlocks
        self.assertIn("e.unlocked === true && root.rustLock", qml)
        # this shell's reader and camera stay off meanwhile
        self.assertRegex(qml, r"fingerWanted: held && !rustLock && !System.sandboxed")
        self.assertRegex(qml, r"faceWanted: held && !rustLock && !System.sandboxed")

    def test_the_qml_lock_never_runs_pam_in_a_sandbox(self):
        # (its PamContext is the system's `login`: in a sandbox that would be
        # your real account's; it's unlocked there with `ipc lock unlock`)
        qml = read(ROOT / "shell/modules/lock/Lock.qml")
        submit = qml[qml.index("function submit()"):qml.index("pam.start();")]
        self.assertIn("if (System.sandboxed) {", submit)
        self.assertIn("return;", submit[submit.index("if (System.sandboxed) {"):])

    def test_rust_takes_the_lock_only_when_told_and_not_in_demo(self):
        src = read(LOCK / "lib.rs")
        # (the session lock is asked for in one place, behind !demo)
        self.assertEqual(src.count("st.lock_session()"), 1)
        self.assertRegex(src, r"fn take\(&mut self, st: &mut State\) \{\s*if !self\.demo \{\s*match st\.lock_session\(\)")
        # taken from the shell's `held` line only
        self.assertEqual(len(re.findall(r"self\.take\(st\)", src)), 1)
        self.assertRegex(src, r"if held \{\s*(eprintln!\([^;]*;\s*)?self\.leaving = false;\s*self\.take\(st\);")
        # let go in one place: release(), from `held: false`
        self.assertEqual(src.count("l.unlock()"), 1)
        self.assertRegex(src, r"\} else \{\s*self\.release\(st\);")
        # the demo draws on overlay layer surfaces (no lock)
        self.assertIn('const NS_DEMO: &str = "tessera:lock";', src)
        self.assertIn("None if self.demo => st.prepare_surface(Spec {", src)
        # quitting never unlocks (the compositor keeps the session locked)
        quit = src[src.index("fn quit(&mut self"):]
        self.assertNotIn(".unlock(", quit.split("\n    }\n")[0])

    def test_the_platform_routes_lock_events_to_the_lock_part_only(self):
        wl = read(CRATE / "core/src/wayland.rs")
        self.assertIn('self.part("lock", |p, st| p.message(st, Box::new(LockEvent::Locked)));', wl)
        self.assertIn('self.part("lock", |p, st| p.message(st, Box::new(LockEvent::Finished)));', wl)


class Pam(unittest.TestCase):
    def test_system_pam_only_when_real(self):
        lock = read(LOCK / "lib.rs")
        auth = read(LOCK / "auth.rs")
        self.assertIn("let real = !demo && !sandboxed();", lock)
        self.assertIn("backend: auth::backend(real)", lock)
        self.assertRegex(auth, r"if real \{\s*return Backend::System;\s*\}")
        # a test service only with both its folder and its name
        self.assertIn('(crate::env::var("LOCK_PAM_DIR"), crate::env::var("LOCK_PAM_SERVICE"))', auth)
        # the reader, the camera and logind: real only
        self.assertIn("self.real && self.held && !self.leaving && !self.sleeping && self.finger_on", lock)
        self.assertIn("self.real && self.held && !self.leaving && !self.standby(st) && self.face_on", lock)
        self.assertIn("if self.real && (self.finger_on || self.face_on) && !self.sleep_watch", lock)
        # faillock only when real
        self.assertRegex(lock, r"fn check_lockout\(&self, st: &State\) \{\s*if !self\.real \{\s*return;")
        # a test never reaches the reader or the camera
        self.assertIn("(Backend::TestService { dir, service }, Kind::Password)", auth)
        self.assertRegex(auth, r"_ => None,\s*\}\s*\}")

    def test_sandboxed_means_the_shells_own_flag(self):
        lock = read(LOCK / "lib.rs")
        self.assertIn('tessera_core::env::sandboxed()', lock)
        self.assertIn('tessera_core::env::demo_mode()', lock)


class Polkit(unittest.TestCase):
    def test_system_agent_only_for_the_real_shell_that_named_it(self):
        src = read(POLKIT / "lib.rs")
        self.assertIn("!crate::env::demo_mode() && !crate::env::sandboxed()", src)
        self.assertIn("explicitly_asked(&std::env::args().collect::<Vec<_>>(), NAME) && stdin_is_pipe()", src)
        # registered once, on the system bus only when real
        self.assertEqual(src.count("agent::spawn("), 1)
        self.assertIn("agent::spawn(self.real, Self::send(st))", src)
        self.assertIn('let test = !self.real && crate::env::var("POLKIT_TEST").as_deref() == Some("1") && Self::fake_helper().is_some();', src)
        # a test runs only its fake helper, the real agent only polkit's
        self.assertIn("let fake = if self.real { None } else { Self::fake_helper() };", src)
        self.assertIn("if !self.real && fake.is_none() {", src)
        agent = read(POLKIT / "agent.rs")
        self.assertIn("if system { zbus::blocking::Connection::system() } else { zbus::blocking::Connection::session() }", agent)
        qml = read(ROOT / "shell/modules/polkit/Polkit.qml")
        self.assertIn("active: !System.sandboxed && !root.inRust", qml)

    def test_the_fake_authority_stays_on_the_session_bus(self):
        src = read(POLKIT / "bin/fake-polkit.rs")
        self.assertIn("Builder::session()", src)
        self.assertNotIn("system()", src)


def binary():
    for kind in ("release", "debug"):
        b = CRATE / "target" / kind / "tessera-ui"
        if b.exists():
            return b
    return None


def pam_exec():
    for d in ("/usr/lib/security", "/lib/security", "/usr/lib64/security", "/lib/x86_64-linux-gnu/security"):
        if Path(d, "pam_exec.so").exists():
            return Path(d, "pam_exec.so")
    return None


@unittest.skipUnless(binary() and pam_exec(), "needs the built tessera-ui and pam_exec")
class TestService(unittest.TestCase):
    # A PAM conversation through `tessera-ui --pam-helper`, against a service
    # of the test's own: pam_exec checks the answer against a word of ours.
    # pam_unix isn't in the stack: no password of anyone's is ever asked.
    ENV = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY")}

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="tessera-pam-"))
        check = self.dir / "check.sh"
        check.write_text('#!/bin/sh\nread -r answer\n[ "$answer" = "tessera-test" ]\n')
        check.chmod(0o755)
        # (a wrong word falls through to pam_deny: an authentication failure,
        # as a wrong password is; pam_exec alone says "system error")
        (self.dir / "tessera-test").write_text(f"auth [success=1 default=ignore] pam_exec.so expose_authtok quiet {check}\nauth requisite pam_deny.so\nauth required pam_permit.so\naccount required pam_permit.so\n")

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def helper(self, answer):
        r = subprocess.run([str(binary()), "--pam-helper", "tessera-test", str(self.dir)], input=answer + "\n", capture_output=True, text=True, timeout=30, env=self.ENV)
        return [l for l in r.stdout.splitlines() if l.startswith("RESULT ")]

    def test_the_right_word(self):
        self.assertEqual([l.split()[1] for l in self.helper("tessera-test")], ["0"])

    def test_a_wrong_word(self):
        res = self.helper("wrong")
        self.assertEqual(len(res), 1)
        # PAM_AUTH_ERR (7): the lock says "Incorrect password", not "couldn't check"
        self.assertEqual(res[0].split()[1], "7")


if __name__ == "__main__":
    unittest.main()
