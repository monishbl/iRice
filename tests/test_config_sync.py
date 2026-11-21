"""tools/config-sync.py (20 Nov audit #16): after an update, the installed
copies follow the checkout. The shell's copy is replaced (a dev install's
link stays); ~/.config/hypr's Tessera files are merged three ways, against
the base of what was shipped last: your edits stay, Tessera's changes come
in, a clash keeps yours with the new file beside it as .new and a
notification. Each test has its own throwaway repository, config and data
folders; nothing real is read or written."""
import importlib.machinery
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
ID = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]


def load():
    name = "config_sync_" + os.urandom(3).hex()
    loader = importlib.machinery.SourceFileLoader(name, str(ROOT / "tools/config-sync.py"))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


HYPRLAND = """-- Tessera Hyprland config
local dir = cfg .. "/hypr"
dofile(dir .. "/tessera/look.lua")
pcall(dofile, dir .. "/tessera/tabs.lua") -- window tabs
dofile(dir .. "/tessera/keybinds.lua")
pcall(dofile, dir .. "/tessera/keyboard.lua") -- written by the installer
"""
KEYBINDS = """local here = TESSERA_HYPR_DIR
local json = dofile(here .. "/json.lua")
local defaults = json.read(here .. "/keymap.json")
local gestures = json.read(here .. "/gestures.json") or {}
bind("SUPER", "Q", "close")
bind("SUPER", "T", "terminal")
bind("SUPER", "E", "files")
bind("SUPER", "B", "browser")
bind("SUPER", "L", "lock")
"""
LOOK = "\n".join(f"hl.config({{ general = {{ gaps_{i} = {i} }} }})" for i in range(1, 9)) + "\n"
KEYMAP = [{"id": "close", "keys": "$MOD + Q", "label": "Close"},
          {"id": "terminal", "keys": "$MOD + T", "label": "Terminal"},
          {"id": "files", "keys": "$MOD + E", "label": "Files"}]
GESTURES = [{"id": "move", "fingers": 3, "action": "move"}, {"id": "overview", "fingers": 4, "action": "overview"}]


def jtext(v):
    return json.dumps(v, indent=1, ensure_ascii=False) + "\n"


class Sync(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.repo, self.config, self.data, self.state = t / "repo", t / "config", t / "data", t / "state"
        for d in (self.config, self.data, self.state):
            d.mkdir()
        env = {"XDG_CONFIG_HOME": str(self.config), "XDG_DATA_HOME": str(self.data), "XDG_STATE_HOME": str(self.state),
               "TESSERA_NO_NOTIFY": "1"}
        self.env = mock.patch.dict(os.environ, env)
        self.env.start()
        self.cs = load()
        self.notes = []
        self.cs.NOTIFY = lambda title, body: self.notes.append((title, body))
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.repo)], check=True)
        self.files = {"hyprland.lua": HYPRLAND, "tessera/keybinds.lua": KEYBINDS, "tessera/json.lua": "return {}\n",
                      "tessera/look.lua": LOOK, "tessera/tabs.lua": "-- tabs\n", "tessera/old.lua": "-- old\n",
                      "tessera/keymap.json": jtext(KEYMAP), "tessera/gestures.json": jtext(GESTURES)}
        self.v1 = self.commit(self.files)
        (self.repo / "shell").mkdir()
        (self.repo / "shell/shell.qml").write_text("// v1\n")
        # Installed as install.sh does: copies, the base, keyboard.lua of its own
        self.hypr = self.config / "hypr"
        for rel, text in self.files.items():
            p = self.hypr / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        (self.hypr / "tessera/keyboard.lua").write_text("-- your layout: de\n")
        (self.hypr / "custom.lua").write_text("-- mine\n")
        shutil.copytree(self.repo / "shell", self.config / "quickshell/tessera")
        self.cs.write_base(self.cs.shipped(str(self.repo)), self.v1)

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def commit(self, files):
        for rel, text in files.items():
            p = self.repo / "hypr" / rel
            if text is None:
                if p.exists():
                    p.unlink()
                continue
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "v", "--allow-empty")
        return git(self.repo, "rev-parse", "HEAD")

    def update(self, files, old_rev=None):
        self.commit(files)
        return self.cs.sync(str(self.repo), old_rev or self.v1)

    def mine(self, rel):
        p = self.hypr / rel
        return p.read_text() if p.exists() else None

    # ── the cases ──────────────────────────────────────────────────────
    def test_a_clean_update_brings_the_new_files(self):
        rep = self.update({"tessera/look.lua": LOOK.replace("gaps_3 = 3", "gaps_3 = 30"), "tessera/new.lua": "-- new\n"})
        self.assertEqual(self.mine("tessera/look.lua"), LOOK.replace("gaps_3 = 3", "gaps_3 = 30"))
        self.assertEqual(self.mine("tessera/new.lua"), "-- new\n")
        self.assertEqual(rep["hypr"]["notes"], [])
        self.assertEqual(self.notes, [])
        self.assertFalse(list(self.hypr.rglob("*.new")))
        # the base is the new version now
        self.assertEqual((self.data / "tessera/hypr-base/tessera/new.lua").read_text(), "-- new\n")

    def test_your_edit_stays_and_tesseras_change_comes_in(self):
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('"Q", "close"', '"W", "close"'))
        self.update({"tessera/keybinds.lua": KEYBINDS.replace('"lock"', '"lock-now"')})
        got = self.mine("tessera/keybinds.lua")
        self.assertIn('"W", "close"', got)  # yours
        self.assertIn('"lock-now"', got)  # Tessera's
        self.assertNotIn('"Q", "close"', got)
        self.assertEqual(self.notes, [])
        self.assertFalse((self.hypr / "tessera/keybinds.lua.new").exists())

    def test_a_line_you_deleted_stays_deleted(self):
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('bind("SUPER", "E", "files")\n', ""))
        self.update({"tessera/keybinds.lua": KEYBINDS.replace('"lock"', '"lock-now"')})
        got = self.mine("tessera/keybinds.lua")
        self.assertNotIn("files", got)
        self.assertIn('"lock-now"', got)

    def test_a_clash_keeps_yours_with_new_beside_it_and_says_so(self):
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('"T", "terminal"', '"Return", "terminal"'))
        new = KEYBINDS.replace('"T", "terminal"', '"X", "terminal"').replace('"lock"', '"lock-now"')
        rep = self.update({"tessera/keybinds.lua": new})
        got = self.mine("tessera/keybinds.lua")
        self.assertIn('"Return", "terminal"', got)  # your lines win
        self.assertIn('"lock-now"', got)  # (Tessera's change elsewhere still comes in)
        self.assertNotIn("<<<<<<<", got)
        self.assertEqual(self.mine("tessera/keybinds.lua.new"), new)
        self.assertEqual(len(self.notes), 1)
        self.assertIn("keybinds.lua: 1 change kept as yours — see keybinds.lua.new", self.notes[0][1])
        self.assertEqual(rep["hypr"]["clashes"], {"tessera/keybinds.lua": 1})
        # the file before the merge is in the backup
        backup = Path(rep["hypr"]["backup"])
        self.assertIn('"Return", "terminal"', (backup / "hypr/tessera/keybinds.lua").read_text())
        self.assertNotIn('"lock-now"', (backup / "hypr/tessera/keybinds.lua").read_text())

    def test_running_it_again_changes_nothing(self):
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('"T", "terminal"', '"Return", "terminal"'))
        self.update({"tessera/keybinds.lua": KEYBINDS.replace('"T", "terminal"', '"X", "terminal"')})
        before = {str(p): p.read_bytes() for p in self.hypr.rglob("*") if p.is_file()}
        self.notes.clear()
        rep = self.cs.sync(str(self.repo), self.v1)
        after = {str(p): p.read_bytes() for p in self.hypr.rglob("*") if p.is_file()}
        self.assertEqual(before, after)
        self.assertEqual(self.notes, [])
        self.assertFalse(rep["hypr"]["any"])

    def test_json_clashes_merge_by_key(self):
        mine = [dict(KEYMAP[0], keys="$MOD + W"), KEYMAP[1], KEYMAP[2], {"id": "mine", "keys": "$MOD + M", "label": "Mine"}]
        (self.hypr / "tessera/keymap.json").write_text(jtext(mine))
        new = [dict(KEYMAP[0], keys="$MOD + C", label="Close the window"), KEYMAP[1], dict(KEYMAP[2], label="Files app"),
               {"id": "notes", "keys": "$MOD + N", "label": "Notes"}]
        self.update({"tessera/keymap.json": jtext(new)})
        got = {x["id"]: x for x in json.loads(self.mine("tessera/keymap.json"))}
        self.assertEqual(got["close"]["keys"], "$MOD + W")  # yours (a clash)
        self.assertEqual(got["close"]["label"], "Close the window")  # Tessera's, in the same object
        self.assertEqual(got["files"]["label"], "Files app")
        self.assertIn("notes", got)  # new from Tessera
        self.assertIn("mine", got)  # added by you
        self.assertTrue((self.hypr / "tessera/keymap.json.new").exists())
        self.assertIn("keymap.json: 1 change kept as yours", self.notes[0][1])

    def test_a_merge_that_breaks_the_file_keeps_yours(self):
        # (a merge that fits line by line but doesn't parse: yours, whole,
        # and the new file beside it; here only the new file itself passes)
        new = LOOK.replace("gaps_8 = 8", "gaps_8 = 80")
        real = self.cs.valid
        self.cs.valid = lambda rel, data: data == new.encode() if rel == "tessera/look.lua" else real(rel, data)
        mine = LOOK.replace("gaps_1 = 1", "gaps_1 = 10")
        (self.hypr / "tessera/look.lua").write_text(mine)
        rep = self.update({"tessera/look.lua": new})
        self.assertEqual(self.mine("tessera/look.lua"), mine)
        self.assertEqual(self.mine("tessera/look.lua.new"), new)
        self.assertIn("look.lua: kept as yours (Tessera's changes didn't merge) — see look.lua.new", rep["hypr"]["notes"])
        self.assertEqual(len(self.notes), 1)

    def test_lua_that_merges_is_checked_with_luac(self):
        if self.cs.lua_ok(b"x = 1\n") is None:
            self.skipTest("no luac here")
        self.assertFalse(self.cs.valid("tessera/look.lua", b"hl.config({\n"))
        self.assertTrue(self.cs.valid("tessera/look.lua", LOOK.encode()))
        self.assertFalse(self.cs.valid("tessera/keymap.json", b"[1,,]"))

    def test_invalid_json_from_a_line_merge_falls_back_to_keys_or_yours(self):
        # yours isn't JSON any more: nothing can merge it, so yours stays with a .new
        (self.hypr / "tessera/gestures.json").write_text('[{"id": "move", "fingers": 3,,]\n')
        new = jtext([dict(GESTURES[0], fingers=2), GESTURES[1]])
        self.update({"tessera/gestures.json": new})
        self.assertEqual(self.mine("tessera/gestures.json"), '[{"id": "move", "fingers": 3,,]\n')
        self.assertEqual(self.mine("tessera/gestures.json.new"), new)
        self.assertTrue(any("gestures.json: kept as yours" in b for _, b in self.notes))

    def test_a_file_you_deleted_stays_deleted(self):
        (self.hypr / "tessera/tabs.lua").unlink()
        self.update({"tessera/tabs.lua": "-- tabs, changed\n"})
        self.assertIsNone(self.mine("tessera/tabs.lua"))  # (hyprland.lua loads it with pcall)
        self.assertEqual(self.notes, [])

    def test_a_needed_file_you_deleted_comes_back(self):
        (self.hypr / "tessera/look.lua").unlink()
        (self.hypr / "tessera/json.lua").unlink()  # (keybinds.lua's dofile)
        (self.hypr / "tessera/keymap.json").unlink()  # (keybinds.lua's json.read, no "or")
        (self.hypr / "tessera/gestures.json").unlink()  # (read with "or {}": not needed)
        rep = self.update({})
        self.assertEqual(self.mine("tessera/look.lua"), LOOK)
        self.assertEqual(self.mine("tessera/json.lua"), "return {}\n")
        self.assertIsNotNone(self.mine("tessera/keymap.json"))
        self.assertIsNone(self.mine("tessera/gestures.json"))
        self.assertEqual(sorted(rep["hypr"]["restored"]), ["tessera/json.lua", "tessera/keymap.json", "tessera/look.lua"])
        self.assertIn("look.lua: put back", self.notes[0][1])

    def test_a_module_you_unhooked_may_stay_deleted(self):
        # (you took look.lua out of hyprland.lua too: nothing needs it)
        (self.hypr / "hyprland.lua").write_text(HYPRLAND.replace('dofile(dir .. "/tessera/look.lua")\n', ""))
        (self.hypr / "tessera/look.lua").unlink()
        self.update({})
        self.assertIsNone(self.mine("tessera/look.lua"))

    def test_a_file_tessera_dropped_goes_only_if_you_never_edited_it(self):
        (self.hypr / "tessera/tabs.lua").write_text("-- tabs, mine\n")
        rep = self.update({"tessera/old.lua": None, "tessera/tabs.lua": None})
        self.assertIsNone(self.mine("tessera/old.lua"))
        self.assertEqual(self.mine("tessera/tabs.lua"), "-- tabs, mine\n")
        self.assertIn("tessera/old.lua", rep["hypr"]["removed"])
        self.assertEqual((Path(rep["hypr"]["backup"]) / "hypr/tessera/old.lua").read_text(), "-- old\n")
        # files of yours Tessera never shipped are never touched
        self.assertEqual(self.mine("custom.lua"), "-- mine\n")

    def test_files_the_installer_writes_are_never_touched(self):
        rep = self.update({"tessera/keyboard.lua": "-- shipped by mistake\n", "tessera/gpu.lua": "-- nope\n"})
        self.assertEqual(self.mine("tessera/keyboard.lua"), "-- your layout: de\n")
        self.assertIsNone(self.mine("tessera/gpu.lua"))
        self.assertNotIn("tessera/keyboard.lua", json.dumps(rep))
        self.assertFalse((self.data / "tessera/hypr-base/tessera/keyboard.lua").exists())

    def test_no_base_yet_merges_against_the_version_updated_from(self):
        shutil.rmtree(self.data / "tessera/hypr-base")
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('"Q", "close"', '"W", "close"'))
        rep = self.update({"tessera/keybinds.lua": KEYBINDS.replace('"lock"', '"lock-now"'),
                           "tessera/look.lua": LOOK.replace("gaps_1 = 1", "gaps_1 = 10")})
        self.assertEqual(rep["hypr"]["base"], "git")
        got = self.mine("tessera/keybinds.lua")
        self.assertIn('"W", "close"', got)
        self.assertIn('"lock-now"', got)
        self.assertEqual(self.mine("tessera/look.lua"), LOOK.replace("gaps_1 = 1", "gaps_1 = 10"))
        self.assertEqual(self.notes, [])
        self.assertTrue((self.data / "tessera/hypr-base/.base.json").exists())  # (kept from now on)

    def test_no_base_and_no_history_keeps_what_differs_with_a_new(self):
        shutil.rmtree(self.data / "tessera/hypr-base")
        (self.hypr / "tessera/keybinds.lua").write_text(KEYBINDS.replace('"Q", "close"', '"W", "close"'))
        new = KEYBINDS.replace('"lock"', '"lock-now"')
        self.commit({"tessera/keybinds.lua": new, "tessera/look.lua": LOOK})
        rep = self.cs.sync(str(self.repo), None)
        self.assertEqual(rep["hypr"]["base"], "none")
        self.assertEqual(self.mine("tessera/keybinds.lua"), KEYBINDS.replace('"Q", "close"', '"W", "close"'))
        self.assertEqual(self.mine("tessera/keybinds.lua.new"), new)
        self.assertFalse((self.hypr / "tessera/look.lua.new").exists())  # (the same as the new one: fine)
        self.assertIn("keybinds.lua: kept as yours", self.notes[0][1])

    def test_the_shell_copy_is_replaced(self):
        (self.config / "quickshell/tessera/stale.qml").write_text("old\n")
        (self.repo / "shell/shell.qml").write_text("// v2\n")
        rep = self.update({})
        self.assertEqual(rep["shell"], "replaced")
        self.assertEqual((self.config / "quickshell/tessera/shell.qml").read_text(), "// v2\n")
        self.assertFalse((self.config / "quickshell/tessera/stale.qml").exists())
        self.assertEqual([p.name for p in (self.config / "quickshell").iterdir()], ["tessera"])  # (nothing left over)

    def test_a_dev_install_keeps_its_links(self):
        shutil.rmtree(self.config / "quickshell/tessera")
        (self.config / "quickshell/tessera").symlink_to(self.repo / "shell")
        rep = self.update({"tessera/look.lua": LOOK + "-- v2\n"})
        self.assertEqual(rep["shell"], "linked")
        self.assertTrue((self.config / "quickshell/tessera").is_symlink())
        self.assertEqual(self.mine("tessera/look.lua"), LOOK + "-- v2\n")  # (hypr is a copy there too: merged)
        # hypr/tessera linked to the checkout: left alone
        shutil.rmtree(self.hypr / "tessera")
        (self.hypr / "tessera").symlink_to(self.repo / "hypr/tessera")
        rep = self.update({"tessera/look.lua": LOOK + "-- v3\n"})
        self.assertEqual(rep["hypr"]["why"], "linked")
        self.assertTrue((self.hypr / "tessera").is_symlink())

    def test_nothing_without_an_install(self):
        shutil.rmtree(self.hypr)
        shutil.rmtree(self.config / "quickshell")
        rep = self.update({"tessera/look.lua": LOOK + "-- v2\n"})
        self.assertEqual(rep["shell"], "none")
        self.assertEqual(rep["hypr"]["why"], "none")
        self.assertFalse(self.hypr.exists())

    def test_a_settled_clash_takes_its_new_away(self):
        (self.hypr / "tessera/tabs.lua").write_text("-- tabs, mine\n")
        self.update({"tessera/tabs.lua": "-- tabs, theirs\n"})
        self.assertTrue((self.hypr / "tessera/tabs.lua.new").exists())
        (self.hypr / "tessera/tabs.lua").write_text("-- tabs, theirs\n")  # (you took the new one)
        self.cs.sync(str(self.repo), self.v1)
        self.assertFalse((self.hypr / "tessera/tabs.lua.new").exists())


class Wiring(unittest.TestCase):
    def test_install_writes_the_base_and_update_merges(self):
        text = (ROOT / "install.sh").read_text()
        self.assertIn('tools/config-sync.py" base', text)
        self.assertIn('tools/config-sync.py" sync --no-shell --old-rev "$TESSERA_UPDATE_FROM"', text)
        self.assertIn("TESSERA_UPDATE_FROM=", (ROOT / "tools/tessera").read_text())

    def test_uninstall_removes_the_base(self):
        text = (ROOT / "uninstall.sh").read_text()
        self.assertIn('"$d/hypr-base"', text)
        self.assertIn("config-sync.json", text)

    def test_the_update_syncs_after_it_applies(self):
        text = (ROOT / "tools/tessera-update.py").read_text()
        self.assertIn("config_sync = sync_installed(old_head, backup)", text)
        self.assertLess(text.index("config_sync = sync_installed(old_head, backup)"), text.index("\"migrate-names.py\"), \"--checkout\""))

    def test_the_updaters_sync_is_never_fatal(self):
        name = "tu_sync_" + os.urandom(3).hex()
        loader = importlib.machinery.SourceFileLoader(name, str(ROOT / "tools/tessera-update.py"))
        spec = importlib.util.spec_from_loader(name, loader)
        tu = importlib.util.module_from_spec(spec)
        loader.exec_module(tu)
        tu.CONFIG_SYNC = "/nonexistent/config-sync.py"
        self.assertIn("error", tu.sync_installed("HEAD", ""))


if __name__ == "__main__":
    unittest.main()
