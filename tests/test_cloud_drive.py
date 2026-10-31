"""cloud-drive.py: Google Drive and OneDrive folders with rclone, against
stand-ins for rclone, systemd-run and mountpoint (nothing real is signed in
to or mounted; HOME is a temporary folder)."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/cloud-drive.py"

FAKE_RCLONE = r'''#!/usr/bin/env python3
import json, os, sys
st = os.path.join(os.environ["FAKE_DIR"], "remotes.json")
remotes = json.load(open(st)) if os.path.exists(st) else []
open(os.path.join(os.environ["FAKE_DIR"], "calls"), "a").write(" ".join(sys.argv[1:]) + "\n")
a = sys.argv[1:]
if a[0] == "listremotes":
    print("\n".join(r + ":" for r in remotes))
elif a[:2] == ["config", "create"]:
    if os.environ.get("FAKE_SIGNIN") == "fail":
        sys.exit(1)
    remotes.append(a[2]); json.dump(remotes, open(st, "w"))
elif a[:2] == ["config", "delete"]:
    remotes = [r for r in remotes if r != a[2]]; json.dump(remotes, open(st, "w"))
'''
FAKE_SYSTEMD_RUN = '#!/bin/sh\necho "$@" >> "$FAKE_DIR/units"\n'
FAKE_MOUNTPOINT = '#!/bin/sh\n[ -e "$FAKE_DIR/mounted" ]\n'


class Drive(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        (t / "bin").mkdir()
        (t / "home").mkdir()
        for name, text in (("rclone", FAKE_RCLONE), ("systemd-run", FAKE_SYSTEMD_RUN), ("mountpoint", FAKE_MOUNTPOINT),
                           ("systemctl", "#!/bin/sh\n"), ("fusermount3", "#!/bin/sh\nrm -f \"$FAKE_DIR/mounted\"\n")):
            (t / "bin" / name).write_text(text)
            (t / "bin" / name).chmod(0o755)
        self.env = dict(os.environ, HOME=str(t / "home"), FAKE_DIR=str(t), PATH=str(t / "bin") + ":" + os.environ["PATH"],
                        IRICE_RCLONE=str(t / "bin" / "rclone"))
        self.t = t

    def tearDown(self):
        self.tmp.cleanup()

    def run_(self, *args, **env):
        r = subprocess.run([sys.executable, "-I", str(SCRIPT)] + list(args), capture_output=True, text=True,
                           env=dict(self.env, **env), timeout=30)
        return json.loads(r.stdout)

    def test_connect_mounts_a_unit(self):
        r = self.run_("connect", "gdrive")
        self.assertTrue(r["ok"], r)
        calls = (self.t / "calls").read_text()
        self.assertIn("config create irice-gdrive drive scope drive config_is_local true", calls)
        units = (self.t / "units").read_text()
        self.assertIn("--unit=irice-drive-gdrive", units)
        self.assertIn("mount irice-gdrive: " + str(self.t / "home" / "Google Drive"), units)
        self.assertIn("--vfs-cache-mode full", units)
        st = self.run_("status")
        self.assertTrue(st["drives"]["gdrive"]["connected"])
        self.assertFalse(st["drives"]["onedrive"]["connected"])

    def test_failed_signin_leaves_nothing(self):
        r = self.run_("connect", "onedrive", FAKE_SIGNIN="fail")
        self.assertFalse(r["ok"])
        self.assertIn("Sign-in", r["error"])
        self.assertFalse(self.run_("status")["drives"]["onedrive"]["connected"])

    def test_folder_with_files_is_left_alone(self):
        d = self.t / "home" / "OneDrive"
        d.mkdir()
        (d / "mine.txt").write_text("keep")
        r = self.run_("connect", "onedrive")
        self.assertFalse(r["ok"])
        self.assertIn("already has files", r["error"])
        self.assertTrue((d / "mine.txt").exists())

    def test_disconnect(self):
        self.run_("connect", "gdrive")
        (self.t / "mounted").write_text("")
        r = self.run_("disconnect", "gdrive")
        self.assertTrue(r["ok"], r)
        self.assertFalse(self.run_("status")["drives"]["gdrive"]["connected"])
        self.assertFalse((self.t / "home" / "Google Drive").exists())  # (the empty mount point)


if __name__ == "__main__":
    unittest.main()
