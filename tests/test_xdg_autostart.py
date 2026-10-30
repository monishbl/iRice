"""XDG autostart (shell/assets/scripts/xdg-autostart.py) with a temporary
home and system folder and a fake setsid on PATH that only notes what it
was asked to start. Nothing real starts."""
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "shell/assets/scripts/xdg-autostart.py"


class Autostart(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        self.user = d / "home/.config/autostart"
        self.user.mkdir(parents=True)
        self.system = d / "etc/xdg/autostart"
        self.system.mkdir(parents=True)
        self.log = d / "started"
        self.rt = d / "run"
        self.rt.mkdir()
        # setsid -f <command…>: one line per start, with the folder it ran in
        self.fake("setsid", f'shift; echo "$PWD|$*" >>{self.log}\n')
        for app in ("syncthing", "nm-applet", "mako", "dropbox", "app", "keepassxc"):
            self.fake(app, "")
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(d / "home"),
                        XDG_CONFIG_HOME=str(d / "home/.config"), XDG_CONFIG_DIRS=str(d / "etc/xdg"),
                        XDG_RUNTIME_DIR=str(self.rt), HYPRLAND_INSTANCE_SIGNATURE="sig1", XDG_CURRENT_DESKTOP="Hyprland")

    def tearDown(self):
        self.tmp.cleanup()

    def fake(self, name, script):
        f = self.bin / name
        f.write_text("#!/bin/sh\n" + script)
        f.chmod(0o755)

    def entry(self, folder, name, **keys):
        body = "[Desktop Entry]\nType=Application\nName=" + name + "\n" + "".join(f"{k}={v}\n" for k, v in keys.items())
        (folder / f"{name}.desktop").write_text(body)

    def run_it(self, *args, **env):
        p = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], env=dict(self.env, **env),
                           capture_output=True, text=True, timeout=20)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p

    def started(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def test_what_starts_and_what_doesnt(self):
        self.entry(self.system, "syncthing", Exec="syncthing serve --no-browser %U")
        self.entry(self.system, "nm-applet", Exec="nm-applet", NotShowIn="KDE;GNOME;")
        self.entry(self.system, "gnome-only", Exec="app", OnlyShowIn="GNOME;")
        self.entry(self.system, "for-hyprland", Exec="app --hypr", OnlyShowIn="Hyprland;")
        self.entry(self.system, "hidden", Exec="app", Hidden="true")
        self.entry(self.system, "gnome-off", Exec="app", **{"X-GNOME-Autostart-enabled": "false"})
        self.entry(self.system, "missing", Exec="app", TryExec="not-installed-anywhere")
        self.entry(self.system, "not-on-path", Exec="not-installed-anywhere --x")
        self.entry(self.system, "mako", Exec="mako")  # (iRice shows notifications)
        self.entry(self.system, "polkit-gnome-authentication-agent-1", Exec="/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1")
        self.entry(self.system, "gnome-keyring-secrets", Exec="/usr/bin/gnome-keyring-daemon --start --components=secrets")
        # the user's copy of a system entry wins: here it turns it off
        self.entry(self.system, "dropbox", Exec="dropbox")
        self.entry(self.user, "dropbox", Exec="dropbox", Hidden="true")
        self.entry(self.user, "keepassxc", Exec='keepassxc "--minimized" %f', Path=str(self.rt))
        plan = json.loads(self.run_it("--dry-run").stdout)
        start = {e["file"]: e["command"] for e in plan["start"]}
        self.assertEqual(start, {"syncthing.desktop": ["syncthing", "serve", "--no-browser"],
                                 "nm-applet.desktop": ["nm-applet"], "for-hyprland.desktop": ["app", "--hypr"],
                                 "keepassxc.desktop": ["keepassxc", "--minimized"]})
        skipped = plan["skipped"]
        self.assertEqual(skipped["hidden.desktop"], "turned off (Hidden)")
        self.assertEqual(skipped["dropbox.desktop"], "turned off (Hidden)")
        self.assertIn("X-GNOME", skipped["gnome-off.desktop"])
        self.assertIn("only for GNOME", skipped["gnome-only.desktop"])
        self.assertEqual(skipped["missing.desktop"], "not installed")
        self.assertEqual(skipped["not-on-path.desktop"], "not installed")
        for n in ("mako", "polkit-gnome-authentication-agent-1", "gnome-keyring-secrets"):
            self.assertEqual(skipped[n + ".desktop"], "iRice does this itself", n)
        self.assertEqual(self.started(), [])  # (a dry run starts nothing)
        # the real run: each one detached, in its Path= (else home)
        self.run_it()
        started = self.started()
        self.assertEqual(len(started), 4, started)
        self.assertIn(f"{self.rt}|keepassxc --minimized", started)
        self.assertIn(f"{self.env['HOME']}|syncthing serve --no-browser", started)

    def test_once_per_session(self):
        self.entry(self.user, "app", Exec="app")
        self.run_it()
        self.run_it()
        self.assertEqual(len(self.started()), 1)
        self.run_it(HYPRLAND_INSTANCE_SIGNATURE="sig2")  # (a new session)
        self.assertEqual(len(self.started()), 2)

    def test_exec_quoting_and_field_codes(self):
        # (\s is a space in the value; quoted, it stays one argument)
        self.entry(self.user, "app", Exec=r'app "two words" "a\sb" %%done %i %c %k', Icon="app-icon")
        plan = json.loads(self.run_it("--dry-run").stdout)
        self.assertEqual(plan["start"][0]["command"],
                         ["app", "two words", "a b", "%done", "--icon", "app-icon", "app", str(self.user / "app.desktop")])

    def test_terminal_apps_go_through_open_app(self):
        self.entry(self.user, "app", Exec="app --tui", Terminal="true")
        self.run_it()
        line = self.started()[0]
        self.assertIn("open-app.sh terminal -- app --tui", line)

    def test_the_tty_session_drops_its_sandbox_folders(self):
        self.fake("setsid", f'shift; echo "${{XDG_DATA_HOME:-none}}" >>{self.log}\n')
        self.entry(self.user, "app", Exec="app")
        self.run_it(IRICE_TTY="1", XDG_DATA_HOME="/sandbox/data")
        self.assertEqual(self.started(), ["none"])


if __name__ == "__main__":
    unittest.main()
