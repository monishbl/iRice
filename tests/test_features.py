"""Tessera's features (shell/assets/scripts/features.py): the one list the
installer's Features step and Settings' "Set up the rest of Tessera" go
by; the root helper's part (tessera-root.py features); and the one-password
run through Software Update (sysupdate.py --features), with a fake pkexec,
pacman and systemctl. Nothing real is read for the plans or changed.
"""
import importlib.util
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"
FEATURES = SCRIPTS / "features.py"
SOURCE = SCRIPTS / "tessera-root.py"
SYSUPDATE = SCRIPTS / "sysupdate.py"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


F = load(FEATURES, "tessera_features_test")
R = load(SOURCE, "tessera_root_features_test")

DELL = {"id": "0a5c:5843", "name": "Broadcom ControlVault 3", "repo": [], "aur": ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"],
        "pam": {"polkit": False, "sudo": False, "login": False}, "driver": True}


def facts(**kw):
    f = {"installed": ["hyprland", "greetd"], "fingerprint": None, "cameras": [], "bluetooth": False, "battery": False,
         "power_manager": "", "display_manager": "", "plymouth_hook": False, "language": "", "ocr": "", "aur_helper": True,
         "features": {}}
    f.update(kw)
    return f


def items(**kw):
    return {i["id"]: i for i in F.plan(facts(**kw))}


class TheList(unittest.TestCase):
    def test_hardware_only_where_it_is(self):
        got = items()
        for gone in ("fingerprint", "face", "bluetooth", "battery", "bootsplash", "ocr"):
            self.assertNotIn(gone, got)
        got = items(fingerprint=DELL, cameras=[{"path": "/dev/v4l/by-path/ir", "name": "Integrated IR camera", "kind": "ir"}],
                    bluetooth=True, battery=True, plymouth_hook=True)
        self.assertEqual(got["fingerprint"]["aur"], DELL["aur"])
        self.assertTrue(got["fingerprint"]["driver"])
        self.assertIn("from the AUR", got["fingerprint"]["detail"])
        self.assertEqual(got["face"]["aur"], ["howdy-next"])
        self.assertEqual(got["face"]["packages"], [])  # (infrared: no OpenCV for a blink)
        self.assertEqual(got["face"]["camera"], "/dev/v4l/by-path/ir")
        for id in ("bluetooth", "battery", "bootsplash"):
            self.assertIn(id, got)
        self.assertFalse(got["battery"]["offer"])  # (the shell turns it on by itself)

    def test_a_regular_camera_needs_opencv_for_the_blink(self):
        got = items(cameras=[{"path": "/dev/video0", "name": "Integrated Webcam", "kind": "rgb"}])
        self.assertEqual(got["face"]["packages"], ["python-opencv"])
        self.assertIn("blinking", got["face"]["detail"])

    def test_set_up_is_told(self):
        fp = dict(DELL, aur=[], pam={"polkit": True, "sudo": True, "login": False})
        got = items(fingerprint=fp, installed=["howdy-next", "yt-dlp", "rclone", "cliphist", "power-profiles-daemon"],
                    cameras=[{"path": "/dev/video2", "name": "IR", "kind": "ir"}], howdy_models=True, ppd_enabled=True,
                    qs_trusted=True, tessera_login=True)
        for id in ("fingerprint", "face", "cast", "clouddrives", "clipboard", "power", "quickshare", "login"):
            self.assertTrue(got[id]["setUp"], id)
        self.assertFalse(got["mirroring"]["setUp"])

    def test_no_energy_modes_next_to_another_power_manager(self):
        self.assertIn("power", items())
        self.assertNotIn("power", items(power_manager="tlp"))

    def test_another_login_screen_is_named(self):
        self.assertIn("SDDM is turned off, and comes back by itself", items(display_manager="SDDM")["login"]["detail"])
        self.assertEqual(items()["login"]["detail"], "Sign in on Tessera's own screen")

    def test_text_recognition_in_your_language(self):
        self.assertEqual(F.locale_language({"LANG": "de_DE.UTF-8"}), ("de", "deu"))
        self.assertEqual(F.locale_language({"LANG": "zh_TW.UTF-8"}), ("zh", "chi_tra"))
        self.assertEqual(F.locale_language({"LANG": "C.UTF-8"}), ("", ""))
        self.assertEqual(F.locale_language({"LC_ALL": "fr_FR.UTF-8", "LANG": "en_US.UTF-8"}), ("fr", "fra"))
        got = items(language="de", ocr="deu")
        self.assertEqual(got["ocr"]["title"], "Text recognition in German")
        self.assertEqual(got["ocr"]["packages"], ["tesseract-data-deu"])
        self.assertNotIn("ocr", items(language="en", ocr="eng"))

    def test_the_aur_helper_only_without_one(self):
        self.assertNotIn("aur", items())
        self.assertIn("aur", items(aur_helper=False))

    def test_every_feature_says_how_to_finish_it(self):
        everything = F.plan(facts(fingerprint=DELL, cameras=[{"path": "/dev/video0", "name": "c", "kind": "ir"}], bluetooth=True,
                                  battery=True, plymouth_hook=True, ocr="deu", language="de", aur_helper=False))
        for i in everything:
            self.assertTrue(i["fix"], i["id"])
            self.assertIn(i["group"], F.GROUPS)
            # (sentence case, no full stop: the screens add their own)
            self.assertFalse(i["title"].endswith("."), i["id"])
            self.assertFalse(i["detail"].endswith("."), i["id"])
        # (the root helper knows every feature with a root part)
        self.assertEqual(F.ROOT_PARTS, set(R.FEATURE_IDS))


class TheOffer(unittest.TestCase):
    def test_missing_leaves_out_declined_set_up_and_not_offered(self):
        f = facts(battery=True, installed=["yt-dlp"], features={"declined": ["phone"]})
        ids = [i["id"] for i in F.missing(F.plan(f), f)]
        self.assertNotIn("phone", ids)      # declined at the install
        self.assertNotIn("cast", ids)       # set up
        self.assertNotIn("battery", ids)    # not offered
        self.assertNotIn("glass", ids)      # Appearance has its own
        self.assertIn("quickshare", ids)

    def test_a_plan_for_the_chosen(self):
        f = facts(fingerprint=DELL, cameras=[{"path": "/dev/v4l/by-path/ir", "name": "IR", "kind": "ir"}], installed=["yt-dlp"])
        p = F.plan_for(F.plan(f), ["fingerprint", "face", "cast", "quickshare", "look", "nope"])
        self.assertEqual(p["ids"], ["fingerprint", "face", "quickshare", "look"])  # (cast is set up; nope isn't one)
        self.assertEqual(p["root"], ["fingerprint", "face", "quickshare"])
        self.assertEqual(p["user"], ["face", "look"])
        self.assertIn("libfprint-tod", p["aur"])
        self.assertIn("howdy-next", p["aur"])
        self.assertIn("darkly-bin", p["aur"])
        self.assertEqual(p["rootArgs"], ["fingerprint,face,quickshare", "--face-camera", "/dev/v4l/by-path/ir", "--fprintd-wait"])
        # (the root helper takes them as they are)
        parsed = R.parse_features(p["rootArgs"])
        self.assertEqual(parsed["ids"], ["fingerprint", "face", "quickshare"])
        self.assertTrue(parsed["fprintd_wait"])

    def test_fprintd_goes_with_the_aurs_libfprint(self):
        fp = dict(DELL, repo=["fprintd"], aur=["libfprint-tod", "libfprint-2-tod1-broadcom"])
        p = F.plan_for(F.plan(facts(fingerprint=fp)), ["fingerprint"])
        self.assertNotIn("fprintd", p["install"])
        self.assertEqual(p["aur"][-1], "fprintd")

    def test_the_login_screen_gets_its_look_and_folder(self):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as t:
            json.dump(facts(installed=[]), t)
        try:
            out = subprocess.run([sys.executable, "-I", str(FEATURES), "--facts", t.name, "plan", "login,cast"],
                                 capture_output=True, text=True, env=dict(os.environ, XDG_CONFIG_HOME=tempfile.gettempdir() + "/nowhere"))
        finally:
            os.unlink(t.name)
        p = json.loads(out.stdout)
        i = p["rootArgs"].index("--login-look")
        look = p["rootArgs"][i + 1]
        self.assertEqual(R.check_login_look(look)["style"], "look")  # (the root helper accepts it)
        self.assertEqual(p["rootArgs"][p["rootArgs"].index("--shell") + 1], str(ROOT / "shell"))
        self.assertEqual(R.parse_features(p["rootArgs"])["shell"], str(ROOT / "shell"))

    def test_what_was_chosen_is_kept(self):
        with tempfile.TemporaryDirectory() as d:
            old = os.environ.get("XDG_CONFIG_HOME")
            os.environ["XDG_CONFIG_HOME"] = d
            try:
                self.assertTrue(F.record(chosen=["cast", "face"], declined=["phone"], offer="done"))
                c = json.loads(Path(d, "tessera/config.json").read_text())
                self.assertEqual(c["features"], {"chosen": ["cast", "face"], "declined": ["phone"], "offer": "done"})
                self.assertEqual(c["welcome"], {"done": False})  # (a new file, as Tessera's first start makes it)
                F.record(chosen=["phone"])
                c = json.loads(Path(d, "tessera/config.json").read_text())
                self.assertEqual(c["features"]["declined"], [])
                self.assertIn("phone", c["features"]["chosen"])
                # (a file that isn't Tessera's settings is left alone)
                Path(d, "tessera/config.json").write_text("[1, 2]")
                self.assertFalse(F.record(offer="pending"))
                self.assertEqual(Path(d, "tessera/config.json").read_text(), "[1, 2]")
            finally:
                if old is None:
                    os.environ.pop("XDG_CONFIG_HOME")
                else:
                    os.environ["XDG_CONFIG_HOME"] = old

    def test_the_command_line(self):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as t:
            json.dump(facts(bluetooth=True), t)
        try:
            run = lambda *a: subprocess.run([sys.executable, "-I", str(FEATURES), "--facts", t.name, *a], capture_output=True, text=True)
            lines = run("list").stdout.splitlines()
            bt = next(l.split("\t") for l in lines if l.startswith("bluetooth\t"))
            self.assertEqual(bt[1:4], ["0", "Hardware", "Bluetooth extras"])
            self.assertEqual(json.loads(run("list", "--json").stdout)["groups"], F.GROUPS)
            self.assertTrue(json.loads(run("missing").stdout)["items"])
            dry = run("apply-user", "look,battery", "--dry-run", "--text")
            self.assertEqual(dry.stdout.splitlines(), ["would set up Tessera's desktop look"])  # (no battery here)
        finally:
            os.unlink(t.name)


class Bluetooth(unittest.TestCase):
    def test_experimental_in_general(self):
        self.assertEqual(R.bt_with_experimental("[General]\n#Experimental = false\nName = x\n[Policy]\nAutoEnable=true\n"),
                         "[General]\nExperimental = true\nName = x\n[Policy]\nAutoEnable=true\n")
        self.assertEqual(R.bt_with_experimental("[General]\nName = x\n\n[Policy]\n"), "[General]\nName = x\n\nExperimental = true\n[Policy]\n")
        self.assertEqual(R.bt_with_experimental("[Policy]\nAutoEnable=true\n"), "[Policy]\nAutoEnable=true\n\n[General]\nExperimental = true\n")
        self.assertEqual(R.bt_with_experimental(""), "[General]\nExperimental = true\n")
        for text in ("[General]\nExperimental = true\n", "[Policy]\n[General]\nExperimental=false\n"):
            self.assertTrue(F.bt_experimental(R.bt_with_experimental(text)))
        self.assertFalse(F.bt_experimental("[Policy]\nExperimental = true\n"))

    def test_main_conf_is_kept_once(self):
        with tempfile.TemporaryDirectory() as d:
            conf = Path(d, "etc/bluetooth/main.conf")
            conf.parent.mkdir(parents=True)
            conf.write_text("[General]\nName = mine\n")
            r = load(SOURCE, "tessera_root_bt")
            r.write_root = lambda dest, data, mode: Path(dest).write_bytes(data)
            r.bluetooth_extras(prefix=d)
            self.assertEqual(Path(str(conf) + ".pre-tessera").read_text(), "[General]\nName = mine\n")
            self.assertIn("Experimental = true", conf.read_text())
            conf.write_text("[General]\nName = changed\n")
            r.bluetooth_extras(prefix=d)
            self.assertEqual(Path(str(conf) + ".pre-tessera").read_text(), "[General]\nName = mine\n")


class RootPart(unittest.TestCase):
    """tessera-root.py features: each on its own, a failure said and the rest going on."""

    def test_arguments_are_checked(self):
        for bad in ([], ["--shell", "/x"], ["rm"], ["face,rm"], ["face", "--face-camera", "/etc/shadow"], ["face", "--bogus"],
                    ["login", "--shell", "relative"]):
            with self.assertRaises(R.Refused, msg=bad):
                R.parse_features(bad)
        p = R.parse_features(["face,face,bluetooth", "--face-camera", "/dev/video2"])
        self.assertEqual(p["ids"], ["face", "bluetooth"])
        self.assertEqual(p["camera"], "/dev/video2")

    def test_one_failing_never_stops_the_rest(self):
        r = load(SOURCE, "tessera_root_features_run")
        did = []
        r.fingerprint = lambda on, where: did.append(("fp", where)) or ["/etc/pam.d/" + where]
        r.face_admin_refresh = lambda run=None: did.append(("face-admin",)) or []
        r.face =lambda args, uid, run=None: (_ for _ in ()).throw(r.Refused("Howdy isn't installed (howdy-next)"))
        r.bluetooth_extras = lambda run=None: did.append(("bt",))
        r.quickshare_trust = lambda on, uid: did.append(("qs", on)) or []
        ran = []

        class Done:
            returncode = 0
        lines = []
        failed = r.features(r.parse_features(["fingerprint,face,bluetooth,quickshare,power", "--face-camera", "/dev/video0", "--fprintd-wait"]),
                            1000, lines.append, run=lambda cmd, **kw: ran.append(cmd) or Done())
        self.assertEqual(failed, 1)
        self.assertIn("@@feature face failed Howdy isn't installed (howdy-next)", lines)
        for ok in ("fingerprint", "bluetooth", "quickshare", "power"):
            self.assertIn(f"@@feature {ok} ok", lines)
        self.assertEqual([d for d in did if d[0] == "fp"], [("fp", "polkit"), ("fp", "sudo"), ("fp", "login")])
        self.assertIn(("face-admin",), did)  # (face admin's copy of polkit's PAM file follows it)
        self.assertIn(R.FIXES["fprintd"], ran)  # (--fprintd-wait: the fix first)
        self.assertIn(["systemctl", "enable", "--now", "power-profiles-daemon"], ran)
        self.assertIn(["systemctl", "enable", "--now", "avahi-daemon"], ran)

    def test_the_login_screen_needs_its_folder_and_look(self):
        lines = []
        self.assertEqual(R.features(R.parse_features(["login"]), 1000, lines.append), 1)
        self.assertTrue(any(l.startswith("@@feature login failed") for l in lines))

    def test_update_takes_them_and_the_broker_runs_them_once(self):
        p = R.parse_update(["--install", "yt-dlp", "--face-camera", "/dev/video0", "--features", "face", "--broker"])
        self.assertEqual(p["features"]["ids"], ["face"])
        self.assertEqual(p["features"]["camera"], "/dev/video0")
        self.assertTrue(p["broker"])
        self.assertIsNone(R.parse_update(["--install", "x"])["features"])
        with self.assertRaises(R.Refused):
            R.parse_update(["--face-camera", "/dev/video0"])
        ours, theirs = socket.socketpair()
        calls = []

        def run_them(say):
            calls.append(1)
            say("@@feature face ok")
            return 0
        t = threading.Thread(target=R.serve, args=(theirs, None, "/bin/false"), kwargs={"on_features": run_them})
        t.start()
        f = ours.makefile("rb")
        answers = []
        for _ in range(2):
            ours.sendall(b'{"features": true}\n')
            text = ""
            for line in f:
                m = json.loads(line)
                text += m.get("out", "")
                if "exit" in m:
                    answers.append((m["exit"], text))
                    break
        ours.shutdown(socket.SHUT_RDWR)
        f.close()
        ours.close()
        t.join(5)
        self.assertEqual(calls, [1])
        self.assertEqual(answers[0], (0, "@@feature face ok\n"))
        self.assertEqual(answers[1][0], 1)  # (a second time is refused)

    def test_the_policy_has_its_action(self):
        policy = (ROOT / "shell/assets/polkit/org.tessera.root.policy").read_text()
        self.assertIn('<action id="org.tessera.root.features">', policy)
        self.assertIn('org.freedesktop.policykit.exec.argv1">features<', policy)
        rules = (ROOT / "shell/assets/polkit/49-tessera.rules").read_text()
        self.assertNotIn("org.tessera.root.features", rules)  # (always the password)


FAKE_PACMAN = r'''#!/usr/bin/env python3
import os, sys
open(os.environ["FAKE_LOG"], "a").write("pacman " + " ".join(sys.argv[1:]) + "\n")
'''
FAKE_SYSTEMCTL = r'''#!/bin/sh
echo "systemctl $*" >> "$FAKE_LOG"
'''
FAKE_PKEXEC = r'''#!/usr/bin/env python3
import os, sys
open(os.environ["FAKE_LOG"], "a").write("pkexec " + " ".join(sys.argv[1:]) + "\n")
args = sys.argv[1:]
i = args.index("update")
code = """
import importlib.util, os, sys
spec = importlib.util.spec_from_file_location("r", sys.argv[1])
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
m.PACMAN = os.environ["FAKE_PACMAN"]
m.PREFIX = os.environ["TESSERA_SYSTEM_ROOT"]
sys.exit(m.update(sys.argv[2:], os.getuid()))
"""
os.execv(sys.executable, [sys.executable, "-c", code, os.environ["HELPER_SOURCE"]] + args[i + 1:])
'''


class OnePassword(unittest.TestCase):
    """sysupdate.py run --features: the packages, then root's part over the
    same pkexec, then yours; each feature's result in the status."""

    def setUp(self):
        if os.path.exists("/var/lib/pacman/db.lck"):
            self.skipTest("a real pacman is running")
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        for name, text in (("pacman", FAKE_PACMAN), ("pkexec", FAKE_PKEXEC), ("systemctl", FAKE_SYSTEMCTL)):
            (self.bin / name).write_text(text)
            (self.bin / name).chmod(0o755)
        self.sysroot = d / "root"
        self.sysroot.mkdir()
        self.run_dir = d / "run"
        self.run_dir.mkdir(mode=0o700)
        self.cfg = d / "config"
        self.log = d / "calls"
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", XDG_RUNTIME_DIR=str(self.run_dir), XDG_CONFIG_HOME=str(self.cfg),
                        TESSERA_SYSTEM_ROOT=str(self.sysroot), FAKE_LOG=str(self.log), FAKE_PACMAN=str(self.bin / "pacman"),
                        HELPER_SOURCE=str(SOURCE), XDG_STATE_HOME=str(d / "state"), XDG_DATA_HOME=str(d / "data"), TESSERA_NO_INHIBIT="1")
        for k in ("TESSERA_FAKE_SYSUPDATE", "TESSERA_SANDBOX"):
            self.env.pop(k, None)

    def tearDown(self):
        self.tmp.cleanup()

    def test_packages_then_root_then_yours(self):
        p = subprocess.run([sys.executable, "-I", str(SYSUPDATE), "run", "--install", "yt-dlp", "--features", "quickshare,cast,power"],
                           env=self.env, capture_output=True, text=True, timeout=120)
        self.assertEqual(p.returncode, 0, p.stderr)
        st = json.loads((self.run_dir / "tessera-sysupdate/status.json").read_text())
        log = (self.run_dir / "tessera-sysupdate/log").read_text()
        self.assertEqual(st["state"], "done", (st, log))
        self.assertEqual(st["kind"], "features")
        self.assertEqual(st["text"], "Set up")
        self.assertEqual(st["steps"][-2:], ["packages", "features"])
        self.assertEqual(st["features"], {"quickshare": {"ok": True, "error": ""}, "power": {"ok": True, "error": ""},
                                          "cast": {"ok": True, "error": ""}})
        calls = self.log.read_text().splitlines()
        pk = [c for c in calls if c.startswith("pkexec ")]
        self.assertEqual(len(pk), 1, calls)  # (one password)
        self.assertIn("--features quickshare,power --broker", pk[0])  # (cast has no part of root's)
        self.assertIn("systemctl enable --now avahi-daemon", calls)
        self.assertTrue((self.sysroot / "etc/tessera/quickshare-trusted").exists())
        c = json.loads((self.cfg / "tessera/config.json").read_text())
        self.assertEqual(c["features"]["chosen"], ["cast", "power", "quickshare"])

    def test_a_failure_is_said_and_the_rest_done(self):
        p = subprocess.run([sys.executable, "-I", str(SYSUPDATE), "run", "--features", "face,quickshare", "--face-camera", "/dev/video9"],
                           env=self.env, capture_output=True, text=True, timeout=120)
        self.assertEqual(p.returncode, 0, p.stderr)
        st = json.loads((self.run_dir / "tessera-sysupdate/status.json").read_text())
        self.assertEqual(st["state"], "done", st)
        self.assertEqual(st["text"], "Set up, 1 didn't finish")
        self.assertFalse(st["features"]["face"]["ok"])
        self.assertTrue(st["features"]["face"]["error"])
        self.assertTrue(st["features"]["quickshare"]["ok"])

    def test_the_sandbox_simulates_it(self):
        env = dict(self.env, TESSERA_FAKE_SYSUPDATE="1")
        p = subprocess.run([sys.executable, "-I", str(SYSUPDATE), "run", "--features", "cast,quickshare,face"], env=env,
                           capture_output=True, text=True, timeout=120)
        self.assertEqual(p.returncode, 0, p.stderr)
        st = json.loads((self.run_dir / "tessera-sysupdate/status.json").read_text())
        self.assertEqual(sorted(st["features"]), ["cast", "face", "quickshare"])
        self.assertFalse(st["features"]["quickshare"]["ok"])  # (the first with a root part: both kinds of result)
        self.assertFalse(self.log.exists())  # (nothing ran)


if __name__ == "__main__":
    unittest.main()
