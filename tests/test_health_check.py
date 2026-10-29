"""System health (shell/assets/scripts/health-check.py) against fake services.

Every command the checks use is a stand-in on PATH whose answer the test
chooses: FAKE_MODE=ok answers like a healthy system, fail like a broken one,
and FAKE_SLOW names one command that hangs. Nothing real is asked.
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "shell/assets/scripts/health-check.py"

# One stand-in for every command: what a healthy system prints, by name and
# arguments. In fail mode each exits 1 with nothing useful.
FAKE = r'''#!/usr/bin/env python3
import os, sys, time
name = os.path.basename(sys.argv[0]); args = " ".join(sys.argv[1:])
log = os.environ.get("FAKE_LOG")
if log:
    open(log, "a").write(name + " " + args + "\n")
if os.environ.get("FAKE_SLOW") == name:
    time.sleep(10)
if os.environ.get("FAKE_MODE") == "fail":
    sys.exit(1)
answers = {
    "pactl": "Server String: /run/pulse\nServer Name: PulseAudio (on PipeWire 1.6)\n",
    "nmcli": "running:connected\n",
    "pgrep": "1234\n",
    "cliphist": "",
    "gpu-screen-recorder": "",
}
if name == "systemctl":
    print("active")
elif name == "busctl":
    if "Powered" in args: print("b true")
    elif "GetVersionString" in args: print('s "avahi 0.9"')
    elif "FileChooser" in args: print("u 4")
    elif "--activatable" in args: print("org.freedesktop.impl.portal.desktop.hyprland\norg.gnome.OnlineAccounts\norg.freedesktop.GeoClue2")
    elif "DaemonVersion" in args: print('s "1.91"')
    elif "ActiveProfile" in args: print('s "balanced"')
    elif "GetConnectionUnixProcessID" in args: print("u " + os.environ.get("FAKE_SHELL_PID", "1"))
    else: sys.exit(1)
elif name == "hyprctl":
    if "irice-glass" in args: print("irice-glass " + os.environ.get("FAKE_PLUGIN", "4"))
    elif "version" in args: print('{"tag": "' + os.environ.get("FAKE_HYPR", "v0.56.2") + '"}')
elif name == "qs":
    print("Quickshell 0.3.2 (revision x)")
else:
    sys.stdout.write(answers.get(name, ""))
'''
COMMANDS = ["pkexec", "pactl", "systemctl", "nmcli", "busctl", "pgrep", "hyprctl", "qs", "cliphist", "gpu-screen-recorder", "wf-recorder"]
SCHEMA = {"id": str, "name": str, "group": str, "status": str, "detail": str, "fix": str, "canFix": bool, "fixRoot": bool, "fixCommand": str}
GROUPS = {"Sound", "Network & sharing", "Power", "Desktop", "iRice itself"}


class HealthCheck(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        fake = self.bin / "fake"
        fake.write_text(FAKE)
        fake.chmod(0o755)
        for c in COMMANDS:
            (self.bin / c).symlink_to(fake)
        self.sysroot = d / "root"  # (/etc and /usr as the sharing checks see them)
        self.home = d / "home"
        (self.home / ".local/state").mkdir(parents=True)
        self.log = d / "calls"
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(self.home), XDG_STATE_HOME=str(self.home / ".local/state"),
                        XDG_RUNTIME_DIR=str(d), IRICE_SYSTEM_ROOT=str(d / "root"), FAKE_MODE="ok", FAKE_SHELL_PID="4242", FAKE_LOG=str(self.log))

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, *args, **env):
        e = dict(self.env, **env)
        out = subprocess.run([sys.executable, "-I", str(SCRIPT), *args, "--shell-pid", "4242"], env=e, capture_output=True, text=True, timeout=20)
        return out

    def rows(self, *args, **env):
        out = self.check(*args, **env)
        self.assertEqual(out.returncode, 0, out.stderr)
        return {r["id"]: r for r in json.loads(out.stdout)}

    def test_healthy_system_is_all_ok_and_matches_the_schema(self):
        rows = self.rows("--polkit", "yes", "--google")
        for r in rows.values():
            for key, kind in SCHEMA.items():
                self.assertIsInstance(r[key], kind, (r["id"], key))
            self.assertIn(r["group"], GROUPS)
            self.assertIn(r["status"], ("ok", "warn", "fail", "off"))
        bad = {k: r for k, r in rows.items() if r["status"] not in ("ok", "off") and k not in ("native", "watchers", "disk")}
        self.assertEqual(bad, {})
        self.assertEqual(rows["notifications"]["status"], "ok")
        self.assertEqual(rows["polkit"]["status"], "ok")
        self.assertEqual(rows["goa"]["status"], "ok")

    def test_broken_services_fail_with_a_fix(self):
        rows = self.rows(FAKE_MODE="fail")
        for k in ("pipewire", "wireplumber", "networkmanager", "avahi", "upower", "profiles", "hypridle", "portal", "plugin"):
            self.assertEqual(rows[k]["status"], "fail", k)
            self.assertNotEqual(rows[k]["fix"], "", k)
        self.assertTrue(rows["pipewire"]["canFix"])
        self.assertTrue(rows["avahi"]["canFix"])
        self.assertTrue(rows["avahi"]["fixRoot"])  # needs the password: confirmed first
        self.assertEqual(rows["avahi"]["fixCommand"], "systemctl enable --now avahi-daemon")
        self.assertFalse(rows["pipewire"]["fixRoot"])
        self.assertIn("sudo systemctl enable --now avahi-daemon", rows["avahi"]["fix"])
        self.assertEqual(rows["polkit"]["status"], "fail")  # not registered
        self.assertEqual(rows["goa"]["status"], "off")      # Google not connected

    def test_a_hung_service_is_reported_and_doesnt_hold_the_rest(self):
        start = time.monotonic()
        rows = self.rows(FAKE_SLOW="nmcli")
        self.assertLess(time.monotonic() - start, 6)
        self.assertEqual(rows["networkmanager"]["status"], "fail")
        self.assertIn("may be hung", rows["networkmanager"]["detail"])
        self.assertEqual(rows["pipewire"]["status"], "ok")

    def test_versions(self):
        rows = self.rows(FAKE_PLUGIN="2", FAKE_HYPR="v0.57.0")
        self.assertEqual(rows["plugin"]["status"], "warn")
        self.assertEqual(rows["hyprland"]["status"], "warn")
        self.assertEqual(self.rows(FAKE_HYPR="v0.56.9")["hyprland"]["status"], "ok")

    def test_notifications_owned_by_another_program_is_a_warning(self):
        rows = self.rows(FAKE_SHELL_PID=str(os.getpid()))  # this test runner, not a Quickshell
        self.assertEqual(rows["notifications"]["status"], "warn")

    def test_fix_runs_only_known_user_level_commands(self):
        out = self.check("fix", "pipewire")
        self.assertEqual(json.loads(out.stdout)["ok"], True)
        calls = self.log.read_text()
        self.assertIn("systemctl --user restart pipewire pipewire-pulse wireplumber", calls)
        # system services only through pkexec, from the fixed list
        out = self.check("fix", "avahi")
        self.assertEqual(json.loads(out.stdout)["ok"], True)
        self.assertIn("pkexec systemctl enable --now avahi-daemon", self.log.read_text())
        out = self.check("fix", "rm -rf /")
        self.assertEqual(json.loads(out.stdout)["ok"], False)
        self.assertNotIn("rm", self.log.read_text())

    def test_firewall_and_pairing(self):
        rows = self.rows()
        self.assertEqual(rows["firewall"]["status"], "ok")  # no firewall here
        self.assertEqual(rows["btpairing"]["status"], "ok")
        (self.sysroot / "etc/ufw").mkdir(parents=True)
        (self.sysroot / "etc/ufw/ufw.conf").write_text("ENABLED=yes\n")
        (self.sysroot / "etc/bluetooth").mkdir(parents=True)
        (self.sysroot / "etc/bluetooth/main.conf").write_text("[General]\nExperimental = true\n")
        rows = self.rows()
        self.assertEqual(rows["firewall"]["status"], "warn")
        self.assertIn("may block", rows["firewall"]["detail"])
        self.assertTrue(rows["firewall"]["fixRoot"])
        self.assertIn("ufw allow 53317:53319/tcp", rows["firewall"]["fixCommand"])
        self.assertEqual(rows["btpairing"]["status"], "warn")
        self.assertIn("--noplugin=rap,vcp,micp,mcp,bass", rows["btpairing"]["fixCommand"])
        # the fix: one pkexec of the fixed script, then the firewall counts as open
        out = self.check("fix", "firewall")
        self.assertTrue(json.loads(out.stdout)["ok"], out.stdout)
        self.assertIn("pkexec /bin/sh -c set -e", self.log.read_text())
        self.assertEqual(self.rows()["firewall"]["status"], "ok")
        # rules readable and without the ports: it blocks (no "may")
        (self.home / ".local/state/irice/sharing-firewall").unlink()
        (self.sysroot / "etc/ufw/user.rules").write_text("-A ufw-user-input -p tcp --dport 22 -j ACCEPT\n")
        self.assertIn("and blocks", self.rows()["firewall"]["detail"])
        # a range, as `ufw allow 53317:53319/tcp` writes it, lets it through
        (self.sysroot / "etc/ufw/user.rules").write_text(
            "-A ufw-user-input -p tcp -m multiport --dports 53317:53319 -j ACCEPT\n")
        self.assertEqual(self.rows()["firewall"]["status"], "ok")
        (self.sysroot / "etc/systemd/system/bluetooth.service.d").mkdir(parents=True)
        dropin = self.sysroot / "etc/systemd/system/bluetooth.service.d/irice-no-rap.conf"
        # the first version left out only rap: phones still asked to pair
        dropin.write_text("[Service]\nExecStart=\nExecStart=/usr/lib/bluetooth/bluetoothd --noplugin=rap\n")
        self.assertEqual(self.rows()["btpairing"]["status"], "warn")
        dropin.write_text("[Service]\nExecStart=\nExecStart=/usr/lib/bluetooth/bluetoothd --noplugin=rap,vcp,micp,mcp,bass\n")
        self.assertEqual(self.rows()["btpairing"]["status"], "ok")

    def test_a_refused_password_says_so(self):
        out = self.check("fix", "avahi", FAKE_MODE="fail")
        self.assertEqual(json.loads(out.stdout)["ok"], False)
        self.assertIn("didn't work", json.loads(out.stdout)["error"])

    def test_text_report_is_scrubbed(self):
        out = self.check("text", FAKE_MODE="fail")
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertIn("need attention", out.stdout)
        self.assertNotIn(str(self.home), out.stdout)


if __name__ == "__main__":
    unittest.main()
