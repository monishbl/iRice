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

    # ── Settings → Login Items: list, set, add, remove, restore, start ──

    def manage(self, *args, ok=True, **env):
        p = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], env=dict(self.env, **env),
                           capture_output=True, text=True, timeout=20)
        out = json.loads(p.stdout)
        self.assertEqual(out.get("ok", True), ok, (args, out, p.stderr))
        self.assertEqual(p.returncode, 0 if ok else 2, (args, out))
        return out

    def items(self):
        return {i["file"]: i for i in self.manage("list")["items"]}

    def test_list_reports_every_entry(self):
        self.entry(self.system, "syncthing", Exec="syncthing serve", Icon="syncthing", Comment="Sync files")
        self.entry(self.system, "mako", Exec="mako")
        self.entry(self.system, "kde-only", Exec="app", OnlyShowIn="KDE;")
        self.entry(self.system, "both", Exec="app", OnlyShowIn="GNOME;XFCE;")
        self.entry(self.system, "gone", Exec="not-installed-anywhere")
        self.entry(self.system, "dropbox", Exec="dropbox")
        self.entry(self.user, "dropbox", Exec="dropbox", Hidden="true")
        self.entry(self.user, "keepassxc", Exec="keepassxc")
        self.entry(self.user, "off-kde", Exec="app", Hidden="true", OnlyShowIn="KDE;")
        out = self.manage("list")
        self.assertTrue(out["startAtLogin"])
        it = {i["file"]: i for i in out["items"]}
        s = it["syncthing.desktop"]
        self.assertEqual((s["name"], s["icon"], s["comment"], s["exec"], s["origin"], s["status"], s["enabled"], s["removable"]),
                         ("syncthing", "syncthing", "Sync files", "syncthing serve", "system", "start", True, False))
        self.assertEqual((it["mako.desktop"]["status"], it["mako.desktop"]["reason"]), ("skipped", "iRice does this itself"))
        self.assertFalse(it["mako.desktop"]["canStart"])
        self.assertEqual(it["kde-only.desktop"]["reason"], "Only for KDE Plasma")
        self.assertEqual(it["both.desktop"]["reason"], "Only for GNOME and Xfce")
        self.assertEqual(it["gone.desktop"]["reason"], "Not installed")
        d = it["dropbox.desktop"]
        self.assertEqual((d["origin"], d["overridden"], d["status"], d["enabled"], d["removable"]), ("system", True, "off", False, False))
        k = it["keepassxc.desktop"]
        self.assertEqual((k["origin"], k["status"], k["removable"], k["canStart"]), ("user", "start", True, True))
        o = it["off-kde.desktop"]
        self.assertEqual((o["status"], o["reason"], o["skippedWhenOn"]), ("off", "Turned off", "Only for KDE Plasma"))

    def test_turning_a_system_entry_off_and_on(self):
        self.entry(self.system, "syncthing", Exec="syncthing serve", Icon="syncthing")
        self.manage("set", "syncthing.desktop", "off")
        override = self.user / "syncthing.desktop"
        text = override.read_text()
        self.assertIn("Hidden=true", text)
        self.assertIn("Exec=syncthing serve", text)  # (a copy: other desktops read it too)
        self.assertEqual(self.items()["syncthing.desktop"]["status"], "off")
        self.assertEqual(json.loads(self.run_it("--dry-run").stdout)["start"], [])
        self.manage("set", "syncthing.desktop", "on")
        self.assertFalse(override.exists())  # (the override is gone: the system's own entry again)
        self.assertEqual(self.items()["syncthing.desktop"]["status"], "start")
        self.assertNotIn("Hidden", (self.system / "syncthing.desktop").read_text())

    def test_turning_on_a_system_entry_that_ships_off(self):
        self.entry(self.system, "app", Exec="app", **{"X-GNOME-Autostart-enabled": "false"})
        self.assertEqual(self.items()["app.desktop"]["status"], "off")
        self.manage("set", "app.desktop", "on")
        self.assertIn("X-GNOME-Autostart-enabled=true", (self.user / "app.desktop").read_text())
        self.assertEqual(self.items()["app.desktop"]["status"], "start")
        self.manage("set", "app.desktop", "off")
        self.assertEqual(self.items()["app.desktop"]["status"], "off")

    def test_turning_your_own_entry_off_keeps_the_file(self):
        (self.user / "app.desktop").write_text("# mine\n[Desktop Entry]\nType=Application\nName=App\nExec=app\n\n[Desktop Action new]\nName=New\nExec=app --new\n")
        self.manage("set", "app.desktop", "off")
        text = (self.user / "app.desktop").read_text()
        self.assertTrue(text.startswith("# mine\n[Desktop Entry]\n"), text)
        self.assertLess(text.index("Hidden=true"), text.index("[Desktop Action new]"))
        self.assertEqual(text.count("Hidden"), 1)
        self.manage("set", "app.desktop", "off")  # (twice: still one line)
        self.assertEqual((self.user / "app.desktop").read_text().count("Hidden"), 1)
        self.manage("set", "app.desktop", "on")
        text = (self.user / "app.desktop").read_text()
        self.assertNotIn("Hidden", text)
        self.assertIn("Exec=app --new", text)
        self.assertEqual(self.items()["app.desktop"]["status"], "start")

    def test_add_copies_the_app_and_remove_restore(self):
        apps = Path(self.tmp.name) / "share/applications"
        (apps / "kde").mkdir(parents=True)
        (apps / "org.app.Notes.desktop").write_text(
            "[Desktop Entry]\nType=Application\nName=Notes\nName[de]=Notizen\nExec=app --notes %U\nIcon=notes\nTerminal=false\nMimeType=text/plain;\nActions=new;\n\n[Desktop Action new]\nExec=app --new\n")
        (apps / "kde/sub.desktop").write_text("[Desktop Entry]\nType=Application\nName=Sub\nExec=app\n")
        env = dict(XDG_DATA_HOME=str(Path(self.tmp.name) / "share"), XDG_DATA_DIRS=str(Path(self.tmp.name) / "none"))
        self.manage("add", "org.app.Notes", **env)
        text = (self.user / "org.app.Notes.desktop").read_text()
        self.assertIn("Name=Notes\n", text)
        self.assertIn("Exec=app --notes %U\n", text)
        self.assertIn("Icon=notes\n", text)
        self.assertNotIn("MimeType", text)
        self.assertNotIn("Desktop Action", text)
        item = self.items()["org.app.Notes.desktop"]
        self.assertEqual((item["origin"], item["status"], item["removable"], item["added"]), ("user", "start", True, True))
        self.assertEqual(json.loads(self.run_it("--dry-run").stdout)["start"][0]["command"], ["app", "--notes"])
        self.manage("add", "kde-sub", **env)
        self.assertTrue((self.user / "kde-sub.desktop").exists())
        # adding one that's there but off turns it on
        self.manage("set", "kde-sub.desktop", "off")
        self.assertTrue(self.manage("add", "kde-sub.desktop", **env)["existed"])
        self.assertEqual(self.items()["kde-sub.desktop"]["status"], "start")
        self.assertIn("isn't installed", self.manage("add", "org.not.There", ok=False, **env)["error"])
        # remove, then Undo
        self.manage("remove", "org.app.Notes.desktop")
        self.assertNotIn("org.app.Notes.desktop", self.items())
        self.manage("restore", "org.app.Notes.desktop")
        self.assertEqual((self.user / "org.app.Notes.desktop").read_text(), text)
        self.manage("restore", "org.app.Notes.desktop", ok=False)

    def test_system_entries_cant_be_removed(self):
        self.entry(self.system, "syncthing", Exec="syncthing")
        self.manage("set", "syncthing.desktop", "off")
        self.assertIn("turn it off", self.manage("remove", "syncthing.desktop", ok=False)["error"])
        self.assertTrue((self.system / "syncthing.desktop").exists())

    def test_inputs_are_file_names_only(self):
        outside = Path(self.tmp.name) / "home/.config/evil.desktop"
        self.entry(self.system, "app", Exec="app")
        for bad in ("../evil.desktop", "/etc/xdg/autostart/app.desktop", "app", ".app.desktop", "a/b.desktop",
                    "..desktop", "app.desktop\n", "x" * 300 + ".desktop", ""):
            self.manage("set", bad, "off", ok=False)
            self.manage("remove", bad, ok=False)
            self.manage("start", bad, ok=False)
        for bad in ("../x", "a/b", "", "-rf", "app\n"):
            self.manage("add", bad, ok=False)
        self.manage("set", "app.desktop", "maybe", ok=False)
        self.manage("set", "missing.desktop", "off", ok=False)
        self.manage("list", "extra", ok=False)
        self.assertFalse(outside.exists())
        self.assertEqual(sorted(os.listdir(self.user)), [])

    def test_start_now(self):
        self.entry(self.user, "app", Exec="app --now", Hidden="true")  # (even one that's off)
        self.manage("start", "app.desktop")
        self.assertEqual(self.started(), [f"{self.env['HOME']}|app --now"])
        self.entry(self.user, "gone", Exec="not-installed-anywhere")
        self.assertIn("isn't installed", self.manage("start", "gone.desktop", ok=False)["error"])

    def test_the_master_switch(self):
        self.entry(self.user, "app", Exec="app")
        cfg = Path(self.env["XDG_CONFIG_HOME"]) / "irice"
        cfg.mkdir()
        (cfg / "config.json").write_text(json.dumps({"loginItems": {"startAtLogin": False}}))
        self.assertFalse(self.manage("list")["startAtLogin"])
        self.assertFalse(json.loads(self.run_it("--dry-run").stdout)["startAtLogin"])
        self.run_it()
        self.assertEqual(self.started(), [])
        (cfg / "config.json").write_text(json.dumps({"loginItems": {"startAtLogin": True}}))
        self.run_it()
        self.assertEqual(len(self.started()), 1)
        (cfg / "config.json").write_text("{ not json")  # (unreadable: on, as by default)
        self.assertTrue(self.manage("list")["startAtLogin"])

    def test_system_root_moves_the_default_system_folder(self):
        root = Path(self.tmp.name) / "root"
        (root / "etc/xdg/autostart").mkdir(parents=True)
        self.entry(root / "etc/xdg/autostart", "app", Exec="app")
        env = dict(self.env)
        env.pop("XDG_CONFIG_DIRS")
        p = subprocess.run([sys.executable, "-I", str(SCRIPT), "list"], env=dict(env, IRICE_SYSTEM_ROOT=str(root)),
                           capture_output=True, text=True, timeout=20)
        self.assertEqual([i["file"] for i in json.loads(p.stdout)["items"]], ["app.desktop"])


if __name__ == "__main__":
    unittest.main()
