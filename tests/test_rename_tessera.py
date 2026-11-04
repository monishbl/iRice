"""iRice is now Tessera, and a session, an install or a system set up while
it was called iRice keeps working before the one-time setup and after it:
your folders move over once (migrate-names.py, at shell start), the shell
finds the old root helper until the new one is installed and setup then
moves the rest, old environment names still count, and the installer and
uninstaller handle both names. Everything runs in temporary folders, with
fakes for sudo, pkexec and qs; nothing real is changed."""
import importlib.util
import json
import os
import re
import shutil
import stat
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
BUNDLE = next((b for b in (Path.home() / ".local/share/tessera/quickshell", Path.home() / ".local/share/irice/quickshell") if b.exists()),
              Path.home() / ".local/share/tessera/quickshell")
QS = BUNDLE / "bin/qs"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


MIG = load(SCRIPTS / "migrate-names.py", "migrate_names")
R = load(SCRIPTS / "tessera-root.py", "tessera_root_rename")


class Temp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.home = self.d / "home"
        self.home.mkdir()
        self.xdg = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.home / ".config"),
                    "XDG_DATA_HOME": str(self.home / ".local/share"), "XDG_STATE_HOME": str(self.home / ".local/state"),
                    "XDG_CACHE_HOME": str(self.home / ".cache")}
        self.bin = self.d / "bin"
        self.bin.mkdir()

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


# ── Your folders: moved once, the old names left as links ────────────────
class Migration(Temp):
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
        self.assertEqual(MIG.migrate(self.xdg), {"config": "moved", "data": "moved", "state": "moved", "cache": "moved"})
        cfg = self.home / ".config"
        self.assertEqual((cfg / "tessera/config.json").read_text(), '{"appearance": {"menuIcon": "irice"}}')
        self.assertTrue((cfg / "irice").is_symlink())
        self.assertEqual(os.readlink(cfg / "irice"), "tessera")  # (relative: the home can move)
        # anything that still uses the old path finds the same files
        self.assertEqual((cfg / "irice/monitors.lua").read_text(), "-- displays\n")
        self.assertEqual((self.home / ".local/share/tessera/whisper/ggml-base.bin").read_text(), "model")
        self.assertTrue(os.access(self.home / ".local/share/tessera/bin/irice-events", os.X_OK))
        self.assertEqual(json.loads((self.home / ".local/state/tessera/state.json").read_text()), {"todos": ["milk"]})
        self.assertIn("Moved here from", (cfg / "tessera" / MIG.NOTE).read_text())

    def test_already_moved_is_left_alone(self):
        self.old_layout()
        MIG.migrate(self.xdg)
        note = (self.home / ".config/tessera" / MIG.NOTE).read_text()
        self.assertEqual(MIG.migrate(self.xdg), {"config": "linked", "data": "linked", "state": "linked", "cache": "linked"})
        self.assertEqual((self.home / ".config/tessera" / MIG.NOTE).read_text(), note)

    def test_both_there_keeps_the_new_and_loses_nothing(self):
        self.old_layout()
        # (something wrote Tessera's first: its settings stay, iRice's others move over)
        self.write(self.home / ".config/tessera/config.json", '{"new": true}')
        self.assertEqual(MIG.migrate(self.xdg)["config"], "merged")
        cfg = self.home / ".config"
        self.assertEqual((cfg / "tessera/config.json").read_text(), '{"new": true}')
        self.assertEqual((cfg / "tessera/monitors.lua").read_text(), "-- displays\n")
        self.assertTrue((cfg / "irice").is_symlink())
        kept = cfg / "irice.before-tessera/config.json"
        self.assertEqual(kept.read_text(), '{"appearance": {"menuIcon": "irice"}}')
        note = (cfg / "tessera" / MIG.NOTE).read_text()
        self.assertIn("monitors.lua", note)
        self.assertIn("irice.before-tessera", note)
        # nothing in both: the old folder simply goes
        self.write(self.home / ".local/state/tessera/other.json", "{}")
        MIG.migrate_one(self.xdg["XDG_STATE_HOME"])  # (already merged above: linked)
        st = Path(self.xdg["XDG_STATE_HOME"])
        shutil.rmtree(st)
        self.write(st / "tessera/a", "new")
        self.write(st / "irice/b", "old")
        self.assertEqual(MIG.migrate_one(str(st)), "merged")
        self.assertEqual(sorted(p.name for p in st.iterdir()), ["irice", "tessera"])
        self.assertEqual((st / "irice/b").read_text(), "old")

    def test_nothing_to_move(self):
        self.assertEqual(MIG.migrate(self.xdg), {"config": "none", "data": "none", "state": "none", "cache": "none"})
        self.assertFalse((self.home / ".config").exists())

    def test_dry_run_changes_nothing(self):
        self.old_layout()
        self.assertEqual(MIG.migrate(self.xdg, dry=True)["config"], "would move")
        self.assertFalse((self.home / ".config/tessera").exists())

    def test_which_folder_to_use_before_and_after(self):
        base = self.home / ".config"
        self.assertEqual(MIG.resolve(str(base)), str(base / "tessera"))  # (a new setup)
        self.old_layout()
        self.assertEqual(MIG.resolve(str(base)), str(base / "irice"))  # (not moved yet)
        MIG.migrate(self.xdg)
        self.assertEqual(MIG.resolve(str(base)), str(base / "tessera"))

    def test_never_stops_a_shell_from_starting(self):
        self.old_layout()
        os.chmod(self.home / ".config", 0o500)  # (can't rename in there)
        try:
            p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "migrate-names.py")], env=dict(os.environ, **self.xdg),
                               capture_output=True, text=True, timeout=20)
        finally:
            os.chmod(self.home / ".config", 0o700)
        self.assertEqual(p.returncode, 0)
        self.assertIn("config: error", p.stderr)
        self.assertTrue((self.home / ".config/irice/config.json").exists())


# The shell itself, under Quickshell: which folder it reads
PATHS_PROBE = '''import QtQuick
import Quickshell
import qs.common
ShellRoot { settings.watchFiles: false
Timer { interval: 50; running: true; onTriggered: { console.log("PATHS " + Paths.name + " " + Paths.config + " " + Paths.state); Qt.quit(); } }
}
'''


@unittest.skipUnless(QS.exists(), "requires bundled Quickshell")
class PathsQml(Temp):
    def paths(self):
        shell = self.d / "shell"
        (shell / "common").mkdir(parents=True, exist_ok=True)
        shutil.copy2(SHELL / "common/Paths.qml", shell / "common/Paths.qml")
        (shell / "shell.qml").write_text(PATHS_PROBE)
        rt = self.d / "rt"
        rt.mkdir(mode=0o700, exist_ok=True)
        env = dict(os.environ, **self.xdg, XDG_RUNTIME_DIR=str(rt), QT_QPA_PLATFORM="offscreen", QML_IMPORT_PATH=str(BUNDLE / "lib/qt6/qml"))
        p = subprocess.run([str(QS), "-p", str(shell)], env=env, capture_output=True, text=True, timeout=20)
        m = re.search(r"PATHS (\S+) (\S+) (\S+)", p.stdout + p.stderr)
        self.assertIsNotNone(m, p.stdout + p.stderr)
        return m.groups()

    def test_a_new_setup_uses_tesseras(self):
        self.assertEqual(self.paths(), ("tessera", f"{self.home}/.config/tessera", f"{self.home}/.local/state/tessera"))

    def test_a_shell_reloaded_before_the_move_keeps_the_old_folders(self):
        self.write(self.home / ".config/irice/config.json", "{}")
        self.assertEqual(self.paths(), ("irice", f"{self.home}/.config/irice", f"{self.home}/.local/state/irice"))

    def test_after_the_move(self):
        self.write(self.home / ".config/irice/config.json", "{}")
        MIG.migrate(self.xdg)
        self.assertEqual(self.paths()[0], "tessera")


# ── launch-shell.sh: moves the folders, passes the old names on ─────────
FAKE_QS = r"""#!/bin/sh
if [ "$3" = ipc ] && [ "$4" = show ]; then echo "No running instances"; exit 0; fi
printf '%s\n' "$@" > "$QS_CALL"
basename "$0" > "$QS_CALL.name"
env > "$QS_CALL.env"
"""


class Launch(Temp):
    def setUp(self):
        super().setUp()
        self.write(self.bin / "qs", FAKE_QS, 0o755)
        self.call = self.d / "call"
        self.sys = self.d / "sys"
        self.sys.mkdir()
        self.env = dict(os.environ, **self.xdg, XDG_RUNTIME_DIR=str(self.d), QS_CALL=str(self.call), PATH=f"{self.bin}:/usr/bin:/bin",
                        QS_ICON_THEME="hicolor", TESSERA_SYSTEM_ROOT=str(self.sys))
        for k in ("HYPRLAND_INSTANCE_SIGNATURE", "TESSERA_TTY", "TESSERA_ROOT", "IRICE_TTY", "IRICE_ROOT", "TESSERA_SHELL", "IRICE_SHELL"):
            self.env.pop(k, None)

    def launch(self, *args, **env):
        subprocess.run(["bash", str(SCRIPTS / "launch-shell.sh"), *(args or ("-p", str(SHELL)))], env=dict(self.env, **env), timeout=30,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return self.call.read_text().split("\n")[:2]

    def name(self):
        return Path(str(self.call) + ".name").read_text().strip()

    def started_env(self):
        return dict(l.split("=", 1) for l in Path(str(self.call) + ".env").read_text().splitlines() if "=" in l)

    def test_the_start_moves_your_folders(self):
        self.write(self.home / ".config/irice/config.json", json.dumps({"appearance": {"look": "glass"}}))
        self.write(self.home / ".local/state/irice/state.json", "{}")
        self.launch()
        self.assertTrue((self.home / ".config/tessera/config.json").exists())
        self.assertTrue((self.home / ".config/irice").is_symlink())
        self.assertTrue((self.home / ".local/state/tessera/state.json").exists())

    def test_a_session_from_before_the_rename_passes_its_names_on(self):
        e = (self.launch("-p", str(SHELL), IRICE_TTY="1", IRICE_ROOT="/somewhere"), self.started_env())[1]
        self.assertEqual((e.get("TESSERA_TTY"), e.get("TESSERA_ROOT")), ("1", "/somewhere"))
        # (and the old markers, for a watchdog or update screen from before)
        self.assertEqual((e.get("TESSERA_SHELL"), e.get("IRICE_SHELL"), e.get("IRICE_TTY")), ("1", "1", "1"))
        e = (self.launch("-p", str(SHELL), TESSERA_TTY="1", IRICE_TTY="0"), self.started_env())[1]
        self.assertEqual(e.get("TESSERA_TTY"), "1")  # (the new name wins)

    def test_the_name_follows_the_priority_rule_installed(self):
        self.launch()
        self.assertEqual(self.name(), "tessera-shell")  # (no ananicy: the new name)
        rules = self.sys / "etc/ananicy.d"
        self.write(rules / "irice.rules", '{ "name": "irice-shell", "type": "LowLatency_RT" }\n')
        self.launch()
        self.assertEqual(self.name(), "irice-shell")  # (until the root setup installs Tessera's rule)
        self.write(rules / "tessera.rules", R.ANANICY_TEXT.decode())
        self.launch()
        self.assertEqual(self.name(), "tessera-shell")

    def name_while(self, script, args, text, env=None):
        f = self.d / script
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(text + "\nsleep 30\n")
        w = subprocess.Popen(["bash", str(f), *args], env=dict(self.env, **(env or {})))
        try:
            time.sleep(0.3)
            self.launch()
            return self.name()
        finally:
            w.kill()
            w.wait()

    def test_watchers_from_before_the_rename_keep_the_name_they_know(self):
        # an update running iRice's copy of its script, which knows irice-shell
        self.assertEqual(self.name_while("irice-update.ab12/run.sh", ["x"], "pgrep -x irice-shell"), "irice-shell")
        self.assertEqual(self.name_while("tessera-update.cd34/run.sh", ["x"], "pgrep -x tessera-shell; pgrep -x irice-shell"), "tessera-shell")
        # a tty session's watchdog started before the rename (it noted itself in irice-watchdog.names)
        self.env["HYPRLAND_INSTANCE_SIGNATURE"] = "sig1"
        names = self.d / "hypr/sig1"
        names.mkdir(parents=True)
        self.assertEqual(self.name_while("old/tty-shell.sh", ["watchdog"], f'echo $$ > "{names}/irice-watchdog.names"'), "irice-shell")
        self.assertEqual(self.name_while("new/tty-shell.sh", ["watchdog"], f'echo $$ > "{names}/tessera-watchdog.names"'), "tessera-shell")

    def test_an_installed_shell_started_by_its_old_name(self):
        q = self.home / ".config/quickshell"
        self.write(q / "irice/shell.qml", "")
        self.assertEqual(self.launch("-c", "irice"), ["-c", "irice"])  # (only the old one there)
        self.write(q / "tessera/shell.qml", "")
        self.assertEqual(self.launch("-c", "irice"), ["-c", "tessera"])
        self.assertEqual(self.launch("-c", "tessera"), ["-c", "tessera"])


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
        # RootHelper.qml's command, as the shell builds it, run with /usr in a
        # temporary folder and a pkexec that says what it was asked
        shell = self.d / "shell"
        (shell / "services").mkdir(parents=True)
        shutil.copy2(SHELL / "services/RootHelper.qml", shell / "services/RootHelper.qml")
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
        self.assertEqual(cmd[:2], ["sh", "-c"])
        # neither installed: Tessera's (pkexec then says it's missing)
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/tessera/tessera-root priority on"))
        self.assertNotEqual(run(knows)[0], 0)
        # only iRice's: that one, with its own policy
        self.system("usr/lib/irice/irice-root")
        self.write(sysroot / "usr/share/polkit-1/actions/org.irice.root.policy", '<action id="org.irice.root.login-look">')
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/irice/irice-root priority on"))
        self.assertEqual(run(knows)[0], 0)
        # Tessera's installed: Tessera's
        self.system("usr/lib/tessera/tessera-root")
        self.assertEqual(run(cmd[2], *cmd[4:]), (0, "/usr/lib/tessera/tessera-root priority on"))
        self.assertNotEqual(run(knows)[0], 0)  # (its policy isn't there yet in this folder)
        self.write(sysroot / "usr/share/polkit-1/actions/org.tessera.root.policy", (SHELL / "assets/polkit/org.tessera.root.policy").read_text())
        self.assertEqual(run(knows)[0], 0)


class RootSetup(Temp):
    """setup, into a folder that has what iRice's helper installed."""

    def irice_system(self):
        s = self.d / "sys"
        w = lambda p, t="", m=None: self.write(s / p, t, m)
        w("usr/lib/irice/irice-root", "#!/usr/bin/python3\n", 0o755)
        w("usr/lib/irice/qs-wifi-helper.py", "")
        w("usr/lib/irice/plymouth/irice-breathe/irice-breathe.plymouth", "")
        w("usr/share/polkit-1/actions/org.irice.root.policy", "<policyconfig/>")
        w("etc/polkit-1/rules.d/49-irice.rules", "// iRice\n")
        w("etc/polkit-1/rules.d/49-irice-quickshare.rules", 'if (subject.user === "me" && subject.active && subject.local)\n')
        w("etc/irice/quickshare-trusted", "me\n")
        w("etc/irice/bootsplash.json", json.dumps({"original": "spinner", "applied": "irice-breathe"}))
        w("var/lib/irice/initramfs-backup/0.img", "x")
        w("etc/ananicy.d/irice.rules", '{ "name": "irice-shell", "type": "LowLatency_RT" }\n')
        w("etc/systemd/system/bluetooth.service.d/irice-no-rap.conf", "# iRice: no Ranging\n[Service]\n")
        w("etc/systemd/system/fprintd.service.d/irice-timeout.conf", "# iRice: firmware\n")
        w("etc/systemd/system/plymouth-quit.service.d/irice.conf", "# iRice: the splash stays (irice uninstall removes this)\n")
        w("etc/modprobe.d/irice-phonecam.conf", "options v4l2loopback\n")
        w("etc/modules-load.d/irice-phonecam.conf", "v4l2loopback\n")
        w("etc/pam.d/sudo", "#%PAM-1.0\nauth       sufficient   pam_fprintd.so max-tries=5 timeout=10   # iRice fingerprint\nauth include system-auth\n")
        w("etc/plymouth/plymouthd.conf", "[Daemon]\nTheme=irice-breathe\nShowDelay=0\n")
        for id in ("breathe", "orbit"):
            w(f"usr/share/plymouth/themes/irice-{id}/irice-{id}.plymouth", "")
        return s

    def test_setup_moves_everything_and_removes_the_old(self):
        s = self.irice_system()
        written = R.setup(str(SHELL), os.getuid(), prefix=str(s))
        e = lambda p: (s / p).exists() or (s / p).is_symlink()
        # Tessera's helper, policy and rule are in; iRice's are gone
        for p in ("usr/lib/tessera/tessera-root", "usr/share/polkit-1/actions/org.tessera.root.policy", "etc/polkit-1/rules.d/49-tessera.rules"):
            self.assertTrue(e(p), p)
        for p in ("usr/lib/irice", "usr/share/polkit-1/actions/org.irice.root.policy", "etc/polkit-1/rules.d/49-irice.rules",
                  "etc/polkit-1/rules.d/49-irice-quickshare.rules", "etc/ananicy.d/irice.rules", "etc/irice", "var/lib/irice"):
            self.assertFalse(e(p), p)
        # the shell's priority, for its new name and its old one (a shell started before keeps it)
        rule = (s / "etc/ananicy.d/tessera.rules").read_text()
        self.assertIn('"tessera-shell"', rule)
        self.assertIn('"irice-shell"', rule)
        # Quick Share's trust, for the same person, now for Tessera's helper
        trust = (s / "etc/polkit-1/rules.d/49-tessera-quickshare.rules").read_text()
        self.assertIn('subject.user === "me"', trust)
        self.assertIn("/usr/lib/tessera/tessera-root", trust)
        self.assertNotIn("irice", trust)
        self.assertEqual((s / "etc/tessera/quickshare-trusted").read_text(), "me\n")
        self.assertTrue(e("var/lib/tessera/initramfs-backup/0.img"))
        # drop-ins and module options renamed, content kept
        self.assertEqual((s / "etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf").read_text(), "# Tessera: no Ranging\n[Service]\n")
        self.assertIn("tessera uninstall", (s / "etc/systemd/system/plymouth-quit.service.d/tessera.conf").read_text())
        for p in ("etc/systemd/system/bluetooth.service.d/irice-no-rap.conf", "etc/systemd/system/fprintd.service.d/irice-timeout.conf",
                  "etc/systemd/system/plymouth-quit.service.d/irice.conf", "etc/modprobe.d/irice-phonecam.conf", "etc/modules-load.d/irice-phonecam.conf"):
            self.assertFalse(e(p), p)
        self.assertTrue(e("etc/modprobe.d/tessera-phonecam.conf"))
        # the fingerprint line is Tessera's now (and still turned off by it)
        pam = (s / "etc/pam.d/sudo").read_text()
        self.assertIn("# Tessera fingerprint", pam)
        self.assertNotIn("iRice", pam)
        # the boot animation in use, under its new name; iRice's copies gone
        self.assertIn("Theme=tessera-breathe\n", (s / "etc/plymouth/plymouthd.conf").read_text())
        self.assertIn("ShowDelay=0", (s / "etc/plymouth/plymouthd.conf").read_text())
        self.assertTrue(e("usr/share/plymouth/themes/tessera-breathe/tessera-breathe.plymouth"))
        self.assertFalse(e("usr/share/plymouth/themes/irice-breathe"))
        self.assertFalse(e("usr/share/plymouth/themes/irice-orbit"))
        self.assertEqual(json.loads((s / "etc/tessera/bootsplash.json").read_text()), {"original": "spinner", "applied": "tessera-breathe"})
        self.assertIn("/usr/lib/irice", written)
        # once: nothing more to do the next time
        self.assertEqual(R.setup(str(SHELL), os.getuid(), prefix=str(s)), [])

    def test_priority_turned_off_stays_off(self):
        s = self.d / "sys"
        self.write(s / "etc/ananicy.d/irice.rules", "x\n")
        self.write(s / "etc/irice/priority-off", "off\n")
        R.setup(str(SHELL), os.getuid(), prefix=str(s))
        self.assertTrue((s / "etc/tessera/priority-off").exists())
        self.assertFalse((s / "etc/ananicy.d/tessera.rules").exists())
        self.assertFalse((s / "etc/ananicy.d/irice.rules").exists())

    def test_priority_on_and_off_take_the_old_rule_out(self):
        s = self.d / "sys"
        self.write(s / "etc/ananicy.d/irice.rules", "x\n")
        self.assertIn(str(s / "etc/ananicy.d/irice.rules"), R.priority(True, prefix=str(s), run=lambda *a, **k: None))
        self.assertTrue((s / "etc/ananicy.d/tessera.rules").exists())
        self.write(s / "etc/ananicy.d/irice.rules", "x\n")
        R.priority(False, prefix=str(s), run=lambda *a, **k: None)
        self.assertEqual(sorted(p.name for p in (s / "etc/ananicy.d").iterdir()), [])

    def test_trust_without_its_marker_reads_the_rule(self):
        s = self.d / "sys"
        self.write(s / "etc/polkit-1/rules.d/49-irice-quickshare.rules", 'if (subject.user === "ana" && subject.active)\n')
        R.migrate_from_irice(str(s))
        self.assertIn('subject.user === "ana"', (s / "etc/polkit-1/rules.d/49-tessera-quickshare.rules").read_text())

    def test_the_old_login_screen_is_written_until_its_replaced(self):
        s = self.d / "sys"
        self.write(s / "usr/share/irice/shell/shell.qml", "old")
        self.assertEqual(R.login_share(str(s)), str(s / "usr/share/irice"))
        # (setup leaves that copy as it was installed: install.sh --only=login replaces it)
        R.setup(str(SHELL), os.getuid(), prefix=str(s))
        self.assertEqual((s / "usr/share/irice/shell/shell.qml").read_text(), "old")
        self.write(s / "usr/share/tessera/shell/shell.qml", "")
        self.assertEqual(R.login_share(str(s)), str(s / "usr/share/tessera"))

    def test_fingerprint_off_takes_out_old_marks(self):
        etc = self.d / "etc"
        self.write(etc / "sudo", "#%PAM-1.0\nauth sufficient pam_fprintd.so   # iRice fingerprint\nauth include system-auth\n")
        R.fingerprint(False, "sudo", etc=str(etc), vendor=str(self.d / "vendor"), module=())
        self.assertEqual((etc / "sudo").read_text(), "#%PAM-1.0\nauth include system-auth\n")

    def test_bootsplash_counts_an_old_animation_as_tesseras(self):
        # (the system's own theme is never one of ours, by either name)
        self.assertTrue("irice-breathe".startswith(("tessera-", "irice-")))
        src = (SCRIPTS / "tessera-root.py").read_text()
        self.assertIn('original.startswith(("tessera-", "irice-"))', src)
        self.assertIn("read_text(at(OLD_BOOTSPLASH_RECORD))", src)


# ── Environment names from before the rename ────────────────────────────
class EnvFallbacks(Temp):
    def test_autostart_reads_the_old_settings_folder_name(self):
        cfg = self.d / "cfg"
        self.write(cfg / "config.json", json.dumps({"loginItems": {"startAtLogin": False}}))
        p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "xdg-autostart.py"), "--dry-run"],
                           env=dict(os.environ, **self.xdg, IRICE_USER_DIR=str(cfg), XDG_RUNTIME_DIR=str(self.d)),
                           capture_output=True, text=True, timeout=20)
        self.assertFalse(json.loads(p.stdout)["startAtLogin"], p.stderr)

    def test_watchers_from_before_are_still_ours(self):
        pi = load(SCRIPTS / "process-identity.py", "process_identity_rename")
        w = subprocess.Popen(["sleep", "30"], env=dict(os.environ, IRICE_WATCH_TOKEN="abc"))
        try:
            time.sleep(0.1)
            self.assertTrue(pi.carries(w.pid, "abc"))
            self.assertFalse(pi.carries(w.pid, "abd"))
        finally:
            w.kill()
            w.wait()

    def test_hypridles_actions_from_before_still_lock(self):
        idle = self.d / "idle"
        self.write(idle / "lock-command", f"touch {self.d}/locked")
        subprocess.run(["bash", str(SCRIPTS / "idle-action.sh"), "before-sleep"],
                       env=dict(os.environ, IRICE_IDLE_DIR=str(idle)), timeout=20, capture_output=True)
        self.assertTrue((self.d / "locked").exists())

    def test_the_glass_plugin_loads_whichever_build_is_there(self):
        # glass.lua with a stand-in for Hyprland's hl
        data = self.d / "data"

        def picked(*files, plugin=None):
            shutil.rmtree(data, ignore_errors=True)
            for f in files:
                self.write(data / f, "so")
            pre = f"TESSERA_GLASS_PLUGIN = {plugin}\n" if plugin else ""
            lua = pre + 'hl = { on = function(_, f) f() end, exec_cmd = function(c) print(c) end }\n' + (ROOT / "hypr/tessera/glass.lua").read_text()
            p = subprocess.run(["lua", "-"], input=lua, env=dict(os.environ, XDG_DATA_HOME=str(data)), capture_output=True, text=True, timeout=10)
            self.assertEqual(p.returncode, 0, p.stderr)
            return p.stdout.strip().replace(str(data), "$DATA")
        if not shutil.which("lua"):
            self.skipTest("needs lua")
        self.assertEqual(picked(), "")
        self.assertEqual(picked("irice/irice-glass.so"), "hyprctl plugin load '$DATA/irice/irice-glass.so'")
        self.assertEqual(picked("tessera/irice-glass.so"), "hyprctl plugin load '$DATA/tessera/irice-glass.so'")
        self.assertEqual(picked("tessera/irice-glass.so", "tessera/tessera-glass.so"), "hyprctl plugin load '$DATA/tessera/tessera-glass.so'")
        # the tty session: the repo's own build first, the one built there before the rename next
        repo = f'{{ "{data}/repo/tessera-glass.so", "{data}/repo/irice-glass.so" }}'
        self.assertEqual(picked("repo/irice-glass.so", "tessera/tessera-glass.so", plugin=repo), "hyprctl plugin load '$DATA/repo/irice-glass.so'")


# ── The installer and uninstaller ───────────────────────────────────────
class Installer(Temp):
    def setUp(self):
        super().setUp()
        self.fake("sudo", 'exec "$@"\n')
        self.sys = self.d / "sys"
        self.sys.mkdir()

    def bash(self, names, code):
        pattern = "; ".join(f"/^{n}() {{/,/^}}/p" for n in names)
        functions = subprocess.run(["sed", "-n", pattern, str(ROOT / "install.sh")], capture_output=True, text=True, check=True).stdout
        env = dict(os.environ, **self.xdg, PATH=f"{self.bin}:/usr/bin:/bin", TESSERA_SYSTEM_ROOT=str(self.sys))
        script = (f'ROOT={ROOT}; CONFIG="$XDG_CONFIG_HOME"; DATA="$XDG_DATA_HOME"; DRY=0\n'
                  'note() { echo "note: $*"; }\n' + functions + "\n" + code)
        p = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout

    def test_an_install_from_before_moves_over(self):
        c = self.home / ".config"
        self.write(c / "irice/config.json", "{}")
        self.write(self.home / ".local/share/irice/source", "/repo\n")
        self.write(c / "hypr/irice/keyboard.lua", "-- yours\n")
        self.write(c / "hypr.pre-irice/hyprland.conf", "# your own\n")
        self.write(c / "fish/conf.d/irice.fish", "fish_add_path\n")
        self.write(self.sys / "etc/greetd/config.toml.pre-irice", "# before\n")
        self.write(self.sys / "etc/pam.d/greetd.pre-irice", "# pam before\n")
        self.bash(["adopt_old_names"], "adopt_old_names")
        self.assertTrue((c / "tessera/config.json").exists())
        self.assertTrue((self.home / ".local/share/tessera/source").exists())
        self.assertEqual((c / "hypr/tessera/keyboard.lua").read_text(), "-- yours\n")
        self.assertTrue((c / "hypr/irice").is_symlink())
        self.assertEqual((c / "hypr.pre-tessera/hyprland.conf").read_text(), "# your own\n")
        self.assertTrue((c / "fish/conf.d/tessera.fish").exists())
        self.assertEqual((self.sys / "etc/greetd/config.toml.pre-tessera").read_text(), "# before\n")
        self.assertTrue((self.sys / "etc/pam.d/greetd.pre-tessera").exists())
        self.assertFalse((self.sys / "etc/greetd/config.toml.pre-irice").exists())
        # (a backup of Tessera's own already there wins: the other stays where it is)
        self.write(self.sys / "etc/default/grub.pre-irice", "a")
        self.write(self.sys / "etc/default/grub.pre-tessera", "b")
        self.bash(["adopt_old_names"], "adopt_old_names")
        self.assertEqual((self.sys / "etc/default/grub.pre-tessera").read_text(), "b")

    def test_the_login_screens_entry_starts_the_same_folder(self):
        repo = self.d / "repo"
        self.write(repo / "sandbox/tessera-session.sh", "#!/bin/sh\n", 0o755)
        sessions = self.sys / "usr/share/wayland-sessions"
        self.write(sessions / "irice.desktop", f"[Desktop Entry]\nName=iRice\nExec={repo}/sandbox/irice-session.sh\nType=Application\n")
        self.bash(["login_session_entry"], "login_session_entry")
        self.assertFalse((sessions / "irice.desktop").exists())
        entry = (sessions / "tessera.desktop").read_text()
        self.assertIn(f"Exec={repo}/sandbox/tessera-session.sh\n", entry)
        self.assertIn("Name=Tessera\n", entry)

    def test_the_old_entry_still_starts_tessera(self):
        # (until that entry is replaced, it starts sandbox/irice-session.sh)
        box = self.d / "box/sandbox"
        box.mkdir(parents=True)
        shutil.copy2(ROOT / "sandbox/irice-session.sh", box / "irice-session.sh")
        self.write(box / "tessera-session.sh", f'#!/bin/sh\necho "tessera $*" > {self.d}/ran\n', 0o755)
        subprocess.run([str(box / "irice-session.sh"), "a"], timeout=10, check=True)
        self.assertEqual((self.d / "ran").read_text(), "tessera a\n")


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
        env = dict(os.environ, **self.xdg, PATH=f"{self.bin}:/usr/bin:/bin", TESSERA_SYSTEM_ROOT=str(s))
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
        self.assertEqual((c / "hypr/hyprland.lua").read_text(), "-- mine\n")  # (iRice's backup of yours)
        self.assertEqual((s / "etc/greetd/config.toml").read_text(), "# before\n")
        self.assertEqual((s / "etc/pam.d/greetd").read_text(), "auth include x\n")
        self.assertEqual((h / ".bash_profile").read_text(), "a\nb\n")
        self.assertIn("systemctl enable sddm", log.read_text())


if __name__ == "__main__":
    unittest.main()
