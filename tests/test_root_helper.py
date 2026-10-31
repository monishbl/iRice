"""iRice's root helper (shell/assets/scripts/irice-root.py) and the update
that goes through it (sysupdate.py), unprivileged: a fake pkexec runs the
helper as you, a fake pacman and a fake yay stand in, and setup installs
into a temporary folder. Nothing real is asked or changed.
"""
import importlib.util
import json
import os
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"
SOURCE = SCRIPTS / "irice-root.py"
SYSUPDATE = SCRIPTS / "sysupdate.py"
SHIM = SCRIPTS / "irice-sudo"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


R = load(SOURCE, "irice_root")

FAKE_PACMAN = r'''#!/usr/bin/env python3
import os, sys
open(os.environ["FAKE_LOG"], "a").write("pacman " + " ".join(sys.argv[1:]) + "\n")
if "-Syu" in sys.argv:
    print(":: Synchronizing package databases...")
    print("(1/1) upgrading vim")
elif "-U" in sys.argv:
    print("(1/1) upgrading apple_cursor")
sys.exit(int(os.environ.get("FAKE_PACMAN_EXIT", "0")))
'''

# pkexec as you: logs its arguments and runs the helper's update() directly
# (the real one refuses without root), with pacman and setup's destination
# pointed at the test's
FAKE_PKEXEC = r'''#!/usr/bin/env python3
import os, sys
open(os.environ["FAKE_LOG"], "a").write("pkexec " + " ".join(sys.argv[1:]) + "\n")
args = sys.argv[1:]
assert args[1:2] == ["-I"] or args[0].endswith("irice-root"), args
i = args.index("update")
code = """
import importlib.util, os, sys
spec = importlib.util.spec_from_file_location("r", sys.argv[1])
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
m.PACMAN = os.environ["FAKE_PACMAN"]
m.PREFIX = os.environ["IRICE_SYSTEM_ROOT"]
sys.exit(m.update(sys.argv[2:], os.getuid()))
"""
os.execv(sys.executable, [sys.executable, "-c", code, os.environ["HELPER_SOURCE"]] + args[i + 1:])
'''

# yay: builds "as you" and asks its --sudo for the root parts
FAKE_YAY = r'''#!/usr/bin/env python3
import os, subprocess, sys
open(os.environ["FAKE_LOG"], "a").write("yay " + " ".join(sys.argv[1:]) + "\n")
sudo = sys.argv[sys.argv.index("--sudo") + 1]
print("==> Making package: apple_cursor 2.0.1-1")
ok = subprocess.run([sudo, "pacman", "-U", "--config", "/etc/pacman.conf", "--noconfirm", "--", "/tmp/apple_cursor.pkg.tar.zst"]).returncode == 0
bad = subprocess.run([sudo, "rm", "-rf", "/tmp/nothing"]).returncode
worse = subprocess.run([sudo, "pacman", "-U", "--hookdir", "/tmp/hooks", "x"]).returncode
again = subprocess.run([sudo, "pacman", "-D", "--asdeps", "apple_cursor"]).returncode
sys.exit(0 if ok and bad != 0 and worse != 0 and again == 0 else 9)
'''


class PacmanChecks(unittest.TestCase):
    def test_allowed(self):
        for argv in (["pacman", "-U", "--config", "/etc/pacman.conf", "--noconfirm", "--", "/a.pkg.tar.zst"],
                     ["/usr/bin/pacman", "-S", "--needed", "--asdeps", "--noconfirm", "foo", "bar"],
                     ["pacman", "-D", "--asexplicit", "foo"], ["pacman", "-Rns", "--noconfirm", "foo"],
                     ["pacman", "-U", "--dbpath=/var/lib/pacman/", "--root", "/", "x"],
                     ["pacman", "-U", "--ignore", "--root", "x"],  # (--root here is --ignore's value)
                     ["pacman", "-U", "--", "--root"]):
            self.assertEqual(R.check_pacman(argv), argv[1:], argv)

    def test_refused(self):
        for argv in (["rm", "-rf", "/"], ["/tmp/pacman", "-U", "x"], ["sh", "-c", "pacman -U x"], [], "pacman -U x",
                     ["pacman", "-U", "--config", "/tmp/evil.conf", "x"], ["pacman", "-U", "--config=/tmp/c", "x"],
                     ["pacman", "-U", "--root", "/mnt", "x"], ["pacman", "-Ur", "/mnt", "x"], ["pacman", "-Sb", "/tmp", "x"],
                     ["pacman", "-U", "--dbpath", "/tmp/db", "x"], ["pacman", "-U", "--hookdir", "/tmp", "x"],
                     ["pacman", "-U", "--gpgdir=/tmp", "x"], ["pacman", "-U", "--cachedir", "/tmp", "x"],
                     ["pacman", "-Syu", "--logfile", "/etc/passwd"], ["pacman", "-U", "--sysroot", "/mnt", "x"],
                     ["pacman", "-U", "--logf=/etc/passwd", "x"], ["pacman", "-U", "--hook", "/tmp", "x"],  # abbreviations
                     ["pacman", "-U", "--config"],  # no value
                     ["pacman", "-Qi", "x"], ["pacman", "-Fy"], ["pacman", "x"], ["pacman", "-S", "-U", "x"],
                     ["pacman", "-U", "a\0b"]):
            with self.assertRaises(R.Refused, msg=argv):
                R.check_pacman(argv)


class Arguments(unittest.TestCase):
    def test_update_arguments(self):
        p = R.parse_update(["--snapshot", "snapper", "--system", "--ignore", "hyprland,aquamarine", "--install", "a,b",
                            "--remove", "c", "--setup", "/home/a/.config/quickshell/irice", "--broker"])
        self.assertEqual((p["snapshot"], p["system"], p["ignore"], p["install"], p["remove"], p["broker"]),
                         ("snapper", True, ["hyprland", "aquamarine"], ["a", "b"], ["c"], True))
        for bad in (["--install", "-x"], ["--install", "a;b"], ["--snapshot", "btrfs"], ["--root", "/"],
                    ["--install"], ["--setup", "relative"], ["--setup", "/a\nb"], ["x"]):
            with self.assertRaises(R.Refused, msg=bad):
                R.parse_update(bad)

    def test_fixes_are_a_fixed_list(self):
        self.assertEqual(R.fix_command("avahi"), ["systemctl", "enable", "--now", "avahi-daemon"])
        self.assertEqual(R.fix_command("firewall")[:2], ["/bin/sh", "-c"])
        for bad in ("rm -rf /", "", "../avahi", "pairing"):
            with self.assertRaises(R.Refused):
                R.fix_command(bad)

    def test_needs_root(self):
        # Unprivileged it does nothing, whatever it's asked
        p = subprocess.run([sys.executable, "-I", str(SOURCE), "fix", "avahi"], capture_output=True, text=True)
        self.assertEqual(p.returncode, R.REFUSED_EXIT)
        self.assertIn("runs as root", p.stderr)


class Wallpaper(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        (self.d / "share").mkdir()
        self.dest = self.d / "share/wallpaper.jpg"

    def tearDown(self):
        self.tmp.cleanup()

    def test_copies_an_image(self):
        src = self.d / "a.png"
        src.write_bytes(b"\x89PNG\r\n\x1a\n" + b"x" * 100)
        R.wallpaper(str(src), os.getuid(), dest=str(self.dest))
        self.assertEqual(self.dest.read_bytes(), src.read_bytes())
        self.assertEqual(stat.S_IMODE(self.dest.stat().st_mode), 0o644)

    def test_refuses_what_isnt_one(self):
        text = self.d / "a.jpg"
        text.write_text("#!/bin/sh\necho hi\n")
        os.mkfifo(self.d / "fifo")
        for bad in (str(text), "relative.jpg", str(self.d), str(self.d / "fifo"), str(self.d / "missing.jpg")):
            with self.assertRaises((R.Refused, OSError), msg=bad):
                R.wallpaper(bad, os.getuid(), dest=str(self.dest))
        self.assertFalse(self.dest.exists())
        # nor when there's no login screen to give it to
        img = self.d / "b.jpg"
        img.write_bytes(b"\xff\xd8\xff\xe0" + b"x" * 10)
        with self.assertRaises(R.Refused):
            R.wallpaper(str(img), os.getuid(), dest=str(self.d / "nowhere/wallpaper.jpg"))

    def test_too_big(self):
        img = self.d / "big.jpg"
        img.write_bytes(b"\xff\xd8\xff" + b"x" * 64)
        old = R.MAX_IMAGE
        R.MAX_IMAGE = 32
        try:
            with self.assertRaisesRegex(R.Refused, "too big"):
                R.wallpaper(str(img), os.getuid(), dest=str(self.dest))
        finally:
            R.MAX_IMAGE = old


class Setup(unittest.TestCase):
    def test_installs_what_changed(self):
        with tempfile.TemporaryDirectory() as t:
            written = R.setup(str(ROOT / "shell"), os.getuid(), prefix=t)
            self.assertEqual(len(written), len(R.FILES))
            helper = Path(t) / "usr/lib/irice/irice-root"
            self.assertEqual(helper.read_bytes(), SOURCE.read_bytes())
            self.assertEqual(stat.S_IMODE(helper.stat().st_mode), 0o755)
            self.assertTrue((Path(t) / "etc/polkit-1/rules.d/49-irice.rules").exists())
            self.assertTrue((Path(t) / "usr/share/polkit-1/actions/org.irice.root.policy").exists())
            self.assertTrue((Path(t) / "usr/lib/irice/qs-wifi-helper.py").exists())
            self.assertEqual(R.setup(str(ROOT / "shell"), os.getuid(), prefix=t), [])  # nothing changed
            helper.chmod(0o777)  # (a wrong mode counts as a change)
            self.assertEqual(R.setup(str(ROOT / "shell"), os.getuid(), prefix=t), [str(helper)])
            self.assertEqual(stat.S_IMODE(helper.stat().st_mode), 0o755)

    def test_policy_actions_match_the_subcommands(self):
        policy = (ROOT / "shell/assets/polkit/org.irice.root.policy").read_text()
        for sub in ("wallpaper", "fix", "wifi-helper", "update"):
            self.assertIn(f'<action id="org.irice.root.{sub}">', policy)
            self.assertIn(f'org.freedesktop.policykit.exec.argv1">{sub}<', policy)
        # every action names its first argument (pkexec takes the first action
        # whose program matches, so one without argv1 could shadow the others)
        self.assertEqual(policy.count("key=\"org.freedesktop.policykit.exec.argv1\""), policy.count("<action "))
        self.assertNotIn("<allow_active>yes", policy)


class Broker(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.log = d / "calls"
        self.pacman = d / "pacman"
        self.pacman.write_text(FAKE_PACMAN)
        self.pacman.chmod(0o755)
        os.environ["FAKE_LOG"] = str(self.log)

    def tearDown(self):
        os.environ.pop("FAKE_LOG", None)
        self.tmp.cleanup()

    def ask(self, requests):
        ours, theirs = socket.socketpair()
        t = threading.Thread(target=R.serve, args=(theirs, None, str(self.pacman)))
        t.start()
        answers = []
        f = ours.makefile("rb")
        for req in requests:
            ours.sendall((json.dumps(req) + "\n").encode())
            text = ""
            for line in f:
                msg = json.loads(line)
                text += msg.get("out", "")
                if "exit" in msg:
                    answers.append((msg["exit"], text))
                    break
        ours.shutdown(socket.SHUT_RDWR)
        f.close()
        ours.close()
        t.join(5)
        self.assertFalse(t.is_alive())  # (it ends when the update closes its end)
        return answers

    def test_runs_pacman_and_nothing_else(self):
        answers = self.ask([
            {"argv": ["pacman", "-U", "--noconfirm", "--", "/tmp/x.pkg.tar.zst"], "cwd": "/"},
            {"argv": ["rm", "-rf", "/tmp/nothing"], "cwd": "/"},
            {"argv": ["pacman", "-U", "--root", "/mnt", "x"], "cwd": "/"},
            {"argv": "pacman -U x"},
            {"argv": ["pacman", "-D", "--asdeps", "x"], "cwd": "/no/such/folder"},
        ])
        self.assertEqual([a[0] for a in answers], [0, 1, 1, 1, 0])
        self.assertIn("(1/1) upgrading apple_cursor", answers[0][1])
        self.assertIn("refused", answers[1][1])
        calls = self.log.read_text().splitlines()
        self.assertEqual(calls, ["pacman -U --noconfirm -- /tmp/x.pkg.tar.zst", "pacman -D --asdeps x"])

    def test_exit_code_comes_back(self):
        os.environ["FAKE_PACMAN_EXIT"] = "3"
        try:
            self.assertEqual(self.ask([{"argv": ["pacman", "-S", "x"], "cwd": "/"}])[0][0], 3)
        finally:
            os.environ.pop("FAKE_PACMAN_EXIT")


class Plan(unittest.TestCase):
    """sysupdate.py's plan: what it leaves out because the system has it covered."""

    def plan(self, args, installed=(), active=()):
        # pacman -Q and systemctl is-active answer from the lists given
        with tempfile.TemporaryDirectory() as d:
            for name, have in (("pacman", installed), ("systemctl", active)):
                (Path(d) / name).write_text("#!/bin/sh\nfor a; do last=$a; done\ncase \" %s \" in *\" $last \"*) exit 0 ;; esac\nexit 1\n" % " ".join(have))
                (Path(d) / name).chmod(0o755)
            old = os.environ["PATH"]
            os.environ["PATH"] = f"{d}:/usr/bin:/bin"
            try:
                return load(SYSUPDATE, "sysupdate_plan").plan_of(args)
            finally:
                os.environ["PATH"] = old

    def test_no_second_snapshot_next_to_snap_pac(self):
        self.assertEqual(self.plan(["--snapshot", "snapper", "--system"])["steps"][0], "snapshot")
        p = self.plan(["--snapshot", "snapper", "--system"], installed=["snap-pac"])
        self.assertEqual(p["snapshot"], "")
        self.assertNotIn("snapshot", p["steps"])
        self.assertEqual(self.plan(["--snapshot", "timeshift"], installed=["timeshift-autosnap"])["snapshot"], "")
        self.assertEqual(self.plan(["--snapshot", "timeshift"], installed=["snap-pac"])["snapshot"], "timeshift")

    def test_no_power_profiles_daemon_next_to_another_power_manager(self):
        args = ["--install", "power-profiles-daemon,jq"]
        self.assertEqual(self.plan(args)["install"], ["power-profiles-daemon", "jq"])
        self.assertEqual(self.plan(args, installed=["tlp"])["install"], ["jq"])
        self.assertEqual(self.plan(args, active=["auto-cpufreq"])["install"], ["jq"])
        self.assertEqual(self.plan(args, installed=["tuned"])["install"], ["jq"])


class AurInstall(Plan):
    """--aur-install: packages Settings offers (a fingerprint reader's driver)."""

    def test_the_aur_step_installs_them(self):
        p = self.plan(["--aur-install", "libfprint-tod,libfprint-2-tod1-broadcom,fprintd"])
        self.assertEqual([s for s in p["steps"] if s != "setup"], ["aur"])  # (setup: when the installed helper is older)
        su = load(SYSUPDATE, "sysupdate_cmd")
        cmd = su.aur_command("yay", p)
        self.assertEqual(cmd[:4], ["yay", "-S", "--needed", "--noconfirm"])
        self.assertEqual(cmd[cmd.index("--") + 1:], ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"])
        self.assertNotIn("-Sua", cmd)

    def test_names_are_checked(self):
        with self.assertRaises(SystemExit):
            self.plan(["--aur-install", "--root=/tmp"])


class FingerprintPam(unittest.TestCase):
    """irice-root.py fingerprint: one marked line, first among the rules."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = Path(self.dir.name)
        self.etc, self.vendor = d / "etc", d / "vendor"
        self.etc.mkdir()
        self.vendor.mkdir()
        self.module = d / "pam_fprintd.so"
        self.module.write_text("")
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_fp")
        self.r.write_root = lambda dest, data, mode: Path(dest).write_bytes(data)

    def tearDown(self):
        self.dir.cleanup()

    def run_fp(self, on, where):
        return self.r.fingerprint(on, where, etc=str(self.etc), vendor=str(self.vendor), module=(str(self.module),))

    def test_sudo_on_and_off(self):
        original = "#%PAM-1.0\nauth\t\tinclude\t\tsystem-auth\naccount\t\tinclude\t\tsystem-auth\n"
        (self.etc / "sudo").write_text(original)
        self.run_fp(True, "sudo")
        lines = (self.etc / "sudo").read_text().splitlines()
        self.assertEqual(lines[0], "#%PAM-1.0")
        self.assertIn("pam_fprintd.so", lines[1])
        self.assertIn("sufficient", lines[1])
        self.run_fp(True, "sudo")  # (twice: still one line)
        self.assertEqual((self.etc / "sudo").read_text().count("pam_fprintd"), 1)
        self.run_fp(False, "sudo")
        self.assertEqual((self.etc / "sudo").read_text(), original)

    def test_a_vendor_file_is_copied_to_etc(self):
        (self.vendor / "polkit-1").write_text("#%PAM-1.0\n\nauth include system-auth\n")
        self.assertEqual(self.run_fp(True, "polkit"), [str(self.etc / "polkit-1")])
        self.assertIn("pam_fprintd", (self.etc / "polkit-1").read_text())
        self.assertNotIn("pam_fprintd", (self.vendor / "polkit-1").read_text())

    def test_login_covers_the_login_screens_installed(self):
        (self.etc / "ly").write_text("#%PAM-1.0\nauth include login\n")
        (self.etc / "sddm").write_text("#%PAM-1.0\nauth include system-login\n")
        self.assertEqual(sorted(self.run_fp(True, "login")), sorted([str(self.etc / "ly"), str(self.etc / "sddm")]))

    def test_refused_without_fprintd_or_for_anything_else(self):
        (self.etc / "sudo").write_text("#%PAM-1.0\nauth include system-auth\n")
        self.module.unlink()
        with self.assertRaises(self.r.Refused):
            self.run_fp(True, "sudo")
        self.run_fp(False, "sudo")  # (taking it out still works)
        with self.assertRaises(self.r.Refused):
            self.run_fp(True, "shadow")


class SysUpdate(unittest.TestCase):
    """The whole real (not --fake) run: one pkexec, the AUR's pacman through it."""

    def setUp(self):
        if os.path.exists("/var/lib/pacman/db.lck"):
            self.skipTest("a real pacman is running")
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        for name, text in (("pacman", FAKE_PACMAN), ("pkexec", FAKE_PKEXEC), ("yay", FAKE_YAY)):
            (self.bin / name).write_text(text)
            (self.bin / name).chmod(0o755)
        self.sysroot = d / "root"
        self.sysroot.mkdir()
        self.run_dir = d / "run"
        self.run_dir.mkdir(mode=0o700)
        self.log = d / "calls"
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", XDG_RUNTIME_DIR=str(self.run_dir),
                        IRICE_SYSTEM_ROOT=str(self.sysroot), FAKE_LOG=str(self.log), FAKE_PACMAN=str(self.bin / "pacman"),
                        HELPER_SOURCE=str(SOURCE))
        for k in ("IRICE_FAKE_SYSUPDATE", "IRICE_SANDBOX"):
            self.env.pop(k, None)

    def tearDown(self):
        self.tmp.cleanup()

    def sysupdate(self, *args):
        return subprocess.run([sys.executable, "-I", str(SYSUPDATE), *args], env=self.env, capture_output=True, text=True, timeout=60)

    def status(self):
        return json.loads((self.run_dir / "irice-sysupdate/status.json").read_text())

    def test_one_password_for_the_system_and_the_aur(self):
        self.assertEqual(self.sysupdate("helper-status").returncode, 1)  # not installed yet
        p = self.sysupdate("run", "--system", "--aur")
        self.assertEqual(p.returncode, 0, p.stderr)
        st = self.status()
        self.assertEqual(st["state"], "done", (st, (self.run_dir / "irice-sysupdate/log").read_text()))
        self.assertEqual(st["steps"], ["setup", "system", "aur"])
        self.assertIn("vim", st["upgraded"])
        self.assertIn("apple_cursor", st["upgraded"])  # (yay's pacman -U, through the root part)
        calls = self.log.read_text().splitlines()
        # one pkexec: this folder's helper (not installed yet), which installs itself
        pk = [c for c in calls if c.startswith("pkexec ")]
        self.assertEqual(len(pk), 1, calls)
        self.assertIn(f"{SOURCE} update --system --setup {ROOT / 'shell'} --broker", pk[0])
        # yay asks irice-sudo, never pkexec
        yay = [c for c in calls if c.startswith("yay ")][0]
        self.assertIn(f"--sudo {SHIM}", yay)
        self.assertNotIn("pkexec", yay)
        # pacman only, and only as allowed
        pacman = [c for c in calls if c.startswith("pacman ")]
        self.assertEqual(pacman, ["pacman -Syu --noconfirm",
                                  "pacman -U --config /etc/pacman.conf --noconfirm -- /tmp/apple_cursor.pkg.tar.zst",
                                  "pacman -D --asdeps apple_cursor"])
        self.assertFalse((self.run_dir / "irice-sysupdate/sudo.sock").exists())
        # setup installed the helper: the next update runs the installed copy
        self.assertEqual(self.sysupdate("helper-status").returncode, 0)
        self.assertTrue(os.access(self.sysroot / "usr/lib/irice/irice-root", os.X_OK))
        self.log.unlink()
        p = self.sysupdate("run", "--system")
        self.assertEqual(self.status()["state"], "done")
        pk = [c for c in self.log.read_text().splitlines() if c.startswith("pkexec ")]
        self.assertEqual(pk, [f"pkexec {self.sysroot}/usr/lib/irice/irice-root update --system"])

    def test_the_shim_refuses_outside_an_update(self):
        env = {k: v for k, v in self.env.items() if not k.startswith("IRICE_SUDO")}
        p = subprocess.run([str(SHIM), "pacman", "-U", "x"], env=env, capture_output=True, text=True)
        self.assertEqual(p.returncode, 1)
        self.assertIn("only works during an iRice update", p.stderr)
        self.assertEqual(subprocess.run([str(SHIM), "-v"], env=env).returncode, 0)


class WifiHelperStart(unittest.TestCase):
    """Quick Share's Wi-Fi helper: the installed root helper's copy without a
    password prompt (and without the island saying one is coming), else
    this folder's under pkexec, as before."""

    def start(self, root_helper):
        sys.path.insert(0, str(SCRIPTS / "quickshare"))
        from nearby import wifi
        with tempfile.TemporaryDirectory() as t:
            log = Path(t) / "calls"
            pk = Path(t) / "pkexec"
            pk.write_text("#!/bin/sh\necho \"$@\" >> " + str(log) + "\necho '{\"ok\": true, \"ready\": true}'\ncat >/dev/null\n")
            pk.chmod(0o755)
            old = os.environ["PATH"]
            os.environ["PATH"] = f"{t}:{old}"
            asked = []
            try:
                h = wifi.WifiHelper(on_auth=lambda: asked.append(1), root_helper=root_helper)

                async def go():
                    await h._start()
                    await h.close()
                import asyncio
                asyncio.run(go())
            finally:
                os.environ["PATH"] = old
            return log.read_text().strip(), asked

    def test_installed_helper_still_asks_at_the_start(self):
        # (it makes and joins Wi-Fi networks: not one of the silent jobs)
        with tempfile.TemporaryDirectory() as t:
            fake = Path(t) / "irice-root"
            fake.write_text("#!/bin/sh\n")
            fake.chmod(0o755)
            cmd, asked = self.start(str(fake))
        self.assertEqual(cmd, f"{fake} wifi-helper")
        self.assertEqual(asked, [1])

    def test_before_setup_its_the_password_as_before(self):
        cmd, asked = self.start("/nonexistent/irice-root")
        self.assertTrue(cmd.endswith("-I " + str(SCRIPTS / "quickshare/qs-wifi-helper.py")), cmd)
        self.assertEqual(asked, [1])


if __name__ == "__main__":
    unittest.main()
