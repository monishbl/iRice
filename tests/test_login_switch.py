"""Switching to Tessera's login screen from another one (tessera-root.py
login-switch, login-fallback, login-check), in a folder standing in for the
system: a fake systemctl keeps display-manager.service's link as the real
one would, fake pacman, loginctl and systemd-detect-virt stand in. Nothing
real is asked or changed, and no login manager is touched.
"""
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "shell/assets/scripts/tessera-root.py"
LOOK = {"style": "text", "look": "glass", "mode": "dark", "seed": "#6750A4", "materialStyle": "tonalSpot",
        "clockFace": "dynamic", "clockWeight": 640, "clockWidth": 112, "clockRound": True}
GREETD_PAM = "#%PAM-1.0\n\nauth       include      system-local-login\naccount    include      system-local-login\n" \
             "session    include      system-local-login\n"


def load():
    spec = importlib.util.spec_from_file_location("tessera_root_login", SOURCE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class System:
    """The commands the helper runs, against the fake root"""

    def __init__(self, root):
        self.root = root
        self.calls = []
        self.refuse_enable = set()  # units whose enable fails
        self.greetd_show = "ActiveState=activating\nActiveEnterTimestampMonotonic=0\n"
        self.sessions = []  # (id, service, class)
        self.pacman_ok = True

    def link(self):
        return self.root / "etc/systemd/system/display-manager.service"

    def __call__(self, cmd, **kw):
        self.calls.append(cmd)
        out, code = "", 0
        if cmd[0] == "systemctl":
            verb, rest = cmd[1], [c for c in cmd[2:] if not c.startswith("--")]
            unit = rest[0] if rest else ""
            name = unit[:-len(".service")] if unit.endswith(".service") else unit
            dm = self.link()
            if verb == "disable" and unit.endswith(".service"):
                if dm.is_symlink() and os.path.basename(os.readlink(dm)) == unit:
                    dm.unlink()
            elif verb == "enable" and unit.endswith(".service"):
                if name in self.refuse_enable:
                    return subprocess.CompletedProcess(cmd, 1, "", f"Failed to enable unit: {unit} is masked.\n")
                if dm.is_symlink():
                    return subprocess.CompletedProcess(cmd, 1, "", f"Failed to enable unit: File {dm} already exists.\n")
                dm.parent.mkdir(parents=True, exist_ok=True)
                dm.symlink_to(f"/usr/lib/systemd/system/{unit}")
            elif verb == "show":
                out = self.greetd_show
        elif cmd[0].endswith("pacman"):
            if not self.pacman_ok:
                return subprocess.CompletedProcess(cmd, 1, "", "error: target not found: greetd\n")
            (self.root / "usr/bin").mkdir(parents=True, exist_ok=True)
            (self.root / "usr/bin/greetd").write_text("")
        elif cmd[0] == "systemd-detect-virt":
            out, code = "none\n", 1
        elif cmd[:2] == ["loginctl", "list-sessions"]:
            out = "".join(f"{s[0]} 1000 me seat0 tty1\n" for s in self.sessions)
        elif cmd[:2] == ["loginctl", "show-session"]:
            s = next(x for x in self.sessions if x[0] == cmd[2])
            out = f"Service={s[1]}\nClass={s[2]}\n"
        return subprocess.CompletedProcess(cmd, code, out, "")

    def ran(self, *cmd):
        return list(cmd) in self.calls


class LoginSwitch(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.root = self.d / "root"
        self.r = load()
        self.home = self.d / "home"
        (self.home / ".config/tessera").mkdir(parents=True)
        self.r.caller_home = lambda uid: str(self.home)
        self.sys = System(self.root)
        # a checkout: the shell, Material, the greeter's files
        self.repo = self.d / "repo"
        for name in ("shell", "material"):
            (self.repo / name).mkdir(parents=True)
            (self.repo / name / "shell.qml").write_text(f"// {name}\n")
        (self.repo / "shell/assets/wallpapers").mkdir(parents=True)
        (self.repo / "shell/assets/wallpapers/tahoe-dusk.jpg").write_bytes(b"\xff\xd8\xff default")
        (self.repo / "hypr").mkdir()
        (self.repo / "hypr/greeter.lua").write_text("-- greeter\n")
        (self.repo / "hypr/greeter-session.sh").write_text("#!/bin/sh\n")
        # the system: SDDM on, its unit installed, the helper installed, this boot
        self.put("usr/lib/systemd/system/sddm.service", "[Unit]\n")
        self.put("usr/lib/systemd/system/greetd.service", "[Unit]\n")
        self.put("usr/lib/tessera/tessera-root", "")
        self.put("etc/pam.d/greetd", GREETD_PAM)
        self.put("usr/lib/security/pam_gnome_keyring.so", "")
        self.put("etc/greetd/config.toml", '[default_session]\ncommand = "agreety --cmd /bin/sh"\n')
        self.boot("boot-1")
        self.sys.link().parent.mkdir(parents=True, exist_ok=True)
        self.sys.link().symlink_to("/usr/lib/systemd/system/sddm.service")

    def tearDown(self):
        self.tmp.cleanup()

    def put(self, rel, text):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def boot(self, bid, up=30):
        self.put("proc/sys/kernel/random/boot_id", bid + "\n")
        self.put("proc/uptime", f"{up}.00 10.00\n")

    def dm(self):
        return self.r.current_dm(str(self.root))

    def record(self):
        return self.r.read_record(str(self.root))

    def switch(self, look=None, which=lambda n: None):
        return self.r.login_switch_on(str(self.repo / "shell"), json.dumps(look or LOOK), os.getuid(),
                                      prefix=str(self.root), run=self.sys, which=which)

    def test_switches_from_sddm_and_keeps_the_way_back(self):
        rec = self.switch()
        self.assertEqual(self.dm(), "greetd")
        self.assertEqual((rec["previous"], rec["state"], rec["boot"]), ("sddm", "trial", "boot-1"))
        self.assertEqual(self.record()["previous"], "sddm")
        # SDDM off before greetd on (never both), the check armed for the restart
        i, j = self.sys.calls.index(["systemctl", "disable", "sddm.service"]), self.sys.calls.index(["systemctl", "enable", "greetd.service"])
        self.assertLess(i, j)
        self.assertTrue(self.sys.ran("systemctl", "enable", "tessera-login-check.timer"))
        # greetd installed (it wasn't), the login screen's copy with the look
        self.assertTrue(self.sys.ran(self.r.PACMAN, "-S", "--needed", "--noconfirm", "greetd"))
        share = self.root / "usr/share/tessera"
        self.assertEqual((share / "shell/shell.qml").read_text(), "// shell\n")
        self.assertEqual((share / "material/shell.qml").read_text(), "// material\n")
        self.assertEqual(json.loads((share / "greeter.json").read_text())["style"], "text")
        self.assertEqual((share / "wallpaper.jpg").read_bytes(), b"\xff\xd8\xff default")
        self.assertEqual((share / "greeter-session.sh").stat().st_mode & 0o777, 0o755)
        self.assertTrue((self.root / "var/lib/tessera/greeter").is_dir())
        # greetd's config (the one before kept), the keyring, the fallback units
        conf = (self.root / "etc/greetd/config.toml").read_text()
        self.assertIn('command = "/usr/share/tessera/greeter-session.sh"', conf)
        self.assertIn("agreety", (self.root / "etc/greetd/config.toml.pre-tessera").read_text())
        self.assertIn("pam_gnome_keyring.so auto_start", (self.root / "etc/pam.d/greetd").read_text())
        self.assertEqual((self.root / "etc/pam.d/greetd.pre-tessera").read_text(), GREETD_PAM)
        dropin = (self.root / "etc/systemd/system/greetd.service.d/tessera-fallback.conf").read_text()
        self.assertIn("OnFailure=tessera-login-fallback.service", dropin)
        self.assertIn("ExecStart=/usr/lib/tessera/tessera-root login-fallback",
                      (self.root / "etc/systemd/system/tessera-login-fallback.service").read_text())
        self.assertIn("OnBootSec=", (self.root / "etc/systemd/system/tessera-login-check.timer").read_text())

    def test_your_wallpaper_when_it_can_be_read(self):
        wall = self.home / "w.png"
        wall.write_bytes(b"\x89PNG\r\n\x1a\n mine")
        (self.home / ".config/tessera/config.json").write_text(json.dumps({"wallpaper": {"path": str(wall)}}))
        self.switch()
        self.assertEqual((self.root / "usr/share/tessera/wallpaper.jpg").read_bytes(), b"\x89PNG\r\n\x1a\n mine")

    def test_greetd_that_cant_be_turned_on_leaves_sddm_on(self):
        self.sys.refuse_enable.add("greetd")
        with self.assertRaises(self.r.Refused) as e:
            self.switch()
        self.assertIn("couldn't turn on greetd", str(e.exception))
        self.assertEqual(self.dm(), "sddm")  # never neither
        self.assertEqual(self.record()["state"], "off")
        self.assertFalse(self.sys.ran("systemctl", "enable", "tessera-login-check.timer"))

    def test_nothing_switches_when_greetd_cant_be_installed(self):
        self.sys.pacman_ok = False
        with self.assertRaises(self.r.Refused) as e:
            self.switch()
        self.assertIn("target not found", str(e.exception))
        self.assertEqual(self.dm(), "sddm")
        self.assertEqual(self.record(), {})

    def test_refuses_odd_looks_and_folders(self):
        with self.assertRaises(self.r.Refused):
            self.switch(look=dict(LOOK, style="rm -rf /"))
        with self.assertRaises(self.r.Refused):
            self.r.login_switch_on(str(self.d / "nowhere"), json.dumps(LOOK), os.getuid(), prefix=str(self.root), run=self.sys, which=lambda n: None)
        (self.repo / "hypr/greeter-session.sh").unlink()
        with self.assertRaises(self.r.Refused):
            self.switch()
        self.assertEqual(self.dm(), "sddm")

    def test_from_the_console(self):
        self.sys.link().unlink()
        rec = self.switch()
        self.assertEqual((rec["previous"], self.dm()), ("", "greetd"))
        self.r.login_switch_off(prefix=str(self.root), run=self.sys)
        self.assertEqual(self.dm(), "")

    def test_again_keeps_the_record(self):
        self.switch()
        self.boot("boot-2")
        rec = self.switch(look=dict(LOOK, style="plain"))
        self.assertEqual((rec["previous"], rec["state"]), ("sddm", "trial"))
        self.assertEqual(json.loads((self.root / "usr/share/tessera/greeter.json").read_text())["style"], "plain")
        self.assertEqual(self.dm(), "greetd")

    def test_go_back_by_choice(self):
        self.switch()
        rec = self.r.login_switch_off(prefix=str(self.root), run=self.sys)
        self.assertEqual((self.dm(), rec["state"]), ("sddm", "off"))
        self.assertTrue(self.sys.ran("systemctl", "disable", "tessera-login-check.timer"))
        # (from the next start: nothing stopped or started now)
        self.assertFalse(any(c[:2] == ["systemctl", "start"] for c in self.sys.calls))

    def test_go_back_without_a_record_takes_install_shs_note(self):
        (self.root / "etc/greetd/config.toml").write_text('command = "/usr/share/tessera/greeter-session.sh"\n')
        self.sys.link().unlink()
        self.sys.link().symlink_to("/usr/lib/systemd/system/greetd.service")
        with self.assertRaises(self.r.Refused):
            self.r.login_switch_off(prefix=str(self.root), run=self.sys)
        for bad in ("../../bin/sh", "nosuchdm", "a b"):
            with self.assertRaises(self.r.Refused):
                self.r.login_switch_off(bad, prefix=str(self.root), run=self.sys)
        self.assertEqual(self.dm(), "greetd")
        self.r.login_switch_off("sddm", prefix=str(self.root), run=self.sys)
        self.assertEqual(self.dm(), "sddm")

    def test_greetd_with_another_greeter_gets_its_config_back(self):
        self.sys.link().unlink()
        self.sys.link().symlink_to("/usr/lib/systemd/system/greetd.service")
        rec = self.switch()
        self.assertEqual(rec["previous"], "greetd")
        self.r.login_switch_off(prefix=str(self.root), run=self.sys)
        self.assertEqual(self.dm(), "greetd")
        self.assertIn("agreety", (self.root / "etc/greetd/config.toml").read_text())

    # ── after the restart ──
    def check(self):
        return self.r.login_check(prefix=str(self.root), run=self.sys)

    def test_check_waits_until_the_restart(self):
        self.switch()
        self.assertEqual(self.check(), "waiting")
        self.assertEqual(self.dm(), "greetd")

    def test_the_login_screen_showed(self):
        self.switch()
        self.boot("boot-2")
        self.put("var/lib/tessera/greeter/seen", "boot-2\n")
        self.assertEqual(self.check(), "confirmed")
        self.assertEqual((self.record()["state"], self.dm()), ("active", "greetd"))
        self.assertTrue(self.sys.ran("systemctl", "disable", "--now", "tessera-login-check.timer"))
        self.assertEqual(self.check(), "done")

    def test_someone_logged_in_through_it(self):
        self.switch()
        self.boot("boot-2")
        self.sys.sessions = [("c1", "greetd", "greeter"), ("2", "greetd", "user")]
        self.assertEqual(self.check(), "confirmed")

    def test_last_boots_mark_doesnt_count(self):
        self.switch()
        self.put("var/lib/tessera/greeter/seen", "boot-1\n")
        self.boot("boot-2", up=40)
        self.assertEqual(self.check(), "waiting")  # (greetd still starting)

    def test_greetd_runs_but_nothing_shows(self):
        self.switch()
        self.boot("boot-2", up=200)
        self.sys.greetd_show = "ActiveState=active\nActiveEnterTimestampMonotonic=20000000\n"
        self.assertEqual(self.check(), "reverted")
        rec = self.record()
        self.assertEqual((rec["state"], rec["previous"], rec["reason"]), ("reverted", "sddm", "the login screen didn't appear"))
        self.assertEqual(self.dm(), "sddm")
        # now, too: greetd stopped, SDDM started, without waiting on either
        self.assertTrue(self.sys.ran("systemctl", "stop", "--no-block", "greetd.service"))
        self.assertTrue(self.sys.ran("systemctl", "start", "--no-block", "sddm.service"))

    def test_greetd_just_started_gets_its_time(self):
        self.switch()
        self.boot("boot-2", up=100)
        self.sys.greetd_show = "ActiveState=active\nActiveEnterTimestampMonotonic=60000000\n"
        self.assertEqual(self.check(), "waiting")

    def test_greetd_failed(self):
        self.switch()
        self.boot("boot-2", up=50)
        self.sys.greetd_show = "ActiveState=failed\n"
        self.assertEqual(self.check(), "reverted")
        self.assertIn("greetd stopped with an error", self.record()["reason"])
        self.assertEqual(self.dm(), "sddm")

    def test_gives_up_after_ten_minutes(self):
        self.switch()
        self.boot("boot-2", up=700)
        self.assertEqual(self.check(), "reverted")
        self.assertEqual(self.dm(), "sddm")

    def test_fallback_when_greetd_fails(self):
        self.switch()
        self.boot("boot-2")
        self.assertEqual(self.r.login_fallback(prefix=str(self.root), run=self.sys), "reverted")
        self.assertEqual((self.dm(), self.record()["state"]), ("sddm", "reverted"))
        self.assertTrue(self.sys.ran("systemctl", "start", "--no-block", "sddm.service"))
        # (and only once: SDDM is on now)
        self.assertEqual(self.r.login_fallback(prefix=str(self.root), run=self.sys), "nothing")

    def test_fallback_long_after_still_brings_it_back(self):
        self.switch()
        self.boot("boot-2")
        self.put("var/lib/tessera/greeter/seen", "boot-2\n")
        self.check()
        self.boot("boot-9")
        self.assertEqual(self.r.login_fallback(prefix=str(self.root), run=self.sys), "reverted")
        self.assertEqual(self.dm(), "sddm")

    def test_fallback_leaves_a_greetd_tessera_didnt_switch_to(self):
        self.sys.link().unlink()
        self.sys.link().symlink_to("/usr/lib/systemd/system/greetd.service")
        self.assertEqual(self.r.login_fallback(prefix=str(self.root), run=self.sys), "nothing")
        self.assertEqual(self.dm(), "greetd")

    def test_fallback_back_to_the_console(self):
        self.sys.link().unlink()
        self.switch()
        self.r.login_fallback(prefix=str(self.root), run=self.sys)
        self.assertEqual(self.dm(), "")
        self.assertTrue(self.sys.ran("systemctl", "start", "--no-block", "getty@tty1.service"))

    def test_systemd_only_runs_the_checks(self):
        env = dict(os.environ, PKEXEC_UID="1000")
        p = subprocess.run(["python3", "-I", "-c", "import importlib.util,os,sys; s=importlib.util.spec_from_file_location('r', sys.argv[1]); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); os.geteuid=lambda: 0; sys.exit(m.main(['login-fallback']))", str(SOURCE)],
                           env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, self.r.REFUSED_EXIT, p.stderr)
        self.assertIn("the system's own", p.stderr)


class KeyringLines(unittest.TestCase):
    def test_after_the_last_auth_and_session_once(self):
        r = load()
        new = r.pam_with_keyring(GREETD_PAM)
        lines = new.splitlines()
        i = lines.index("auth       include      system-local-login")
        self.assertEqual(lines[i + 1:i + 3], [r.KEYRING_MARK, "-auth      optional     pam_gnome_keyring.so"])
        self.assertEqual(lines[-1], "-session   optional     pam_gnome_keyring.so auto_start")
        self.assertIsNone(r.pam_with_keyring(new))
        self.assertIsNone(r.pam_with_keyring(GREETD_PAM + "-session optional pam_kwallet5.so\n"))
        self.assertIsNone(r.pam_with_keyring("#%PAM-1.0\n"))


class Policy(unittest.TestCase):
    def test_login_switch_always_asks(self):
        policy = (ROOT / "shell/assets/polkit/org.tessera.root.policy").read_text()
        self.assertIn('<action id="org.tessera.root.login-switch">', policy)
        self.assertIn('policykit.exec.argv1">login-switch<', policy)
        self.assertNotIn("login-switch", (ROOT / "shell/assets/polkit/49-tessera.rules").read_text())


class GreeterSeen(unittest.TestCase):
    """greeter-start.sh writes this boot's id once someone logged in (or the
    login screen stayed up), never when it quit at once with an error"""

    def run_greeter(self, code):
        with tempfile.TemporaryDirectory() as t:
            t = Path(t)
            share = t / "share"
            (share / "shell").mkdir(parents=True)
            qs = t / "qs"
            qs.write_text(f"#!/bin/sh\nexit {code}\n")
            qs.chmod(0o755)
            (t / "seen").mkdir()
            (t / "run").mkdir()
            env = dict(os.environ, TESSERA_SHARE=str(share), TESSERA_QS=str(qs), TESSERA_GREETER_UI=str(t / "none"),
                       TESSERA_GREETER_SEEN=str(t / "seen/seen"), XDG_RUNTIME_DIR=str(t / "run"))
            subprocess.run(["bash", str(ROOT / "shell/assets/scripts/greeter-start.sh")], env=env, capture_output=True, timeout=60)
            f = t / "seen/seen"
            return f.read_text().strip() if f.exists() else None

    def test_written_after_a_login(self):
        self.assertEqual(self.run_greeter(0), Path("/proc/sys/kernel/random/boot_id").read_text().strip())

    def test_not_when_it_failed_at_once(self):
        self.assertIsNone(self.run_greeter(1))


if __name__ == "__main__":
    unittest.main()
