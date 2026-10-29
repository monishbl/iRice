"""Battery & Power helpers: the per-app energy grouping, the critical
battery guard's decision, and the hypridle config start-idle.sh writes from
the Settings timers."""
import importlib.util
import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, file)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


energy = load("app_energy", SCRIPTS / "app-energy.py")
guard = load("battery_guard", SCRIPTS / "battery-guard.py")


class AppEnergy(unittest.TestCase):
    def test_scopes_name_the_app(self):
        self.assertEqual(energy.scope_app("app-com.microsoft.VSCode-1964782.scope"), "com.microsoft.VSCode")
        self.assertEqual(energy.scope_app("app-flatpak-org.gimp.GIMP-123.scope"), "org.gimp.GIMP")
        self.assertEqual(energy.scope_app("app-firefox@abc.service"), "firefox")
        self.assertIsNone(energy.scope_app("session-c3.scope"))

    def test_grouping_by_scope_window_and_top_process(self):
        # pid: (ppid, start, ticks, comm, cgroup)
        procs = {
            10: (1, 1, 0, "Hyprland", "session-c3.scope"),
            20: (10, 2, 0, "zen-bin", "session-c3.scope"),     # owns a window
            21: (20, 3, 0, "Isolated Web Co", "session-c3.scope"),
            30: (1, 4, 0, "code", "app-com.microsoft.VSCode-5.scope"),
            31: (30, 5, 0, "code", "app-com.microsoft.VSCode-5.scope"),
            40: (10, 6, 0, "qs", "session-c3.scope"),
        }
        keys = energy.group(procs, {20: "zen"})
        self.assertEqual(keys[20], "class:zen")
        self.assertEqual(keys[21], "class:zen")  # a helper of the browser counts as the browser
        self.assertEqual(keys[31], "app:com.microsoft.VSCode")
        self.assertEqual(keys[40], "proc:qs")      # below the session: its own
        self.assertEqual(keys[10], "proc:Hyprland")

    def test_deltas_count_growth_and_skip_a_reused_pid(self):
        boot = time.time() - float(open("/proc/uptime").read().split()[0])
        since = time.time() - 300
        tick = energy.TICK
        old_start = int((since - boot - 1000) * tick)  # long before the last look
        new_start = int((since - boot + 10) * tick)    # after it
        prev = {"5": [old_start, 100], "6": [old_start, 50]}
        procs = {
            5: (1, old_start, 160, "a", ""),   # same process: +60
            6: (1, new_start, 30, "b", ""),    # pid reused by a new process: all 30
            7: (1, old_start, 999, "c", ""),   # running before the first look, unseen: skipped
        }
        d = energy.deltas(prev, procs, {5: "app:a", 6: "app:b", 7: "app:c"}, since)
        self.assertEqual(d, {"app:a": 60, "app:b": 30})

    def test_top_keeps_an_hour_and_drops_trivia(self):
        now = 10_000.0
        tick = energy.TICK
        ring = [{"t": now - 4000, "apps": {"app:old": 999 * tick}},
                {"t": now - 600, "apps": {"app:a": 30 * tick, "app:b": 0.5 * tick}}]
        out = energy.top(ring, {"app:a": 10 * tick}, now)
        self.assertEqual(out, [{"key": "app:a", "seconds": 40.0}])


class BatteryGuard(unittest.TestCase):
    def test_sleeps_only_on_battery_at_three_percent(self):
        self.assertTrue(guard.should_sleep(3, guard.DISCHARGING, 3))
        self.assertTrue(guard.should_sleep(1, guard.EMPTY, 3))
        self.assertFalse(guard.should_sleep(4, guard.DISCHARGING, 3))
        self.assertFalse(guard.should_sleep(2, 1, 3))  # charging


class IdleConfig(unittest.TestCase):
    def run_idle(self, listeners, own=None):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)
            (p / "bin").mkdir()
            (p / "config/irice").mkdir(parents=True)
            (p / "config/irice/idle-listeners").write_text(listeners)
            if own is not None:
                (p / "config/hypr").mkdir(parents=True)
                (p / "config/hypr/hypridle.conf").write_text(own)
            stub = p / "bin/hypridle"
            stub.write_text('#!/bin/sh\ncat "$2" > "$IDLE_CAPTURE"\n')
            stub.chmod(0o755)
            env = dict(os.environ, PATH=str(p / "bin") + ":" + os.environ["PATH"], IRICE_SANDBOX="1", IRICE_TTY="1",
                       XDG_RUNTIME_DIR=d, XDG_CONFIG_HOME=str(p / "config"), HYPRLAND_INSTANCE_SIGNATURE="fixture",
                       IDLE_CAPTURE=str(p / "captured"))
            subprocess.run(["bash", str(SCRIPTS / "start-idle.sh"), "qs"], env=env, check=True, timeout=5)
            return (p / "captured").read_text(), (p / "irice-idle-fixture/grace").read_text().strip()

    def test_settings_timers_become_listeners(self):
        conf, grace = self.run_idle("battery dim 120\nac off 600\nbattery sleep 900\nnonsense line\nac lock x\ngrace 0\n")
        self.assertIn("timeout = 120\n    on-timeout = " + str(SCRIPTS / "idle-action.sh") + " battery dim", conf)
        self.assertIn("on-resume = " + str(SCRIPTS / "idle-action.sh") + " any undim", conf)
        self.assertIn("ac off\n    on-resume = " + str(SCRIPTS / "idle-action.sh") + " any on", conf)
        self.assertIn("battery sleep", conf)
        self.assertNotIn("nonsense", conf)
        self.assertNotIn("ac lock x", conf)
        self.assertIn("before_sleep_cmd = qs -p", conf)  # no grace: locked before sleeping
        self.assertEqual(grace, "0")

    def test_a_grace_moves_the_lock_to_the_wake(self):
        conf, grace = self.run_idle("ac lock 300\ngrace 60\n")
        self.assertEqual(grace, "60")
        self.assertIn("idle-action.sh before-sleep", conf)
        self.assertIn("after_sleep_cmd = " + str(SCRIPTS / "idle-action.sh") + " after-sleep", conf)

    def test_your_own_hypridle_conf_still_wins(self):
        conf, _ = self.run_idle("ac sleep 60\n", own="listener {\n timeout = 1234\n}\n")
        self.assertIn("timeout = 1234", conf)
        self.assertNotIn("ac sleep", conf)


if __name__ == "__main__":
    unittest.main()
