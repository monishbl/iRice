"""The login screen in Rust (tessera-ui --greeter, native/tessera-ui/
surfaces/greeter, the tessera-greeter crate) beside the QML one (shell/modules/greeter):
  - the sandbox's fake greetd speaks greetd's protocol (the crate's own
    tests drive the Rust client and the login logic against a fake of
    their own; this one is for trying either greeter in the arena);
  - the Rust greeter words things as the QML one does;
  - without a display it says so and ends, never touching anyone's session
    (an empty runtime folder: no Wayland socket to find).
Nothing here talks to a real greetd or to a real account."""
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "native/tessera-ui"
FAKE = CRATE / "tools/fake-greetd.py"
GREETER = CRATE / "surfaces/greeter/src"


def send(s, obj):
    body = json.dumps(obj).encode()
    s.sendall(struct.pack("=I", len(body)) + body)


def recv(s):
    head = s.recv(4, socket.MSG_WAITALL)
    (n,) = struct.unpack("=I", head)
    return json.loads(s.recv(n, socket.MSG_WAITALL))


class FakeGreetd(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="tessera-fake-greetd-")
        self.addCleanup(self.dir.cleanup)
        self.sock = os.path.join(self.dir.name, "greetd.sock")
        self.log = os.path.join(self.dir.name, "started.jsonl")
        self.p = subprocess.Popen([sys.executable, "-I", str(FAKE), self.sock, "--finger", "0.2", "--log", self.log], stderr=subprocess.DEVNULL)
        self.addCleanup(self.p.kill)
        for _ in range(100):
            if os.path.exists(self.sock):
                break
            time.sleep(0.05)
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(self.sock)
        self.addCleanup(self.s.close)

    def ask(self, obj):
        send(self.s, obj)
        return recv(self.s)

    def test_a_password_then_the_session(self):
        self.assertEqual(self.ask({"type": "create_session", "username": "ada"})["auth_message_type"], "secret")
        r = self.ask({"type": "post_auth_message_response", "response": "wrong"})
        self.assertEqual((r["type"], r["error_type"]), ("error", "auth_error"))
        self.assertEqual(self.ask({"type": "cancel_session"}), {"type": "success"})
        self.ask({"type": "create_session", "username": "ada"})
        self.assertEqual(self.ask({"type": "post_auth_message_response", "response": "tessera"}), {"type": "success"})
        self.assertEqual(self.ask({"type": "start_session", "cmd": ["Hyprland"], "env": []}), {"type": "success"})
        self.assertEqual(json.loads(Path(self.log).read_text())["cmd"], ["Hyprland"])

    def test_the_fingerprint_reader_then_the_password(self):
        r = self.ask({"type": "create_session", "username": "grace"})
        self.assertEqual(r["auth_message_type"], "info")
        self.assertIn("finger", r["auth_message"])
        r = self.ask({"type": "post_auth_message_response", "response": None})
        self.assertEqual(r["auth_message_type"], "error")
        self.assertEqual(self.ask({"type": "post_auth_message_response", "response": None})["auth_message_type"], "secret")
        self.assertEqual(self.ask({"type": "post_auth_message_response", "response": "tessera"}), {"type": "success"})

    def test_strangers_are_refused(self):
        self.ask({"type": "create_session", "username": "root"})
        self.assertEqual(self.ask({"type": "post_auth_message_response", "response": "tessera"})["error_type"], "auth_error")


class SameWords(unittest.TestCase):
    def test_rust_says_what_the_qml_says(self):
        rust = "\n".join(p.read_text() for p in GREETER.glob("*.rs"))
        qml = "\n".join(p.read_text() for p in (ROOT / "shell/modules/greeter").glob("*.qml")) + (ROOT / "material/modules/greeter/GreeterSurface.qml").read_text()
        for words in ("Incorrect password. Try again.", "Type your username first.", "Login failed. Try again.",
                      "This session's launch command isn't supported. Choose another with Tab.",
                      "Preview: the password is tessera. Esc quits.", "Preview: the password is tessera. Esc closes it.",
                      "preview: the password is tessera, Esc quits", "Use Fingerprint or Enter Password", "Enter password",
                      ": click it again or press Return. Esc cancels.", ": press Enter or click it again",
                      "Enter (or the same key again) to confirm, Esc to cancel", "tessera-greeter.json"):
            self.assertIn(words, rust, words)
            self.assertIn(words, qml, words)
        # (no product names on the login screen)
        self.assertNotIn("Touch ID", qml + rust)

    def test_the_greeter_is_a_mode_not_a_shell_surface(self):
        known = (CRATE / "src/surfaces.rs").read_text()
        self.assertNotIn('"greeter"', known.split("pub const KNOWN")[1].split("\n")[0])
        main = (CRATE / "src/main.rs").read_text()
        # (handed over before the arguments are checked: --greeter isn't a surface's)
        self.assertLess(main.index("tessera_greeter::wanted(&args)"), main.index("surfaces::unknown_arg(&args)"))


def binary():
    for kind in ("release", "debug"):
        b = CRATE / "target" / kind / "tessera-ui"
        if b.exists():
            return b
    return None


@unittest.skipUnless(binary(), "tessera-ui isn't built (tools/build-native.sh)")
class Binary(unittest.TestCase):
    def test_without_a_display_it_ends(self):
        with tempfile.TemporaryDirectory(prefix="tessera-greeter-rt-") as rt:
            env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY", "GREETD_SOCK", "HYPRLAND_INSTANCE_SIGNATURE")}
            env["XDG_RUNTIME_DIR"] = rt
            r = subprocess.run([str(binary()), "--greeter", "--demo"], capture_output=True, text=True, timeout=30, env=env, stdin=subprocess.DEVNULL)
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn("no Wayland display", r.stderr)


if __name__ == "__main__":
    unittest.main()
