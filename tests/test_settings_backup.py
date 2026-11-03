"""Back up and restore settings (settings-backup.py): what leaves the computer
(no keys, accounts or displays), what restore keeps (this computer's own),
paths moved to the new home, bundled wallpapers found in this install, the
Undo copy, and files that aren't iRice's refused."""
import importlib.util
import io
import json
import os
import tarfile
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "shell/assets/scripts"


def load():
    spec = importlib.util.spec_from_file_location("settings_backup", SCRIPTS / "settings-backup.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Backup(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.sb = load()
        self.old_home = os.environ.get("HOME")
        self.home = self.root / "home"
        self.home.mkdir()
        os.environ["HOME"] = str(self.home)
        # An install with its bundled pictures
        self.shell = self.root / "install/shell"
        (self.shell / "assets/wallpapers").mkdir(parents=True)
        (self.shell / "assets/wallpapers/tahoe-dusk.jpg").write_bytes(b"jpg")
        (self.root / "install/material/assets/wallpapers").mkdir(parents=True)
        (self.root / "install/material/assets/wallpapers/material-dark.jpg").write_bytes(b"jpg")
        self.config = self.home / ".config/irice"
        self.config.mkdir(parents=True)
        self.out = self.root / "out"
        self.out.mkdir()

    def tearDown(self):
        if self.old_home is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = self.old_home
        self.tmp.cleanup()

    def write(self, name, data):
        (self.config / name).write_text(json.dumps(data))

    def read(self, name):
        return json.loads((self.config / name).read_text())

    def members(self, path):
        with tarfile.open(path, "r:gz") as t:
            return {i.name: t.extractfile(i).read() for i in t}

    def test_no_keys_accounts_or_displays_leave(self):
        self.write("config.json", {
            "appearance": {"look": "material"},
            "emoji": {"klipyKey": "SECRET-1"},
            "lock": {"passwordAfterSleep": 60},
            "sharing": {"name": "Desk", "enabled": True},
            "google": {"taskListId": "a|b"}, "cloud": {}, "displays": {"x": 1},
            "ai": {"models": [json.dumps({"id": "m", "apiKey": "SECRET-2", "name": "Mine"}), "not json"], "extraModels": [{"key": "SECRET-3"}]},
        })
        f = self.sb.export(str(self.out), str(self.config))
        self.assertTrue(os.path.basename(f).startswith("iRice settings "))
        self.assertEqual(os.stat(f).st_mode & 0o777, 0o600)
        m = self.members(f)
        self.assertNotIn(b"SECRET", b"".join(m.values()))
        c = json.loads(m["config.json"])
        for sec in ("google", "cloud", "displays"):
            self.assertNotIn(sec, c)
        self.assertNotIn("name", c["sharing"])
        self.assertEqual(c["lock"]["passwordAfterSleep"], 60)  # (a number, not a password)
        self.assertEqual([json.loads(x)["name"] for x in c["ai"]["models"]], ["Mine"])
        # A second one the same day gets its own name
        self.assertNotEqual(self.sb.export(str(self.out), str(self.config)), f)

    def make_backup(self, config, home="/home/olduser", focus=None, keybinds=None, pictures=None, extra=None):
        manifest = {"app": "iRice", "format": 1, "created": "2026-11-03T10:00:00", "home": home, "wallpapers": pictures or {}}
        files = {"manifest.json": json.dumps(manifest).encode(), "config.json": json.dumps(config).encode()}
        if focus is not None:
            files["focus.json"] = json.dumps(focus).encode()
        if keybinds is not None:
            files["keybinds.json"] = json.dumps(keybinds).encode()
        files.update(extra or {})
        p = self.root / "b.irice"
        with tarfile.open(p, "w:gz") as t:
            for n, d in files.items():
                i = tarfile.TarInfo(n)
                i.size = len(d)
                t.addfile(i, io.BytesIO(d))
        return str(p)

    def test_restore_keeps_this_computers_own_and_moves_home(self):
        self.write("config.json", {"emoji": {"klipyKey": "MINE"}, "google": {"taskListId": "here"}, "sharing": {"name": "Laptop"}, "appearance": {"look": "glass"}})
        notes = self.home / "Notes"
        notes.mkdir()
        b = self.make_backup({
            "appearance": {"look": "material", "mode": "light"},
            "notes": {"folder": "/home/olduser/Notes"},
            "google": {"taskListId": "theirs"},
            "wallpaper": {"path": "/home/olduser/Pictures/beach.jpg", "dynamic": "time", "dynamicFolder": "/home/olduser/Missing", "live": "/home/olduser/v.mp4"},
        }, keybinds={"version": 1, "shortcuts": [{"id": "custom-1", "action": "/home/olduser/bin/x"}]},
            focus={"modes": [{"id": "work", "name": "Work", "wallpaper": ""}], "active": "dnd"})
        r = self.sb.restore(b, str(self.config), str(self.shell), str(self.root / "pics"))
        c = self.read("config.json")
        self.assertEqual(c["emoji"]["klipyKey"], "MINE")
        self.assertEqual(c["google"]["taskListId"], "here")
        self.assertEqual(c["sharing"]["name"], "Laptop")
        self.assertEqual(c["appearance"]["look"], "material")
        self.assertEqual(r["look"], "material")
        self.assertEqual(c["notes"]["folder"], str(notes))
        # Pictures and folders that aren't here: off, not broken
        self.assertEqual(c["wallpaper"]["path"], "")
        self.assertEqual((c["wallpaper"]["dynamic"], c["wallpaper"]["dynamicFolder"], c["wallpaper"]["live"]), ("off", "", ""))
        self.assertEqual(self.read("keybinds.json")["shortcuts"][0]["action"], str(self.home) + "/bin/x")
        f = self.read("focus.json")
        self.assertEqual([m["id"] for m in f["modes"]], ["work"])
        self.assertEqual(f["active"], "")  # (dnd isn't among them)

    def test_bundled_and_carried_pictures(self):
        self.write("config.json", {})
        b = self.make_backup({"wallpaper": {"path": "/x/shell/assets/wallpapers/tahoe-dusk.jpg", "darkPath": "/old/m.jpg", "lightPath": "/x/material-shell/assets/wallpapers/material-dark.jpg"}},
                             pictures={"wallpaper.path": {"bundled": "tahoe-dusk.jpg"}, "wallpaper.lightPath": {"bundled": "material-dark.jpg"}, "wallpaper.darkPath": {"file": "wallpapers/0-m.jpg"}},
                             extra={"wallpapers/0-m.jpg": b"picture"})
        pics = self.root / "pics"
        (pics).mkdir()
        (pics / "m.jpg").write_bytes(b"other")  # (a name already taken)
        self.sb.restore(b, str(self.config), str(self.shell), str(pics))
        wp = self.read("config.json")["wallpaper"]
        self.assertEqual(wp["path"], str(self.shell / "assets/wallpapers/tahoe-dusk.jpg"))
        self.assertEqual(wp["lightPath"], str(self.root / "install/material/assets/wallpapers/material-dark.jpg"))
        self.assertEqual(wp["darkPath"], str(pics / "m 2.jpg"))
        self.assertEqual((pics / "m 2.jpg").read_bytes(), b"picture")

    def test_undo_copy_comes_first(self):
        self.write("config.json", {"appearance": {"look": "glass", "mode": "dark"}})
        b = self.make_backup({"appearance": {"look": "material"}})
        r = self.sb.restore(b, str(self.config), str(self.shell))
        self.assertTrue(r["undo"].startswith(str(self.config / "backups" / "before-restore-")))
        self.assertEqual(json.loads(self.members(r["undo"])["config.json"])["appearance"], {"look": "glass", "mode": "dark"})
        # Undo: restoring it, without another copy
        r2 = self.sb.restore(r["undo"], str(self.config), str(self.shell), undo=False)
        self.assertEqual(r2["undo"], "")
        self.assertEqual(self.read("config.json")["appearance"]["look"], "glass")
        self.assertEqual(len(os.listdir(self.config / "backups")), 1)

    def test_files_that_arent_irices_are_refused(self):
        self.write("config.json", {"keep": 1})
        bad = [
            {"../evil": b"x"},
            {"/etc/passwd": b"x"},
            {"wallpapers/../../x.jpg": b"x"},
            {"something.sh": b"x"},
        ]
        for extra in bad:
            b = self.make_backup({}, extra=extra)
            with self.assertRaises(self.sb.Refused):
                self.sb.restore(b, str(self.config), str(self.shell))
        # A link inside
        p = self.root / "link.irice"
        with tarfile.open(p, "w:gz") as t:
            for n, d in (("manifest.json", json.dumps({"app": "iRice", "format": 1}).encode()), ("config.json", b"{}")):
                i = tarfile.TarInfo(n)
                i.size = len(d)
                t.addfile(i, io.BytesIO(d))
            i = tarfile.TarInfo("wallpapers/a.jpg")
            i.type = tarfile.SYMTYPE
            i.linkname = "/etc/shadow"
            t.addfile(i)
        with self.assertRaises(self.sb.Refused):
            self.sb.restore(str(p), str(self.config), str(self.shell))
        # Not an archive; a newer format
        (self.root / "x.irice").write_text("hello")
        with self.assertRaises(self.sb.Refused):
            self.sb.unpack(str(self.root / "x.irice"))
        b = self.make_backup({})
        with tarfile.open(b, "r:gz"):
            pass
        newer = self.root / "n.irice"
        with tarfile.open(newer, "w:gz") as t:
            d = json.dumps({"app": "iRice", "format": 99}).encode()
            i = tarfile.TarInfo("manifest.json")
            i.size = len(d)
            t.addfile(i, io.BytesIO(d))
            i = tarfile.TarInfo("config.json")
            i.size = 2
            t.addfile(i, io.BytesIO(b"{}"))
        with self.assertRaisesRegex(self.sb.Refused, "newer"):
            self.sb.unpack(str(newer))
        # Nothing was touched by any of them
        self.assertEqual(self.read("config.json"), {"keep": 1})
        self.assertFalse((self.config / "backups").exists())

    def test_inspect_says_whats_inside(self):
        self.write("config.json", {"appearance": {"look": "material", "mode": "auto"}, "wallpaper": {"path": str(self.shell / "assets/wallpapers/tahoe-dusk.jpg")}})
        self.write("keybinds.json", {"version": 1, "keybinds": {"a": {}}, "shortcuts": [{}], "gestures": {"changes": {}, "custom": [{}]}})
        self.write("focus.json", {"modes": [{"id": "dnd", "name": "Do Not Disturb"}, {"id": "work", "name": "Work"}], "active": "work"})
        f = self.sb.export(str(self.out), str(self.config))
        s = self.sb.summary(self.sb.unpack(f))
        self.assertEqual((s["look"], s["mode"], s["wallpaper"], s["shortcuts"], s["focusModes"]), ("material", "auto", "tahoe-dusk.jpg", 3, ["Do Not Disturb", "Work"]))
        self.assertNotIn("active", json.loads(self.members(f)["focus.json"]))

    def test_cli_prints_json(self):
        import contextlib
        self.write("config.json", {})
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = self.sb.main(["inspect", str(self.root / "missing.irice")])
        self.assertEqual(code, 1)
        out = json.loads(buf.getvalue())
        self.assertFalse(out["ok"])
        self.assertIn("Back Up Settings", out["error"])


if __name__ == "__main__":
    unittest.main()
