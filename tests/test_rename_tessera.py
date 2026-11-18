"""Tessera was called iRice until 4 November 2026. An install from before
updates itself across the rename: its folders, its checkout (~/iRice →
~/tessera), its settings and files move over once (migrate-names.py, at the
shell's start), the root setup moves the system pieces and the login screen's
entry, the old environment names count until the removal date, and the old
names live only in the migration modules. Everything runs in temporary
folders, with fakes for sudo, pkexec, systemctl and qs; nothing real is
changed.

This file is one of the places allowed to name the old name (with the
modules: docs/rename-to-tessera.md has the list). REMOVE AFTER 2027-02-01."""
import datetime
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SHELL = ROOT / "shell"
SCRIPTS = SHELL / "assets/scripts"
BUNDLE = Path.home() / ".local/share/tessera/quickshell"
QS = BUNDLE / "bin/qs"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MIG = load(SCRIPTS / "migrate-names.py", "migrate_names")
R = load(SCRIPTS / "tessera-root.py", "tessera_root_rename")
DAY = 24 * 3600


class Temp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.home = self.d / "home"
        self.home.mkdir()
        self.xdg = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.home / ".config"),
                    "XDG_DATA_HOME": str(self.home / ".local/share"), "XDG_STATE_HOME": str(self.home / ".local/state"),
                    "XDG_CACHE_HOME": str(self.home / ".cache"), "TESSERA_SYSTEM_ROOT": str(self.d / "sys")}
        self.bin = self.d / "bin"
        self.bin.mkdir()
        self.calls = []

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, path, text="", mode=None):
        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
        if mode:
            p.chmod(mode)
        return p

    def fake(self, name, script):
        self.write(self.bin / name, "#!/bin/sh\n" + textwrap.dedent(script), 0o755)

    def run_fake(self, args, **kw):
        """subprocess.run's stand-in: notes every call, and runs only git
        (on the temporary checkouts); the rest (systemctl, the file-manager
        menus) only answer"""
        self.calls.append(list(args))
        if args[0] == "git":
            return subprocess.run(args, **kw)
        return subprocess.CompletedProcess(args, 0, "", "")

    def checkout(self, where, remote=None):
        """A Tessera checkout with what an update and a start need."""
        where = Path(where)
        for rel in ("shell/assets/scripts/launch-shell.sh", "shell/assets/scripts/old-names.sh", "shell/assets/scripts/migrate-names.py",
                    "shell/assets/scripts/file-manager-share.py", "sandbox/tessera-session.sh", "sandbox/irice-session.sh"):
            (where / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / rel, where / rel)
        self.write(where / "shell/shell.qml", "import Quickshell\nShellRoot {}\n")
        subprocess.run(["git", "init", "-q", str(where)], check=True)
        if remote:
            subprocess.run(["git", "-C", str(where), "remote", "add", "origin", remote], check=True)
        return where


# ── Your folders: moved once, the old names left as links ────────────────
class Folders(Temp):
    def old_layout(self):
        c = self.home / ".config/irice"
        self.write(c / "config.json", '{"appearance": {"menuIcon": "irice"}}')
        self.write(c / "monitors.lua", "-- displays\n")
        self.write(self.home / ".local/share/irice/bin/irice-events", "#!/bin/sh\n", 0o755)
        self.write(self.home / ".local/share/irice/whisper/ggml-base.bin", "model")
        self.write(self.home / ".local/state/irice/state.json", '{"todos": ["milk"]}')
        self.write(self.home / ".cache/irice/rates.json", "{}")

    def test_old_folders_move_and_the_old_names_become_links(self):
        self.old_layout()
        os.symlink(self.home / ".local/share/irice/whisper", self.home / ".local/share/irice/models")  # (absolute, into the old folder)
        self.assertEqual(MIG.migrate(self.xdg), {"config": "moved", "data": "moved", "state": "moved", "cache": "moved"})
        cfg = self.home / ".config"
        self.assertTrue((cfg / "irice").is_symlink())
        self.assertEqual(os.readlink(cfg / "irice"), "tessera")  # (relative: the home can move)
        self.assertEqual((cfg / "irice/monitors.lua").read_text(), "-- displays\n")
        self.assertEqual((self.home / ".local/share/tessera/whisper/ggml-base.bin").read_text(), "model")
        self.assertEqual(os.readlink(self.home / ".local/share/tessera/models"), str(self.home / ".local/share/tessera/whisper"))
        self.assertIn("Moved here from", (cfg / "tessera" / MIG.NOTE).read_text())

    def test_already_moved_is_left_alone(self):
        self.old_layout()
        MIG.migrate(self.xdg)
        self.assertEqual(set(MIG.migrate(self.xdg).values()), {"linked"})

    def test_both_there_keeps_the_new_and_loses_nothing(self):
        self.old_layout()
        self.write(self.home / ".config/tessera/config.json", '{"new": true}')
        self.assertEqual(MIG.migrate(self.xdg)["config"], "merged")
        cfg = self.home / ".config"
        self.assertEqual((cfg / "tessera/config.json").read_text(), '{"new": true}')
        self.assertEqual((cfg / "tessera/monitors.lua").read_text(), "-- displays\n")
        self.assertTrue((cfg / "irice").is_symlink())
        self.assertEqual((cfg / "irice.before-tessera/config.json").read_text(), '{"appearance": {"menuIcon": "irice"}}')

    def test_a_console_session_moves_your_homes_own_folders_too(self):
        # (XDG_* point at a sandbox home; ~/.local/share/irice is your own)
        self.write(self.home / ".local/share/irice/previous-login-manager", "sddm\n")
        self.write(self.home / ".local/share/tessera/bin/tessera-ui", "")
        env = dict(self.xdg, XDG_DATA_HOME=str(self.d / "sandbox/.local/share"))
        self.assertEqual(MIG.migrate(env)["home data"], "merged")
        self.assertEqual((self.home / ".local/share/tessera/previous-login-manager").read_text(), "sddm\n")

    def test_dry_run_changes_nothing(self):
        self.old_layout()
        self.assertEqual(MIG.migrate(self.xdg, dry=True)["config"], "would move")
        self.assertFalse((self.home / ".config/tessera").exists())

    def test_never_stops_a_shell_from_starting(self):
        self.old_layout()
        os.chmod(self.home / ".config", 0o500)  # (can't rename in there)
        try:
            p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "migrate-names.py")], env=dict(os.environ, **self.xdg),
                               capture_output=True, text=True, timeout=60)
        finally:
            os.chmod(self.home / ".config", 0o700)
        self.assertEqual(p.returncode, 0)
        self.assertIn("config: error", p.stderr)


# ── The checkout: ~/iRice → ~/tessera ───────────────────────────────────
class Checkout(Temp):
    def old_install(self):
        """A console-session install from before: run from ~/iRice."""
        old = self.checkout(self.home / "iRice", remote="https://github.com/monishbl/iRice.git")
        sb = old / ".sandbox/home"
        self.write(sb / ".config/tessera/config.json", json.dumps(
            {"wallpaper": {"path": f"{old}/material/assets/wallpapers/material-dark.jpg"}, "appearance": {"menuIcon": "irice"}}))
        self.write(sb / ".config/tessera/keybinds.json", json.dumps({"shortcuts": [{"exec": "$IRICE_SHELL/assets/scripts/x.sh"}]}))
        self.write(sb / ".config/qt6ct/qt6ct.conf", f"[Appearance]\ncolor_scheme_path={old}/.sandbox/home/.config/qt6ct/colors/TesseraDesktop.conf\n")
        self.write(sb / ".config/tessera/MOVED-FROM-IRICE.txt", "2026-11-04: moved\n")
        self.write(self.home / ".local/share/tessera/source", f"{old}\n")
        self.write(self.home / ".config/systemd/user/tessera-battery-guard.service",
                   f"[Service]\nExecStart=/usr/bin/env python3 -I {old}/shell/assets/scripts/battery-guard.py --sleep-at 3\n")
        self.write(self.home / "iRice-notes/keep.txt", f"{old}2 is not {old}\n")  # (a longer name is never touched)
        os.symlink(old / "shell", sb / ".config/tessera/shell-link")
        return old

    def test_the_checkout_moves_and_everything_points_at_it(self):
        old = self.old_install()
        out = MIG.run_all(self.xdg, given=[str(old)], run=self.run_fake)
        new = self.home / "tessera"
        self.assertEqual(out["checkout"], "moved")
        self.assertTrue((new / "shell/shell.qml").is_file())
        self.assertTrue(old.is_symlink())
        self.assertEqual(os.readlink(old), "tessera")
        sb = new / ".sandbox/home"
        cfg = json.loads((sb / ".config/tessera/config.json").read_text())
        self.assertEqual(cfg["wallpaper"]["path"], f"{new}/material/assets/wallpapers/material-dark.jpg")
        self.assertEqual(cfg["appearance"]["menuIcon"], "tessera")
        self.assertIn("$TESSERA_SHELL/assets/scripts/x.sh", (sb / ".config/tessera/keybinds.json").read_text())
        self.assertIn(f"color_scheme_path={new}/", (sb / ".config/qt6ct/qt6ct.conf").read_text())
        self.assertEqual((self.home / ".local/share/tessera/source").read_text(), f"{new}\n")
        self.assertIn(f"{new}/shell/assets/scripts/battery-guard.py", (self.home / ".config/systemd/user/tessera-battery-guard.service").read_text())
        self.assertEqual(os.readlink(sb / ".config/tessera/shell-link"), f"{new}/shell")
        self.assertEqual((self.home / "iRice-notes/keep.txt").read_text(), f"{old}2 is not {old}\n")
        # the note: with the settings, never in the checkout; the 4 Nov one folded in
        note = (sb / ".config/tessera" / MIG.NOTE).read_text()
        self.assertIn("2026-11-04: moved", note)
        self.assertFalse((sb / ".config/tessera/MOVED-FROM-IRICE.txt").exists())
        self.assertFalse((new / MIG.NOTE).exists())
        # the repository's old address: its own
        remote = subprocess.run(["git", "-C", str(new), "remote", "get-url", "origin"], capture_output=True, text=True).stdout.strip()
        self.assertEqual(remote, "https://github.com/monishbl/tessera.git")
        self.assertIn(["git", "-C", str(new), "worktree", "repair"], self.calls)
        self.assertIn(["systemctl", "--user", "daemon-reload"], self.calls)
        # once: nothing more the next time
        self.assertNotIn("checkout", MIG.run_all(self.xdg, given=[str(old)], run=self.run_fake))

    def test_only_the_checkout_at_irice_and_never_over_another(self):
        elsewhere = self.checkout(self.home / "src/tessera")
        self.assertNotIn("checkout", MIG.run_all(self.xdg, given=[str(elsewhere)], run=self.run_fake))
        self.assertTrue(elsewhere.is_dir())
        old = self.old_install()
        self.write(self.home / "tessera/mine.txt", "yours")
        self.assertTrue(MIG.run_all(self.xdg, given=[str(old)], run=self.run_fake)["checkout"].startswith("error"))
        self.assertTrue(old.is_dir() and not old.is_symlink())
        self.assertEqual((self.home / "tessera/mine.txt").read_text(), "yours")

    def test_a_build_never_moves_the_checkout_it_runs_in(self):
        old = self.old_install()
        self.assertNotIn("checkout", MIG.run_all(self.xdg, given=[str(old)], run=self.run_fake, move=False))
        self.assertFalse(old.is_symlink())

    def test_the_link_goes_once_nothing_uses_it(self):
        old = self.old_install()
        MIG.run_all(self.xdg, given=[str(old)], run=self.run_fake)
        fake_proc = self.d / "proc"
        fake_proc.mkdir()
        later = time.time() + 15 * DAY
        # a login screen's entry still names it: kept
        entry = self.write(self.d / "sys/usr/share/wayland-sessions/tessera.desktop", f"Exec={old}/sandbox/tessera-session.sh\n")
        self.assertEqual(MIG.old_link(str(old), str(self.home / "tessera"), self.xdg, now=later, proc=str(fake_proc)), "kept")
        entry.write_text(f"Exec={self.home}/tessera/sandbox/tessera-session.sh\n")
        # a program started from it (its environment names it): kept
        p = subprocess.Popen(["sleep", "30"], env=dict(os.environ, TESSERA_ROOT=str(old)))
        try:
            time.sleep(0.2)
            self.assertEqual(MIG.old_link(str(old), str(self.home / "tessera"), self.xdg, now=later), "kept")
        finally:
            p.kill()
            p.wait()
        # too new: kept
        self.assertEqual(MIG.old_link(str(old), str(self.home / "tessera"), self.xdg, proc=str(fake_proc)), "kept")
        # old enough and unused: gone
        self.assertEqual(MIG.old_link(str(old), str(self.home / "tessera"), self.xdg, now=later, proc=str(fake_proc)), "removed")
        self.assertFalse(os.path.lexists(old))
        # a link of yours elsewhere is never ours
        os.symlink(self.home / "src", old)
        self.assertEqual(MIG.old_link(str(old), str(self.home / "tessera"), self.xdg, now=later, proc=str(fake_proc)), "none")


# ── A fake old install, updated, then started: it works ─────────────────
FAKE_QS = r"""#!/bin/sh
if [ "$3" = ipc ] && [ "$4" = show ]; then echo "No running instances"; exit 0; fi
printf '%s\n' "$@" > "$QS_CALL"
env > "$QS_CALL.env"
"""


class OldInstallUpdated(Temp):
    def test_an_old_install_starts_from_its_new_folder(self):
        old = self.checkout(self.home / "iRice", remote="git@github.com:monishbl/iRice.git")
        sb = old / ".sandbox/home"
        # (as before the 4 Nov move too: the console session's folders under the old name)
        self.write(sb / ".config/irice/config.json", json.dumps({"appearance": {"look": "glass", "menuIcon": "irice"},
                                                                 "rust": {"standalone": False}}))
        self.write(sb / ".local/state/irice/state.json", json.dumps({"_irice_revisions": {"a": [1, "x", 0]}}))
        self.write(self.bin / "qs", FAKE_QS, 0o755)
        call = self.d / "call"
        env = dict(os.environ, HOME=str(self.home), XDG_RUNTIME_DIR=str(self.d), QS_CALL=str(call), PATH=f"{self.bin}:/usr/bin:/bin",
                   QS_ICON_THEME="hicolor", TESSERA_SYSTEM_ROOT=str(self.d / "sys"), TESSERA_TTY="1", IRICE_LAB="1",
                   XDG_CONFIG_HOME=str(sb / ".config"), XDG_DATA_HOME=str(sb / ".local/share"),
                   XDG_STATE_HOME=str(sb / ".local/state"), XDG_CACHE_HOME=str(sb / ".cache"), TESSERA_ROOT=str(old))
        for k in ("HYPRLAND_INSTANCE_SIGNATURE", "TESSERA_SHELL", "TESSERA_STANDALONE", "TESSERA_LAB"):
            env.pop(k, None)
        # the session's start after the update, as the console session does it
        subprocess.run(["bash", str(old / "shell/assets/scripts/launch-shell.sh"), "-p", str(old / "shell")], env=env, timeout=90,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        new = self.home / "tessera"
        self.assertTrue(old.is_symlink() and (new / "shell/shell.qml").is_file())
        # the shell started, from the new folder, with the new names
        self.assertEqual(call.read_text().split("\n")[:2], ["-p", str(new / "shell")])
        started = dict(l.split("=", 1) for l in Path(str(call) + ".env").read_text().splitlines() if "=" in l)
        self.assertEqual(started.get("TESSERA_LAB"), "1")  # (an old environment name still counts)
        self.assertEqual(started.get("TESSERA_SHELL"), "1")
        self.assertNotIn("IRICE_SHELL", started)
        # its settings moved and say Tessera
        nsb = new / ".sandbox/home"
        self.assertEqual(json.loads((nsb / ".config/tessera/config.json").read_text())["appearance"]["menuIcon"], "tessera")
        self.assertIn("_tessera_revisions", json.loads((nsb / ".local/state/tessera/state.json").read_text()))
        remote = subprocess.run(["git", "-C", str(new), "remote", "get-url", "origin"], capture_output=True, text=True).stdout.strip()
        self.assertEqual(remote, "git@github.com:monishbl/tessera.git")
        # and the login screen's entry from before still starts it, through the link
        self.write(new / "sandbox/tessera-session.sh", f'#!/bin/sh\necho "tessera $*" > {self.d}/ran\n', 0o755)
        subprocess.run([str(old / "sandbox/irice-session.sh"), "a"], timeout=10, check=True)
        self.assertEqual((self.d / "ran").read_text(), "tessera a\n")


# ── Settings and files saved under the old name ─────────────────────────
class Files(Temp):
    def test_settings_values_take_the_new_name(self):
        f = self.home / "cfg"
        self.write(f / "config.json", json.dumps({"appearance": {"menuIcon": "irice"}, "screenshot": {"editor": "irice"}}))
        self.write(f / "spaces.lua", 'IRICE_SPACE_BASES = { ["name:DP-1"] = 100 }\n')
        self.write(f / "state.json", json.dumps({"_irice_revisions": {"k": [2, "a", 1]}, "todos": []}))
        done = MIG.settings(str(f))
        self.assertEqual(len(done), 3)
        c = json.loads((f / "config.json").read_text())
        self.assertEqual((c["appearance"]["menuIcon"], c["screenshot"]["editor"]), ("tessera", "tessera"))
        self.assertIn("TESSERA_SPACE_BASES", (f / "spaces.lua").read_text())
        s = json.loads((f / "state.json").read_text())
        self.assertEqual(s, {"_tessera_revisions": {"k": [2, "a", 1]}, "todos": []})
        self.assertEqual(MIG.settings(str(f)), [])

    def test_your_files_from_before(self):
        c, data = self.home / ".config", self.home / ".local/share"
        self.write(c / "hypr/irice/keyboard.lua", "-- yours\n")
        self.write(c / "hypr.pre-irice/hyprland.conf", "# your own\n")
        self.write(c / "fish/conf.d/irice.fish", "fish_add_path\n")
        self.write(c / "gtk-3.0/gtk.css", "@import 'irice-colors.css';\n")
        self.write(c / "gtk-3.0/irice-colors.css", "x")
        self.write(c / "kdeglobals", "[General]\nColorScheme=iRiceDesktopDark\n[KDE]\nColorScheme=iRiceOther\n")
        self.write(c / "autostart/app.desktop", "[Desktop Entry]\nOnlyShowIn=irice;KDE;\n")
        self.write(data / "tessera/bin/irice-events", "#!/bin/sh\n", 0o755)
        self.write(data / "tessera/irice-glass.so", "so")
        self.write(data / "nemo/actions/irice-share.nemo_action", "x")
        self.write(self.home / ".local/bin/irice", "#!/bin/sh\n# iRice's command\n", 0o755)
        self.write(self.home / ".local/bin/irice-other", "mine")
        self.write(c / "systemd/user/irice-battery-guard.service", "[Service]\n")
        MIG.user_files(self.xdg, run=self.run_fake)
        self.assertEqual((c / "hypr/tessera/keyboard.lua").read_text(), "-- yours\n")
        self.assertTrue((c / "hypr/irice").is_symlink())
        self.assertEqual((c / "hypr.pre-tessera/hyprland.conf").read_text(), "# your own\n")
        self.assertTrue((c / "fish/conf.d/tessera.fish").exists())
        self.assertEqual((c / "gtk-3.0/gtk.css").read_text(), "@import 'tessera-colors.css';\n")
        self.assertFalse((c / "gtk-3.0/irice-colors.css").exists())
        self.assertEqual((c / "kdeglobals").read_text(), "[General]\nColorScheme=TesseraDesktopDark\n[KDE]\nColorScheme=iRiceOther\n")
        self.assertEqual((c / "autostart/app.desktop").read_text(), "[Desktop Entry]\nOnlyShowIn=Tessera;KDE;\n")
        self.assertTrue(os.access(data / "tessera/bin/tessera-events", os.X_OK))  # (the same helper, under its name)
        self.assertFalse((data / "tessera/irice-glass.so").exists())  # (answers only to the old name: goes)
        self.assertFalse((data / "nemo/actions/irice-share.nemo_action").exists())
        self.assertIn([sys.executable, "-I", str(SCRIPTS / "file-manager-share.py"), "install", "nemo"], self.calls)
        self.assertFalse((self.home / ".local/bin/irice").exists())
        self.assertTrue((self.home / ".local/bin/irice-other").exists())
        self.assertFalse((c / "systemd/user/irice-battery-guard.service").exists())
        self.assertIn(["systemctl", "--user", "disable", "--now", "irice-battery-guard.service"], self.calls)

    def test_remotes_at_the_old_address_point_at_the_new(self):
        repo = self.checkout(self.d / "repo", remote="https://github.com/monishbl/iRice")
        subprocess.run(["git", "-C", str(repo), "remote", "add", "mirror", "https://gitlab.com/monishbl/iRice.git"], check=True)
        subprocess.run(["git", "-C", str(repo), "remote", "add", "fork", "https://github.com/someone/iRice.git"], check=True)
        MIG.remotes(str(repo))
        url = lambda r: subprocess.run(["git", "-C", str(repo), "remote", "get-url", r], capture_output=True, text=True).stdout.strip()
        self.assertEqual(url("origin"), "https://github.com/monishbl/tessera")
        self.assertEqual(url("mirror"), "https://gitlab.com/monishbl/tessera.git")
        self.assertEqual(url("fork"), "https://github.com/someone/iRice.git")


# ── The root helper: the old one until setup, then setup moves the rest ─
class HelperChoice(Temp):
    def system(self, *helpers):
        s = self.d / "sys"
        for h in helpers:
            self.write(s / h, "#!/bin/sh\n", 0o755)
        return s

    def python_choice(self, code, sys_root):
        p = subprocess.run([sys.executable, "-c", code], env=dict(os.environ, TESSERA_SYSTEM_ROOT=str(sys_root)),
                           capture_output=True, text=True, timeout=20, cwd=str(SCRIPTS / "quickshare"))
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout.strip().replace(str(sys_root), "")

    def test_sharing_check_uses_the_helper_thats_there(self):
        code = f"import importlib.util as u; s = u.spec_from_file_location('c', {str(SCRIPTS / 'sharing-check.py')!r}); m = u.module_from_spec(s); s.loader.exec_module(m); print(m.HELPER)"
        self.assertEqual(self.python_choice(code, self.system()), "/usr/lib/tessera/tessera-root")
        self.assertEqual(self.python_choice(code, self.system("usr/lib/irice/irice-root")), "/usr/lib/irice/irice-root")
        self.assertEqual(self.python_choice(code, self.system("usr/lib/tessera/tessera-root")), "/usr/lib/tessera/tessera-root")

    def test_quick_shares_wifi_helper_too(self):
        code = "from nearby import wifi; print(wifi.installed_root_helper())"
        self.assertEqual(self.python_choice(code, self.system("usr/lib/irice/irice-root")), "/usr/lib/irice/irice-root")
        self.assertEqual(self.python_choice(code, self.system("usr/lib/tessera/tessera-root")), "/usr/lib/tessera/tessera-root")

    @unittest.skipUnless(QS.exists(), "requires bundled Quickshell")
    def test_the_shells_pkexec_calls(self):
        # RootHelper.qml's command, as the shell builds it (with OldNames.qml),
        # run with /usr in a temporary folder and a pkexec that says what it was asked
        shell = self.d / "shell"
        (shell / "services").mkdir(parents=True)
        (shell / "common").mkdir(parents=True)
        shutil.copy2(SHELL / "services/RootHelper.qml", shell / "services/RootHelper.qml")
        shutil.copy2(SHELL / "common/OldNames.qml", shell / "common/OldNames.qml")
        (shell / "shell.qml").write_text('import QtQuick\nimport Quickshell\nimport qs.services\nShellRoot { settings.watchFiles: false\n'
                                         'Timer { interval: 50; running: true; onTriggered: { console.log("CMD " + JSON.stringify(RootHelper.command(["priority", "on"]))); '
                                         'console.log("KNOWS " + JSON.stringify(RootHelper.knows("login-look"))); console.log("MSG " + RootHelper.message("irice-root: no")); Qt.quit(); } }\n}\n')
        rt = self.d / "rt"
        rt.mkdir(mode=0o700)
        p = subprocess.run([str(QS), "-p", str(shell)], env=dict(os.environ, XDG_RUNTIME_DIR=str(rt), QT_QPA_PLATFORM="offscreen",
                                                                   QML_IMPORT_PATH=str(BUNDLE / "lib/qt6/qml")), capture_output=True, text=True, timeout=20)
        out = p.stdout + p.stderr
        cmd = json.loads(re.search(r"CMD (\[.*\])", out).group(1))
        knows = json.loads(re.search(r'KNOWS (".*")', out).group(1))
        self.assertIn("MSG no", out)
        self.fake("pkexec", 'echo "$@"\n')
        sysroot = self.d / "sys"

        def run(script, *args):
            script = script.replace("/usr/", f"{sysroot}/usr/")
            r = subprocess.run(["sh", "-c", script, "sh", *args], env=dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin"),
                               capture_output=True, text=True, timeout=10)
            return r.returncode, r.stdout.strip().replace(str(sysroot), "")
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/tessera/tessera-root priority on"))
        self.assertNotEqual(run(knows)[0], 0)
        self.system("usr/lib/irice/irice-root")
        self.write(sysroot / "usr/share/polkit-1/actions/org.irice.root.policy", '<action id="org.irice.root.login-look">')
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/irice/irice-root priority on"))
        self.assertEqual(run(knows)[0], 0)
        self.system("usr/lib/tessera/tessera-root")
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/tessera/tessera-root priority on"))


class RootSetup(Temp):
    """setup, into a folder that has what the helper installed under the old name."""

    def old_system(self):
        s = self.d / "sys"
        w = lambda p, t="", m=None: self.write(s / p, t, m)
        w("usr/lib/irice/irice-root", "#!/usr/bin/python3\n", 0o755)
        w("usr/lib/irice/plymouth/irice-breathe/irice-breathe.plymouth", "")
        w("usr/share/polkit-1/actions/org.irice.root.policy", "<policyconfig/>")
        w("etc/polkit-1/rules.d/49-irice.rules", "// iRice\n")
        w("etc/polkit-1/rules.d/49-irice-quickshare.rules", 'if (subject.user === "me" && subject.active && subject.local)\n')
        w("etc/irice/quickshare-trusted", "me\n")
        w("etc/irice/bootsplash.json", json.dumps({"original": "spinner", "applied": "irice-breathe"}))
        w("var/lib/irice/initramfs-backup/0.img", "x")
        w("etc/ananicy.d/irice.rules", '{ "name": "irice-shell", "type": "LowLatency_RT" }\n')
        w("etc/systemd/system/bluetooth.service.d/irice-no-rap.conf", "# iRice: no Ranging\n[Service]\n")
        w("etc/systemd/system/plymouth-quit.service.d/irice.conf", "# iRice: the splash stays (irice uninstall removes this)\n")
        w("etc/modprobe.d/irice-phonecam.conf", "options v4l2loopback\n")
        w("etc/pam.d/sudo", "#%PAM-1.0\nauth       sufficient   pam_fprintd.so max-tries=5 timeout=10   # iRice fingerprint\nauth include system-auth\n")
        w("etc/pam.d/greetd.pre-irice", "# pam before\n")
        w("etc/greetd/config.toml.pre-irice", "# before\n")
        w("etc/plymouth/plymouthd.conf", "[Daemon]\nTheme=irice-breathe\nShowDelay=0\n")
        for id in ("breathe", "orbit"):
            w(f"usr/share/plymouth/themes/irice-{id}/irice-{id}.plymouth", "")
        return s

    def test_setup_moves_everything_and_removes_the_old(self):
        s = self.old_system()
        written = R.setup(str(SHELL), os.getuid(), prefix=str(s))
        e = lambda p: (s / p).exists() or (s / p).is_symlink()
        for p in ("usr/lib/tessera/tessera-root", "usr/share/polkit-1/actions/org.tessera.root.policy", "etc/polkit-1/rules.d/49-tessera.rules"):
            self.assertTrue(e(p), p)
        for p in ("usr/lib/irice", "usr/share/polkit-1/actions/org.irice.root.policy", "etc/polkit-1/rules.d/49-irice.rules",
                  "etc/polkit-1/rules.d/49-irice-quickshare.rules", "etc/ananicy.d/irice.rules", "etc/irice", "var/lib/irice",
                  "etc/systemd/system/bluetooth.service.d/irice-no-rap.conf", "etc/modprobe.d/irice-phonecam.conf",
                  "usr/share/plymouth/themes/irice-breathe", "usr/share/plymouth/themes/irice-orbit",
                  "etc/greetd/config.toml.pre-irice", "etc/pam.d/greetd.pre-irice"):
            self.assertFalse(e(p), p)
        rule = (s / "etc/ananicy.d/tessera.rules").read_text()
        self.assertIn('"tessera-shell"', rule)
        self.assertNotIn("irice", rule)
        trust = (s / "etc/polkit-1/rules.d/49-tessera-quickshare.rules").read_text()
        self.assertIn('subject.user === "me"', trust)
        self.assertEqual((s / "etc/tessera/quickshare-trusted").read_text(), "me\n")
        self.assertEqual((s / "etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf").read_text(), "# Tessera: no Ranging\n[Service]\n")
        self.assertIn("tessera uninstall", (s / "etc/systemd/system/plymouth-quit.service.d/tessera.conf").read_text())
        self.assertIn("# Tessera fingerprint", (s / "etc/pam.d/sudo").read_text())
        self.assertIn("Theme=tessera-breathe\n", (s / "etc/plymouth/plymouthd.conf").read_text())
        self.assertEqual(json.loads((s / "etc/tessera/bootsplash.json").read_text()), {"original": "spinner", "applied": "tessera-breathe"})
        self.assertEqual((s / "etc/greetd/config.toml.pre-tessera").read_text(), "# before\n")
        self.assertTrue(e("etc/pam.d/greetd.pre-tessera"))
        self.assertIn("/usr/lib/irice", written)
        self.assertEqual(R.setup(str(SHELL), os.getuid(), prefix=str(s)), [])  # (once)

    def test_the_login_screens_entry_takes_tesseras_name_and_folder(self):
        s = self.d / "sys"
        new = self.checkout(self.home / "tessera")
        os.symlink("tessera", self.home / "iRice")
        old = self.home / "iRice"
        sessions = s / "usr/share/wayland-sessions"
        self.write(sessions / "irice.desktop", f"[Desktop Entry]\nName=iRice\nExec={old}/sandbox/irice-session.sh\nType=Application\n")
        done = R.session_entry(str(old / "shell"), str(s))
        self.assertEqual(done, [R.SESSION_ENTRY, R.OLD_SESSION_ENTRY])
        self.assertFalse((sessions / "irice.desktop").exists())
        entry = (sessions / "tessera.desktop").read_text()
        self.assertIn(f"Exec={new}/sandbox/tessera-session.sh\n", entry)
        self.assertIn("Name=Tessera\n", entry)
        self.assertEqual(R.session_entry(str(new / "shell"), str(s)), [])  # (once)
        # an entry that starts the checkout through its old folder name: its own path
        (sessions / "tessera.desktop").write_text(f"[Desktop Entry]\nExec={old}/sandbox/tessera-session.sh\n")
        R.session_entry(str(new / "shell"), str(s))
        self.assertIn(f"Exec={new}/sandbox/tessera-session.sh\n", (sessions / "tessera.desktop").read_text())
        # another checkout's entry, or none: nothing new at the login screen
        other = self.checkout(self.d / "other")
        (sessions / "tessera.desktop").write_text(f"[Desktop Entry]\nExec={other}/sandbox/tessera-session.sh\n")
        self.assertEqual(R.session_entry(str(new / "shell"), str(s)), [])
        (sessions / "tessera.desktop").unlink()
        self.assertEqual(R.session_entry(str(new / "shell"), str(s)), [])
        self.assertFalse((sessions / "tessera.desktop").exists())

    def test_priority_turned_off_stays_off(self):
        s = self.d / "sys"
        self.write(s / "etc/ananicy.d/irice.rules", "x\n")
        self.write(s / "etc/irice/priority-off", "off\n")
        R.setup(str(SHELL), os.getuid(), prefix=str(s))
        self.assertTrue((s / "etc/tessera/priority-off").exists())
        self.assertFalse((s / "etc/ananicy.d/tessera.rules").exists())
        self.assertFalse((s / "etc/ananicy.d/irice.rules").exists())

    def test_the_old_login_screen_is_written_until_its_replaced(self):
        s = self.d / "sys"
        self.write(s / "usr/share/irice/shell/shell.qml", "old")
        self.assertEqual(R.login_share(str(s)), str(s / "usr/share/irice"))
        R.setup(str(SHELL), os.getuid(), prefix=str(s))
        self.assertEqual((s / "usr/share/irice/shell/shell.qml").read_text(), "old")
        self.write(s / "usr/share/tessera/shell/shell.qml", "")
        self.assertEqual(R.login_share(str(s)), str(s / "usr/share/tessera"))

    def test_fingerprint_off_takes_out_old_marks(self):
        etc = self.d / "etc"
        self.write(etc / "sudo", "#%PAM-1.0\nauth sufficient pam_fprintd.so   # iRice fingerprint\nauth include system-auth\n")
        R.fingerprint(False, "sudo", etc=str(etc), vendor=str(self.d / "vendor"), module=())
        self.assertEqual((etc / "sudo").read_text(), "#%PAM-1.0\nauth include system-auth\n")


# ── Environment names, the pending check, the dev container (bash) ──────
class Bash(Temp):
    def bash(self, code, **env):
        e = dict(os.environ, **self.xdg, PATH=f"{self.bin}:/usr/bin:/bin", **env)
        p = subprocess.run(["bash", "-c", f'source {SCRIPTS / "old-names.sh"}\n' + code], env=e, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout

    def test_old_environment_names_count_until_the_new_one_is_set(self):
        out = self.bash('old_names_env; echo "$TESSERA_ARENA $TESSERA_TTY"', IRICE_ARENA="3", IRICE_TTY="1", TESSERA_TTY="0")
        self.assertEqual(out.strip(), "3 0")

    def test_pending_only_while_something_is_left(self):
        self.assertEqual(self.bash("old_names_pending && echo yes || echo no").strip(), "no")
        self.write(self.home / ".cache/irice/x", "")
        self.assertEqual(self.bash("old_names_pending && echo yes || echo no").strip(), "yes")
        shutil.rmtree(self.home / ".cache/irice")
        self.checkout(self.home / "iRice")
        self.assertEqual(self.bash("old_names_pending && echo yes || echo no").strip(), "yes")

    def test_the_dev_container_until_its_renamed(self):
        self.fake("distrobox", 'echo "ID | NAME | STATUS | IMAGE"; echo "abc | $BOXES | Up | arch"\n')
        self.assertEqual(self.bash("old_names_dev_box", BOXES="irice").strip(), "irice")
        self.assertEqual(self.bash("old_names_dev_box", BOXES="tessera").strip(), "tessera")
        self.assertEqual(self.bash("old_names_dev_box", BOXES="other").strip(), "tessera")

    def test_the_installer_renames_root_backups_and_the_entry(self):
        s = self.d / "sys"
        self.fake("sudo", 'exec "$@"\n')
        repo = self.checkout(self.d / "repo")
        sessions = s / "usr/share/wayland-sessions"
        self.write(sessions / "irice.desktop", f"[Desktop Entry]\nName=iRice\nExec={repo}/sandbox/irice-session.sh\nType=Application\n")
        self.write(s / "etc/greetd/config.toml.pre-irice", "# before\n")
        self.write(s / "etc/default/grub.pre-irice", "a")
        self.write(s / "etc/default/grub.pre-tessera", "b")
        self.bash(f'old_names_install "{s}" 0')
        self.assertFalse((sessions / "irice.desktop").exists())
        self.assertIn(f"Exec={repo}/sandbox/tessera-session.sh\n", (sessions / "tessera.desktop").read_text())
        self.assertEqual((s / "etc/greetd/config.toml.pre-tessera").read_text(), "# before\n")
        self.assertEqual((s / "etc/default/grub.pre-tessera").read_text(), "b")  # (Tessera's own wins)


class Uninstall(Temp):
    def test_both_names_go(self):
        s = self.d / "sys"
        log = self.d / "log"
        self.fake("sudo", 'exec "$@"\n')
        self.fake("systemctl", f'echo "systemctl $*" >> {log}; [ "$1" = is-enabled ] && exit 0; exit 0\n')
        h, c, data = self.home, self.home / ".config", self.home / ".local/share"
        for name in ("tessera", "irice"):
            self.write(c / f"quickshell/{name}/shell.qml")
            self.write(h / f".local/bin/{name}", "", 0o755)
            self.write(c / f"fish/conf.d/{name}.fish")
            self.write(data / f"tessera/{name}-glass.so")
            self.write(data / f"tessera/bin/{name}-events")
            self.write(s / f"usr/lib/{name}/x")
            self.write(s / f"usr/share/{name}/shell/greeter.qml")
            self.write(s / f"usr/share/polkit-1/actions/org.{name}.root.policy")
            self.write(s / f"etc/polkit-1/rules.d/49-{name}.rules")
            self.write(s / f"etc/ananicy.d/{name}.rules")
            self.write(s / f"etc/{name}/priority-off")
            self.write(s / f"usr/share/wayland-sessions/{name}.desktop")
            self.write(s / f"etc/systemd/system/plymouth-quit.service.d/{name}.conf")
        self.write(c / "hypr/tessera/look.lua")
        self.write(c / "hypr.pre-irice/hyprland.lua", "-- mine\n")
        self.write(s / "etc/greetd/config.toml", 'command = "/usr/share/irice/greeter-session.sh"\n')
        self.write(s / "etc/greetd/config.toml.pre-irice", "# before\n")
        self.write(s / "etc/pam.d/greetd", "auth include x\n# irice: unlock the keyring with the login password\n-auth optional pam_gnome_keyring.so\n")
        self.write(h / ".bash_profile", "a\n# irice: start Hyprland when logging in on the first console\nexec start-hyprland\nb\n")
        self.write(data / "tessera/previous-login-manager", "sddm\n")
        env = dict(os.environ, **self.xdg, PATH=f"{self.bin}:/usr/bin:/bin")
        p = subprocess.run(["bash", str(ROOT / "uninstall.sh"), "--dry-run"], env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertTrue((s / "usr/lib/irice/x").exists())  # (a dry run changes nothing)
        p = subprocess.run(["bash", str(ROOT / "uninstall.sh"), "--yes"], env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        for name in ("tessera", "irice"):
            for path in (c / f"quickshell/{name}", h / f".local/bin/{name}", c / f"fish/conf.d/{name}.fish", data / f"tessera/{name}-glass.so",
                         data / f"tessera/bin/{name}-events", s / f"usr/lib/{name}", s / f"usr/share/{name}", s / f"usr/share/polkit-1/actions/org.{name}.root.policy",
                         s / f"etc/polkit-1/rules.d/49-{name}.rules", s / f"etc/ananicy.d/{name}.rules", s / f"etc/{name}",
                         s / f"usr/share/wayland-sessions/{name}.desktop", s / f"etc/systemd/system/plymouth-quit.service.d/{name}.conf"):
                self.assertFalse(path.exists(), path)
        self.assertEqual((c / "hypr/hyprland.lua").read_text(), "-- mine\n")
        self.assertEqual((s / "etc/greetd/config.toml").read_text(), "# before\n")
        self.assertEqual((s / "etc/pam.d/greetd").read_text(), "auth include x\n")
        self.assertEqual((h / ".bash_profile").read_text(), "a\nb\n")
        self.assertIn("systemctl enable sddm", log.read_text())


# ── The old name lives only in the migration modules, until they go ─────
ALLOWED = {
    "shell/assets/scripts/migrate-names.py", "shell/assets/scripts/old-names.sh", "shell/assets/scripts/tessera-root.py",
    "native/tessera-ui/core/src/old_names.rs", "shell/common/OldNames.qml", "sandbox/irice-session.sh",
    "site/wrangler.jsonc", "site/bugs/wrangler.jsonc", "site/bugs/README.md", "CHANGELOG.md", "docs/rename-to-tessera.md",
    "tests/test_rename_tessera.py",
}


class Confined(unittest.TestCase):
    @unittest.skipUnless((ROOT / ".git").exists(), "needs the git checkout")
    def test_the_old_name_is_only_in_the_migration_modules(self):
        files = subprocess.run(["git", "-C", str(ROOT), "grep", "-il", "-I", "irice"], capture_output=True, text=True).stdout.split()
        self.assertEqual(sorted(set(files) - ALLOWED), [])
        names = subprocess.run(["git", "-C", str(ROOT), "ls-files"], capture_output=True, text=True).stdout.split()
        self.assertEqual([n for n in names if "irice" in n.lower() and n not in ALLOWED], [])

    def test_the_root_helper_keeps_it_in_its_own_section(self):
        lines = (SCRIPTS / "tessera-root.py").read_text().split("\n")
        start = next(i for i, l in enumerate(lines) if l.startswith("# ── Tessera's old name"))
        end = next(i for i, l in enumerate(lines) if i > start and l.startswith('LIB = "/usr/lib/tessera"'))
        outside = [l for i, l in enumerate(lines) if not start <= i < end and "irice" in l.lower()]
        self.assertEqual(outside, [])

    def test_not_past_its_removal_date(self):
        self.assertEqual(MIG.REMOVE_AFTER, datetime.date(2027, 2, 1))
        self.assertLessEqual(datetime.date.today(), MIG.REMOVE_AFTER,
                             "past the removal date: remove Tessera's old name (docs/rename-to-tessera.md)")


if __name__ == "__main__":
    unittest.main()
