"""Adapting to what the computer has (rather than assuming kitty, waybar-free
sessions and so on):

- open-app.sh runs a command in whichever terminal is chosen or installed,
  each with its own way to take a command and to hold the window open
- conflicts.py names another bar, notification daemon or a second hypridle
  from what Hyprland and /proc say, and leaves iRice's own alone
- system-stack.sh escapes unit names the way systemd's D-Bus paths want
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "shell/assets/scripts"


def stub(folder, name, body):
    p = Path(folder) / name
    p.write_text("#!/bin/sh\n" + body + "\n")
    p.chmod(0o755)


class TerminalTests(unittest.TestCase):
    def run_in(self, terminal, *args, config=None, env=None):
        with tempfile.TemporaryDirectory() as d:
            bins = Path(d) / "bin"
            bins.mkdir()
            # each fake terminal prints its arguments, one per line
            for t in ("kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "xfce4-terminal", "xterm", "st"):
                stub(bins, t, f'echo {t}; for a in "$@"; do printf "%s\\n" "$a"; done')
            cfg = Path(d) / "cfg"
            cfg.mkdir()
            (cfg / "config.json").write_text(json.dumps(config if config is not None else {"defaultApps": {"terminal": terminal}}))
            e = {"PATH": f"{bins}:/usr/bin:/bin", "IRICE_USER_DIR": str(cfg), "HOME": d}
            e.update(env or {})
            out = subprocess.run(["bash", str(SCRIPTS / "open-app.sh"), "terminal", *args], capture_output=True, text=True, env=e, timeout=10)
            return out.stdout.splitlines()

    def test_each_terminal_takes_the_command_its_own_way(self):
        self.assertEqual(self.run_in("kitty", "--", "htop"), ["kitty", "htop"])
        self.assertEqual(self.run_in("foot", "--", "htop"), ["foot", "htop"])
        self.assertEqual(self.run_in("alacritty", "--", "htop"), ["alacritty", "-e", "htop"])
        self.assertEqual(self.run_in("wezterm", "--", "htop"), ["wezterm", "start", "--", "htop"])
        self.assertEqual(self.run_in("gnome-terminal", "--", "htop"), ["gnome-terminal", "--", "htop"])
        self.assertEqual(self.run_in("xfce4-terminal", "--", "htop"), ["xfce4-terminal", "-x", "htop"])
        self.assertEqual(self.run_in("ghostty", "--", "htop"), ["ghostty", "-e", "htop"])

    def test_hold_uses_the_terminals_flag_or_waits_for_enter(self):
        self.assertEqual(self.run_in("kitty", "--hold", "--", "ls", "a b"), ["kitty", "--hold", "ls", "a b"])
        self.assertEqual(self.run_in("konsole", "--hold", "--", "ls"), ["konsole", "--hold", "-e", "ls"])
        self.assertEqual(self.run_in("xterm", "--hold", "--", "ls"), ["xterm", "-hold", "-e", "ls"])
        held = self.run_in("gnome-terminal", "--hold", "--", "ls", "a b")
        self.assertEqual(held[:4], ["gnome-terminal", "--", "sh", "-c"])
        self.assertIn("read", held[4])
        self.assertEqual(held[5:], ["sh", "ls", "a b"])

    def test_without_a_choice_terminal_env_then_the_first_installed(self):
        self.assertEqual(self.run_in("", "--", "btop", config={}, env={"TERMINAL": "st"}), ["st", "-e", "btop"])
        self.assertEqual(self.run_in("", "--", "btop", config={}), ["kitty", "btop"])

    def test_choice_with_options_keeps_them(self):
        self.assertEqual(self.run_in("foot -o pad=8x8", "--", "nvim"), ["foot", "-o", "pad=8x8", "nvim"])


class ConflictTests(unittest.TestCase):
    def check(self, layers):
        with tempfile.TemporaryDirectory() as d:
            stub(d, "hyprctl", "cat <<'EOF'\n" + json.dumps(layers) + "\nEOF")
            stub(d, "busctl", "exit 1")  # no other notification daemon
            stub(d, "systemctl", "exit 1")
            env = {"PATH": f"{d}:/usr/bin:/bin", "HOME": d, "HYPRLAND_INSTANCE_SIGNATURE": "test-session-not-real"}
            out = subprocess.run(["python3", "-I", str(SCRIPTS / "conflicts.py")], capture_output=True, text=True, env=env, timeout=20)
            return json.loads(out.stdout)

    def test_another_bar_is_named_and_irice_layers_are_not(self):
        layers = {"DP-1": {"levels": {
            "0": [{"namespace": "irice:wallpaper", "pid": 1}],
            "2": [{"namespace": "irice:menubar", "pid": 1}, {"namespace": "waybar", "pid": 999999999}, {"namespace": "waybar", "pid": 999999999}],
            "3": [{"namespace": "hyprpicker", "pid": 999999998}]}}}
        found = self.check(layers)
        self.assertEqual([f["id"] for f in found], ["bar:waybar"])
        self.assertIn("waybar", found[0]["fix"])

    def test_nothing_else_running_is_quiet(self):
        self.assertEqual(self.check({"DP-1": {"levels": {"2": [{"namespace": "irice:dock", "pid": 1}]}}}), [])


class SystemStackTests(unittest.TestCase):
    def test_unit_paths_are_escaped_like_systemd(self):
        out = subprocess.run(["bash", "-c", f'source <(sed -n "/^esc()/,/^}}/p" "{SCRIPTS / "system-stack.sh"}"); esc power-profiles-daemon.service'],
                             capture_output=True, text=True, timeout=10).stdout
        self.assertEqual(out, "power_2dprofiles_2ddaemon_2eservice")

    def test_it_always_answers(self):
        out = subprocess.run(["bash", str(SCRIPTS / "system-stack.sh")], capture_output=True, text=True, timeout=30)
        self.assertEqual(out.returncode, 0)
        keys = {l.split("=", 1)[0] for l in out.stdout.splitlines() if "=" in l}
        self.assertTrue({"net", "powerd", "ppd", "wifi_app"} <= keys)


if __name__ == "__main__":
    unittest.main()
