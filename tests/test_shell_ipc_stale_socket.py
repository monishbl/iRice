"""shell-ipc.sh with a socket a crashed tessera-ui left behind.

tessera-ui that dies without its quit() (a panic, a segfault, SIGKILL)
leaves $XDG_RUNTIME_DIR/tessera/ipc.sock. While the QML fallback runs,
shell-ipc.sh used to exec `tessera-ui ipc …`, which found nobody on the
socket and exited 1: hypridle's `shell-ipc.sh call lock lock` never locked.
Now an exit of 1 (nothing answers) goes on to the QML shell; any other exit
is tessera-ui's answer. Everything here is fake (tessera-ui, qs) and lives in
a throwaway home and runtime folder.
"""
import os
import socket
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/shell-ipc.sh"


class StaleSocket(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        root = Path(self.dir.name)
        self.root = root
        self.bin = root / "bin"
        self.bin.mkdir()
        run = root / "run"
        (run / "tessera").mkdir(parents=True)
        # (a socket file nothing listens on: bound, then closed)
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.bind(str(run / "tessera/ipc.sock"))
        s.close()
        # qs: a running QML shell that records the call it got
        self.stub("qs", '#!/bin/sh\nfor a in "$@"; do [ "$a" = show ] && { echo "target lock"; exit 0; }; done\necho "$@" > "$QS_CALL"\n')
        self.env = dict(os.environ, HOME=str(root), XDG_RUNTIME_DIR=str(run), XDG_CONFIG_HOME=str(root / "config"),
                        XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"),
                        QS=str(self.bin / "qs"), QS_CALL=str(root / "qs-call"), TESSERA_UI=str(self.bin / "tessera-ui"),
                        PATH=f"{self.bin}:/usr/bin:/bin")
        self.env.pop("TESSERA_STANDALONE", None)

    def tearDown(self):
        self.dir.cleanup()

    def stub(self, name, text):
        p = self.bin / name
        p.write_text(text)
        p.chmod(0o755)

    def call(self):
        return subprocess.run(["bash", str(SCRIPT), "call", "lock", "lock"], env=self.env, capture_output=True, text=True, timeout=20)

    def test_nobody_on_the_socket_goes_on_to_the_qml_shell(self):
        self.stub("tessera-ui", '#!/bin/sh\necho "tessera-ui ipc: no Tessera shell is running" >&2\nexit 1\n')
        r = self.call()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("call lock lock", (self.root / "qs-call").read_text())

    def test_tessera_ui_answering_is_the_answer(self):
        self.stub("tessera-ui", '#!/bin/sh\necho "locked"\nexit 0\n')
        r = self.call()
        self.assertEqual((r.returncode, r.stdout.strip()), (0, "locked"))
        self.assertFalse((self.root / "qs-call").exists(), "(the QML shell is not asked too)")

    def test_a_usage_error_is_not_retried_on_the_qml_shell(self):
        self.stub("tessera-ui", '#!/bin/sh\nexit 2\n')
        r = self.call()
        self.assertEqual(r.returncode, 2)
        self.assertFalse((self.root / "qs-call").exists())


if __name__ == "__main__":
    unittest.main()
