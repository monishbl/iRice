"""iRice's root helper (shell/assets/scripts/irice-root.py) and the update
that goes through it (sysupdate.py), unprivileged: a fake pkexec runs the
helper as you, a fake pacman and a fake yay stand in, and setup installs
into a temporary folder. Nothing real is asked or changed.
"""
import importlib.util
import json
import os
import shutil
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
        fprintd = R.fix_command("fprintd")[2]
        self.assertIn("TimeoutStartSec=5min", fprintd)
        self.assertIn("/etc/systemd/system/fprintd.service.d/irice-timeout.conf", fprintd)
        self.assertTrue(fprintd.rstrip().endswith("systemctl start fprintd"))
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


class LoginLook(unittest.TestCase):
    GOOD = {"style": "look", "look": "material", "mode": "dark", "seed": "#6750A4", "materialStyle": "vibrant",
            "clockFace": "dynamic", "clockWeight": 640, "clockWidth": 112, "clockRound": True}

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        (self.d / "share").mkdir()
        self.dest = self.d / "share/greeter.json"

    def tearDown(self):
        self.tmp.cleanup()

    def test_writes_it_checked_and_sorted(self):
        R.login_look(json.dumps(self.GOOD), dest=str(self.dest))
        text = self.dest.read_text()
        self.assertEqual(json.loads(text), self.GOOD)
        self.assertEqual(text, json.dumps(self.GOOD, sort_keys=True, separators=(",", ":")))
        self.assertEqual(stat.S_IMODE(self.dest.stat().st_mode), 0o644)

    def test_refuses_anything_else(self):
        bad = [
            "not json", "[]", json.dumps({**self.GOOD, "extra": 1}),
            json.dumps({k: v for k, v in self.GOOD.items() if k != "seed"}),
            json.dumps({**self.GOOD, "style": "$(reboot)"}), json.dumps({**self.GOOD, "look": "aqua"}),
            json.dumps({**self.GOOD, "seed": "red"}), json.dumps({**self.GOOD, "seed": "#12345"}),
            json.dumps({**self.GOOD, "clockWeight": 2000}), json.dumps({**self.GOOD, "clockWeight": 640.5}),
            json.dumps({**self.GOOD, "clockWidth": True}), json.dumps({**self.GOOD, "clockRound": 1}),
            json.dumps({**self.GOOD, "clockFace": "digital"}), json.dumps({**self.GOOD, "clock24": "yes"}), "x" * 5000,
        ]
        for b in bad:
            with self.assertRaises(R.Refused, msg=b[:60]):
                R.login_look(b, dest=str(self.dest))
        self.assertFalse(self.dest.exists())

    def test_24_hour_time_is_optional(self):
        R.login_look(json.dumps({**self.GOOD, "clock24": True}), dest=str(self.dest))
        self.assertTrue(json.loads(self.dest.read_text())["clock24"])
        R.login_look(json.dumps(self.GOOD), dest=str(self.dest))  # (a shell from before it)
        self.assertNotIn("clock24", json.loads(self.dest.read_text()))

    def test_not_without_the_login_screen(self):
        with self.assertRaises(R.Refused):
            R.login_look(json.dumps(self.GOOD), dest=str(self.d / "nowhere/greeter.json"))
        (self.d / "link").symlink_to(self.d / "share")
        with self.assertRaises(R.Refused):
            R.login_look(json.dumps(self.GOOD), dest=str(self.d / "link/greeter.json"))


class LoginCopy(unittest.TestCase):
    """setup refreshes the login screen's copy of iRice (./install.sh
    --greeter's /usr/share/irice) from the checkout, only where there is one."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.repo = self.d / "repo"
        for name in ("shell", "material"):
            (self.repo / name / "modules").mkdir(parents=True)
            (self.repo / name / "shell.qml").write_text(f"// {name}\n")
            (self.repo / name / "modules/A.qml").write_text("Item {}\n")
        (self.repo / "shell/assets/scripts").mkdir(parents=True)
        script = self.repo / "shell/assets/scripts/greeter-start.sh"
        script.write_text("#!/bin/sh\n")
        script.chmod(0o755)
        (self.repo / "shell/__pycache__").mkdir()
        (self.repo / "shell/__pycache__/x.pyc").write_bytes(b"x")
        (self.repo / "shell/link.qml").symlink_to("/etc/shadow")
        (self.repo / "hypr").mkdir()
        (self.repo / "hypr/greeter.lua").write_text("-- greeter\n")
        self.share = self.d / "root/usr/share/irice"

    def tearDown(self):
        self.tmp.cleanup()

    def refresh(self, src=None):
        return R.refresh_login(str(src or self.repo / "shell"), os.getuid(), prefix=str(self.d / "root"))

    def test_nothing_without_the_login_screen(self):
        self.assertEqual(self.refresh(), [])
        self.assertFalse(self.share.exists())

    def test_copies_the_shell_and_material(self):
        (self.share / "shell").mkdir(parents=True)
        (self.share / "shell/old.qml").write_text("gone\n")
        (self.share / "greeter.lua").write_text("-- old\n\n-- virtual machine (kvm): software cursor\nhl.config({})\n")
        written = self.refresh()
        self.assertEqual(sorted(written), sorted(str(self.share / x) for x in ("shell", "material", "greeter.lua")))
        self.assertEqual((self.share / "material/shell.qml").read_text(), "// material\n")
        self.assertFalse((self.share / "shell/old.qml").exists())
        self.assertFalse((self.share / "shell/__pycache__").exists())
        self.assertFalse((self.share / "shell/link.qml").exists())  # (links aren't followed)
        self.assertEqual(stat.S_IMODE((self.share / "shell/assets/scripts/greeter-start.sh").stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE((self.share / "shell/shell.qml").stat().st_mode), 0o644)
        lua = (self.share / "greeter.lua").read_text()
        self.assertTrue(lua.startswith("-- greeter\n"))
        self.assertIn("-- virtual machine (kvm)", lua)  # (install.sh's addition stays)
        self.assertFalse((self.share / "shell.new").exists() or (self.share / "shell.old").exists())
        self.assertEqual(self.refresh(), [])  # nothing changed

    def test_from_an_updates_staged_tree(self):
        (self.share / "shell").mkdir(parents=True)
        staged = self.d / "incoming"
        shutil.copytree(self.repo, staged / "tree", symlinks=True)
        self.assertIn(str(self.share / "shell"), self.refresh(staged))
        # (a staged folder of only the helper's files: nothing to copy from)
        self.assertEqual(R.refresh_login(str(self.d / "empty"), os.getuid(), prefix=str(self.d / "root")), [])


class LoginTreeStaged(unittest.TestCase):
    """An update stages the incoming version's whole tree when the login
    screen has its own copy (sysupdate.stage_login_tree)."""

    def test_staged_only_with_a_login_copy(self):
        su = load(SYSUPDATE, "sysupdate_stage")
        with tempfile.TemporaryDirectory() as t:
            repo = Path(t) / "repo"
            for p in ("shell/shell.qml", "material/shell.qml", "hypr/greeter.lua", "README.md"):
                (repo / p).parent.mkdir(parents=True, exist_ok=True)
                (repo / p).write_text(p + "\n")
            git = ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t"]
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(git + ["add", "-A"], check=True)
            subprocess.run(git + ["commit", "-qm", "x"], check=True)
            out = Path(t) / "incoming"
            out.mkdir()
            su.stage_login_tree(str(repo), "HEAD", str(out), copy=str(Path(t) / "missing"))
            self.assertFalse((out / "tree").exists())
            su.stage_login_tree(str(repo), "HEAD", str(out), copy=t)
            self.assertEqual((out / "tree/material/shell.qml").read_text(), "material/shell.qml\n")
            self.assertTrue((out / "tree/hypr/greeter.lua").exists())
            self.assertFalse((out / "tree/README.md").exists())
            self.assertEqual(R.login_tree(str(out)), str(out / "tree"))


class Setup(unittest.TestCase):
    def test_ananicy_rule_only_where_ananicy_is(self):
        with tempfile.TemporaryDirectory() as t:
            R.setup(str(ROOT / "shell"), os.getuid(), prefix=t)
            self.assertFalse((Path(t) / "etc/ananicy.d/irice.rules").exists())
        with tempfile.TemporaryDirectory() as t:
            (Path(t) / "etc/ananicy.d").mkdir(parents=True)
            written = R.setup(str(ROOT / "shell"), os.getuid(), prefix=t)
            rule = Path(t) / "etc/ananicy.d/irice.rules"
            self.assertIn(str(rule), written)
            self.assertIn('"irice-shell"', rule.read_text())

    def test_installs_what_changed(self):
        with tempfile.TemporaryDirectory() as t:
            written = R.setup(str(ROOT / "shell"), os.getuid(), prefix=t)
            self.assertEqual(len(written), len(R.FILES) + len(R.BOOTSPLASH_IDS))  # (and a folder per boot animation)
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
        for sub in ("wallpaper", "login-look", "fix", "wifi-helper", "update", "bootsplash"):
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


class AurHelperChoice(unittest.TestCase):
    """sysupdate.py aur_setup: whatever Arch-based system this is."""

    def setUp(self):
        self.su = load(SYSUPDATE, "sysupdate_choice")
        self.which = self.su.shutil.which  # (the shared shutil module: put back after)

    def choose(self, have=(), repo=(), installed=()):
        su = self.su
        su.shutil.which = lambda n: "/usr/bin/" + n if n in have else None
        su.repo_has = lambda n: n in repo
        su.installed = lambda n: n in installed
        return su.aur_setup()

    def tearDown(self):
        self.su.shutil.which = self.which

    def test_yay_or_paru_there(self):
        self.assertEqual(self.choose(have=("paru",)), ("paru", [], False))

    def test_manjaro_pamac(self):
        self.assertEqual(self.choose(have=("pamac",), repo=("yay",)), ("pamac", [], False))

    def test_from_the_repositories(self):
        # (CachyOS, EndeavourOS, Garuda: yay or paru in their own repositories)
        self.assertEqual(self.choose(repo=("paru",)), ("paru", ["paru"], False))

    def test_plain_arch_builds_yay(self):
        self.assertEqual(self.choose(installed=("git",)), ("yay", ["base-devel"], True))

    def test_pamac_commands(self):
        self.assertEqual(self.su.aur_command("pamac", {"aur_install": ["howdy-next"]}), ["pamac", "build", "--no-confirm", "howdy-next"])
        self.assertEqual(self.su.aur_command("pamac", {"aur_install": [], "ignore": []})[:3], ["pamac", "upgrade", "--aur"])
        # (what you hold back stays held back with pamac too)
        self.assertEqual(self.su.aur_command("pamac", {"aur_install": [], "ignore": ["hyprland", "mesa"]})[-2:], ["--ignore", "hyprland,mesa"])


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
        # (off again: the copy goes, and the package's own file is used)
        self.assertEqual(self.run_fp(False, "polkit"), [str(self.etc / "polkit-1")])
        self.assertFalse((self.etc / "polkit-1").exists())

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


class PhoneCam(unittest.TestCase):
    """irice-root.py phonecam: the v4l2loopback drop-ins, and loading it."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = Path(self.dir.name)
        self.etc, self.dev = d / "etc", d / "dev"
        self.etc.mkdir()
        self.dev.mkdir()
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_cam")

        def write(dest, data, mode):
            Path(dest).parent.mkdir(parents=True, exist_ok=True)
            Path(dest).write_bytes(data)
        self.r.write_root = write
        self.calls = []

    def tearDown(self):
        self.dir.cleanup()

    def fake(self, modinfo=0, load=0, appears=True):
        def run(cmd, **kw):
            self.calls.append(cmd)
            rc = modinfo if cmd[0] == "modinfo" else load if cmd == ["modprobe", "v4l2loopback"] else 0
            if cmd == ["modprobe", "v4l2loopback"] and rc == 0 and appears:
                (self.dev / "video42").write_text("")  # (the module made it)
            return subprocess.CompletedProcess(cmd, rc, "", "")
        return run

    def test_on(self):
        changed = self.r.phonecam(True, etc=str(self.etc), run=self.fake(), dev_dir=str(self.dev))
        self.assertEqual(len(changed), 2)
        self.assertIn('video_nr=42 card_label="Phone camera" exclusive_caps=1', (self.etc / "modprobe.d/irice-phonecam.conf").read_text())
        self.assertEqual((self.etc / "modules-load.d/irice-phonecam.conf").read_text(), "v4l2loopback\n")
        self.assertIn(["modprobe", "v4l2loopback"], self.calls)

    def test_already_there(self):
        (self.dev / "video42").write_text("")
        self.r.phonecam(True, etc=str(self.etc), run=self.fake(), dev_dir=str(self.dev))
        self.assertNotIn(["modprobe", "-r", "v4l2loopback"], self.calls)  # (a camera in use isn't pulled away)

    def test_no_module(self):
        with self.assertRaises(self.r.Refused):
            self.r.phonecam(True, etc=str(self.etc), run=self.fake(modinfo=1), dev_dir=str(self.dev))
        self.assertFalse((self.etc / "modprobe.d").exists())

    def test_busy(self):
        with self.assertRaises(self.r.Refused):
            self.r.phonecam(True, etc=str(self.etc), run=self.fake(load=1), dev_dir=str(self.dev))

    def test_loaded_by_another_app(self):
        # (modprobe -r refused, modprobe a no-op: no video42)
        with self.assertRaises(self.r.Refused):
            self.r.phonecam(True, etc=str(self.etc), run=self.fake(appears=False), dev_dir=str(self.dev))

    def test_off(self):
        self.r.phonecam(True, etc=str(self.etc), run=self.fake(), dev_dir=str(self.dev))
        changed = self.r.phonecam(False, etc=str(self.etc), run=self.fake(), dev_dir=str(self.dev))
        self.assertEqual(len(changed), 2)
        self.assertFalse((self.etc / "modprobe.d/irice-phonecam.conf").exists())


class FaceUnlock(unittest.TestCase):
    """irice-root.py face: Howdy's camera in its config, and faces only for
    the person who asked."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = Path(self.dir.name)
        self.config = d / "config.ini"
        self.models = d / "models"
        self.models.mkdir()
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_face")
        self.r.write_root = lambda dest, data, mode: Path(dest).write_bytes(data)
        self.calls = []
        self.uid = os.getuid()
        self.me = __import__("pwd").getpwuid(self.uid).pw_name

    def tearDown(self):
        self.dir.cleanup()

    def face(self, args, exists=lambda p: True, which=lambda n: "/usr/bin/howdy"):
        def run(cmd, **kw):
            self.calls.append(cmd)
            return subprocess.CompletedProcess(cmd, 0, "", "")
        return self.r.face(args, self.uid, run=run, config=str(self.config), exists=exists, which=which, models=str(self.models))

    def test_setup_sets_the_camera(self):
        self.config.write_text("[core]\nno_confirmation = false\n\n[video]\ndevice_path = none\ndark_threshold = 60\n")
        cam = "/dev/v4l/by-path/pci-0000:00:14.0-usb-0:6:1.2-video-index0"
        self.face(["setup", cam])
        text = self.config.read_text()
        self.assertIn("device_path = " + cam, text)
        self.assertEqual(text.count("device_path"), 1)
        self.assertIn("dark_threshold = 60", text)
        self.assertIn(["/usr/bin/howdy", "download-models"], self.calls)

    def test_setup_without_a_video_section(self):
        self.config.write_text("[core]\nx = 1\n")
        (self.models / "a.onnx").write_text("")
        self.face(["setup", "/dev/video2"])
        self.assertTrue(self.config.read_text().endswith("[video]\ndevice_path = /dev/video2\n"))
        self.assertEqual(self.calls, [])  # (models already there)

    def test_not_a_camera(self):
        for bad in ("/etc/shadow", "/dev/video2; rm -rf /", "/dev/v4l/by-path/../../etc/passwd"):
            with self.assertRaises(self.r.Refused):
                self.face(["setup", bad])
        with self.assertRaises(self.r.Refused):
            self.face(["setup", "/dev/video9"], exists=lambda p: False)

    def test_faces_are_the_callers(self):
        self.face(["add"])
        self.face(["clear"])
        self.assertEqual(self.calls[0][:5], ["/usr/bin/howdy", "-U", self.me, "--plain", "-y"])
        self.assertEqual(self.calls[1], ["/usr/bin/howdy", "-U", self.me, "-y", "clear"])
        with self.assertRaises(self.r.Refused):
            self.face(["add", "someone-else"])

    def test_no_howdy(self):
        with self.assertRaises(self.r.Refused):
            self.face(["add"], which=lambda n: None)


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
                        HELPER_SOURCE=str(SOURCE), XDG_STATE_HOME=str(d / "state"))
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
        # (pacman -Q before and after: what changed, for going back)
        pacman = [c for c in calls if c.startswith("pacman ") and c != "pacman -Q"]
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


class QuickShareTrust(unittest.TestCase):
    """irice-root.py quickshare-trust: a polkit rule letting Quick Share's
    Wi-Fi helper start without the password, for the one person who asked."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_trust")
        self.r.write_root = lambda dest, data, mode: (os.makedirs(os.path.dirname(dest), exist_ok=True), Path(dest).write_bytes(data))
        self.user = __import__("pwd").getpwuid(os.getuid()).pw_name

    def tearDown(self):
        self.dir.cleanup()

    def test_on_and_off(self):
        changed = self.r.quickshare_trust(True, os.getuid(), prefix=self.dir.name)
        rule = Path(self.dir.name, "etc/polkit-1/rules.d/49-irice-quickshare.rules")
        mark = Path(self.dir.name, "etc/irice/quickshare-trusted")
        self.assertEqual(sorted(changed), sorted([str(rule), str(mark)]))
        self.assertIn(f'subject.user === "{self.user}"', rule.read_text())
        self.assertEqual(mark.read_text().strip(), self.user)
        self.r.quickshare_trust(False, os.getuid(), prefix=self.dir.name)
        self.assertFalse(rule.exists() or mark.exists())

    def test_not_for_root_or_odd_names(self):
        with self.assertRaises(self.r.Refused):
            self.r.quickshare_trust(True, 0, prefix=self.dir.name)
        with self.assertRaises(self.r.Refused):
            self.r.quickshare_trust_rule('x"; return polkit.Result.YES; //')

    @unittest.skipUnless(shutil.which("node"), "needs node")
    def test_the_rule_allows_only_the_wifi_helper_for_that_user(self):
        harness = r"""
var rules = [];
var polkit = { Result: { YES: "yes", NOT_HANDLED: "nh" }, addRule: function (f) { rules.push(f); } };
%s
function ask(id, prog, cmd, user, active, local) {
    var action = { id: id, lookup: function (k) { return k === "program" ? prog : cmd; } };
    var subject = { user: user, active: active, local: local };
    return rules[0](action, subject);
}
var H = "/usr/lib/irice/irice-root";
console.log(JSON.stringify([
    ask("org.irice.root.wifi-helper", H, H + " wifi-helper", "alice", true, true),
    ask("org.irice.root.wifi-helper", H, H + " wifi-helper", "bob", true, true),
    ask("org.irice.root.wifi-helper", H, H + " wifi-helper", "alice", false, true),
    ask("org.irice.root.wifi-helper", H, H + " wifi-helper", "alice", true, false),
    ask("org.irice.root.update", H, H + " update", "alice", true, true),
    ask("org.irice.root.wifi-helper", "/bin/sh", H + " wifi-helper", "alice", true, true),
    ask("org.irice.root.wifi-helper", H, H + " wifi-helper extra", "alice", true, true)
]));
""" % self.r.quickshare_trust_rule("alice")
        out = subprocess.run(["node", "-e", harness], capture_output=True, text=True, timeout=30)
        self.assertEqual(json.loads(out.stdout), ["yes", "nh", "nh", "nh", "nh", "nh", "nh"], out.stderr)


class BootSplash(unittest.TestCase):
    """irice-root.py bootsplash: the boot animation, in a folder standing in
    for the system, with stand-ins for plymouth-set-default-theme and
    mkinitcpio. Nothing real is asked or changed."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_splash")
        self.r.write_root = lambda dest, data, mode: (os.makedirs(os.path.dirname(dest), exist_ok=True), Path(dest).write_bytes(data))
        put = self.put
        put("usr/share/plymouth/themes/cachyos-bootanimation/cachyos-bootanimation.plymouth", "[Plymouth Theme]\n")
        put("usr/share/plymouth/themes/spinner/spinner.plymouth", "[Plymouth Theme]\n")
        put("usr/share/plymouth/themes/text/text.plymouth", "[Plymouth Theme]\n")
        put("usr/share/plymouth/plymouthd.defaults", "[Daemon]\nTheme=bgrt\nShowDelay=0\n")
        put("etc/plymouth/plymouthd.conf", "[Daemon]\nTheme=cachyos-bootanimation\n")
        put("etc/mkinitcpio.conf", "MODULES=()\nHOOKS=(base systemd autodetect microcode kms modconf block keyboard sd-vconsole plymouth filesystems)\n")
        put("etc/mkinitcpio.d/linux.preset", 'PRESETS=(\'default\')\ndefault_image="/boot/initramfs-linux.img"\n#fallback_image="/boot/initramfs-linux-fallback.img"\n')
        put("boot/initramfs-linux.img", "built with cachyos-bootanimation")
        put("boot/initramfs-linux-lts.img", "built with cachyos-bootanimation (lts)")
        # setup's copy of the themes (root's, in /usr/lib/irice/plymouth)
        self.r.install_themes(str(ROOT / "shell"), os.getuid(), str(self.root))
        self.calls = []
        self.fail = False

    def tearDown(self):
        self.dir.cleanup()

    def put(self, rel, text):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def theme(self):
        return self.r.current_theme(str(self.root))

    def run_(self, cmd, **kw):
        self.calls.append(cmd)
        if cmd[0] == "plymouth-set-default-theme":
            self.put("etc/plymouth/plymouthd.conf", f"[Daemon]\nTheme={cmd[1]}\n")
            return subprocess.CompletedProcess(cmd, 0, "", "")
        if cmd == ["mkinitcpio", "-P"]:
            for img in ("boot/initramfs-linux.img", "boot/initramfs-linux-lts.img"):
                self.put(img, "half a boot image" if self.fail else "built with " + self.theme())
            if self.fail:
                return subprocess.CompletedProcess(cmd, 1, "==> Building image\n==> ERROR: module not found: 'nvidia'\n", "")
            return subprocess.CompletedProcess(cmd, 0, "==> Image generation successful\n", "")
        raise AssertionError(cmd)

    def splash(self, choice, tools=("plymouth-set-default-theme", "mkinitcpio")):
        return self.r.bootsplash(choice, prefix=str(self.root), run=self.run_,
                                 which=lambda n: "/usr/bin/" + n if n in tools else None)

    def record(self):
        p = self.root / "etc/irice/bootsplash.json"
        return json.loads(p.read_text()) if p.exists() else None

    def test_a_design_is_installed_set_and_built(self):
        self.splash("breathe")
        theme = self.root / "usr/share/plymouth/themes/irice-breathe"
        src = ROOT / "shell/assets/plymouth/irice-breathe"
        self.assertEqual(sorted(p.name for p in theme.iterdir()), sorted(p.name for p in src.iterdir()))
        self.assertEqual((theme / "irice-breathe.script").read_bytes(), (src / "irice-breathe.script").read_bytes())
        self.assertEqual(self.theme(), "irice-breathe")
        self.assertEqual(self.calls, [["plymouth-set-default-theme", "irice-breathe"], ["mkinitcpio", "-P"]])
        self.assertEqual((self.root / "boot/initramfs-linux.img").read_text(), "built with irice-breathe")
        self.assertEqual(self.record(), {"original": "cachyos-bootanimation", "applied": "irice-breathe"})
        self.assertFalse((self.root / "var/lib/irice/initramfs-backup").exists())

    def test_system_puts_back_the_original(self):
        self.splash("breathe")
        self.splash("orbit")  # (another design: the original is still the system's)
        self.assertFalse((self.root / "usr/share/plymouth/themes/irice-breathe").exists())
        self.assertEqual(self.record(), {"original": "cachyos-bootanimation", "applied": "irice-orbit"})
        self.splash("system")
        self.assertEqual(self.theme(), "cachyos-bootanimation")
        self.assertEqual((self.root / "boot/initramfs-linux.img").read_text(), "built with cachyos-bootanimation")
        self.assertIsNone(self.record())
        self.assertFalse((self.root / "usr/share/plymouth/themes/irice-orbit").exists())

    def test_none_is_plymouths_text_theme(self):
        self.splash("none")
        self.assertEqual(self.theme(), "text")
        self.assertEqual(self.record(), {"original": "cachyos-bootanimation", "applied": "text"})
        self.splash("system")
        self.assertEqual(self.theme(), "cachyos-bootanimation")

    def test_already_set_rebuilds_nothing(self):
        self.splash("frost")
        self.calls.clear()
        self.assertEqual(self.splash("frost"), ["already irice-frost"])
        self.assertEqual(self.splash("system") and self.splash("system"), ["already cachyos-bootanimation"])

    def test_a_theme_set_elsewhere_becomes_the_systems(self):
        self.splash("bloom")
        self.put("etc/plymouth/plymouthd.conf", "[Daemon]\nTheme=spinner\n")  # (plymouth-set-default-theme by hand)
        self.splash("orbit")
        self.assertEqual(self.record()["original"], "spinner")
        self.splash("system")
        self.assertEqual(self.theme(), "spinner")

    def test_a_failed_rebuild_changes_nothing(self):
        conf = (self.root / "etc/plymouth/plymouthd.conf").read_bytes()
        self.fail = True
        with self.assertRaises(self.r.Refused) as e:
            self.splash("orbit")
        self.assertIn("nothing changed", str(e.exception))
        self.assertIn("ERROR: module not found: 'nvidia'", str(e.exception))
        self.assertEqual((self.root / "etc/plymouth/plymouthd.conf").read_bytes(), conf)
        self.assertEqual((self.root / "boot/initramfs-linux.img").read_text(), "built with cachyos-bootanimation")
        self.assertEqual((self.root / "boot/initramfs-linux-lts.img").read_text(), "built with cachyos-bootanimation (lts)")
        self.assertFalse((self.root / "usr/share/plymouth/themes/irice-orbit").exists())
        self.assertIsNone(self.record())
        self.assertFalse((self.root / "var/lib/irice/initramfs-backup").exists())

    def test_a_failed_rebuild_keeps_the_design_before(self):
        self.splash("breathe")
        self.fail = True
        with self.assertRaises(self.r.Refused):
            self.splash("bloom")
        self.assertEqual(self.theme(), "irice-breathe")
        self.assertEqual((self.root / "boot/initramfs-linux.img").read_text(), "built with irice-breathe")
        self.assertTrue((self.root / "usr/share/plymouth/themes/irice-breathe/irice-breathe.plymouth").exists())
        self.assertEqual(self.record()["applied"], "irice-breathe")

    def test_only_its_names(self):
        for bad in ("", "irice-breathe", "../breathe", "BREATHE", "breathe;reboot", "spinner", "text"):
            with self.assertRaises(self.r.Refused, msg=bad):
                self.splash(bad)
        self.assertEqual(self.calls, [])

    def test_without_plymouth_or_mkinitcpio_it_says_why(self):
        for tools, said in ((("mkinitcpio",), "Install the plymouth package"),
                            (("plymouth-set-default-theme", "dracut"), "dracut"),
                            (("plymouth-set-default-theme", "booster"), "booster")):
            with self.assertRaises(self.r.Refused) as e:
                self.splash("orbit", tools)
            self.assertIn(said, str(e.exception))
        self.put("etc/mkinitcpio.conf", "HOOKS=(base udev autodetect block filesystems)\n")
        with self.assertRaises(self.r.Refused) as e:
            self.splash("orbit")
        self.assertIn("Add plymouth to HOOKS", str(e.exception))
        self.put("etc/mkinitcpio.conf.d/plymouth.conf", "HOOKS+=(plymouth)\n")  # (a drop-in adding it counts)
        self.splash("orbit")
        self.assertEqual(self.calls[-1], ["mkinitcpio", "-P"])

    def test_themes_come_only_from_setups_copy(self):
        shutil.rmtree(self.root / "usr/lib/irice/plymouth/irice-frost")
        # (a theme folder of that name elsewhere is never used)
        self.put("usr/share/plymouth/themes/irice-frost/irice-frost.plymouth", "[Plymouth Theme]\n")
        with self.assertRaises(self.r.Refused) as e:
            self.splash("frost")
        self.assertIn("Update iRice", str(e.exception))
        lib = self.root / "usr/lib/irice/plymouth/irice-orbit"
        (lib / "run.sh").write_text("#!/bin/sh\n")
        with self.assertRaises(self.r.Refused):
            self.splash("orbit")
        self.assertEqual(self.calls, [])

    def test_setup_copies_only_theme_files(self):
        with tempfile.TemporaryDirectory() as t:
            src = Path(t) / "shell"
            shutil.copytree(ROOT / "shell/assets/plymouth/irice-orbit", src / "assets/plymouth/irice-orbit")
            out = Path(t) / "root"
            self.assertEqual(self.r.install_themes(str(src), os.getuid(), str(out)), [str(out / "usr/lib/irice/plymouth/irice-orbit")])
            self.assertEqual(self.r.install_themes(str(src), os.getuid(), str(out)), [])  # nothing changed
            (src / "assets/plymouth/irice-orbit/notes.txt").write_text("x")
            with self.assertRaises(self.r.Refused):
                self.r.install_themes(str(src), os.getuid(), str(out))

    def test_every_design_is_shipped(self):
        made = sorted(p.name[len("irice-"):] for p in (ROOT / "shell/assets/plymouth").glob("irice-*"))
        self.assertEqual(made, sorted(self.r.BOOTSPLASH_IDS))
        for id in made:
            folder = ROOT / "shell/assets/plymouth" / ("irice-" + id)
            text = (folder / f"irice-{id}.plymouth").read_text()
            self.assertIn(f"ScriptFile=/usr/share/plymouth/themes/irice-{id}/irice-{id}.script", text)
            self.assertIn("ModuleName=script", text)
            self.assertTrue(all(self.r.THEME_FILE.match(p.name) for p in folder.iterdir()))
            # every image the script names is there, in both sizes
            script = (folder / f"irice-{id}.script").read_text()
            for name in set(__import__("re").findall(r'scaled\("([a-z0-9]+)"\)', script)) | set(__import__("re").findall(r'names\[\d+\] = "([a-z0-9]+)"', script)):
                for size in ("@1x", "@2x"):
                    self.assertTrue((folder / f"{name}{size}.png").exists(), f"{id}: {name}{size}.png")
            self.assertEqual(script.count("{"), script.count("}"), id)
            self.assertEqual(script.count("("), script.count(")"), id)
