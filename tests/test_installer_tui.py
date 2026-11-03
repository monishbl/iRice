"""The interactive installer (tools/installer/tui.py), driven by scripted keys
with fixture hardware: each screen renders, the choices become install.sh's
flags, and install.sh's output becomes the live step list and the summary.
Nothing is installed or run: --script --no-run only prints. Needs
python-rich (skipped without it)."""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TUI = ROOT / "tools/installer/tui.py"
HAS_RICH = importlib.util.find_spec("rich") is not None

FACTS = {
    "cpu": "intel", "laptop": True, "installed": ["linux", "linux-firmware", "papirus-icon-theme"], "power_manager": "",
    "pci": [{"slot": "00:02.0", "cls": "0300", "vendor": "8086", "device": "46a8", "kind": "VGA", "name": "Intel Iris Xe",
             "driver": "i915", "modules": []},
            {"slot": "01:00.0", "cls": "0302", "vendor": "10de", "device": "25a2", "kind": "3D", "name": "NVIDIA GeForce RTX 3050",
             "driver": "nouveau", "modules": []}],
    "fingerprint": {"id": "0a5c:5843", "name": "Broadcom ControlVault 3", "repo": [], "aur": ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"]},
    "ir_camera": None, "screens": [], "monitors_set": False, "hyprland": "0.56.2",
    "pulseaudio": False, "network": "", "display_manager": "", "apps_missing": False, "walls_ready": True,
}


@unittest.skipUnless(HAS_RICH, "python-rich isn't installed")
class Scripted(unittest.TestCase):
    def run_tui(self, keys, facts=None, *args, width=100):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "facts.json"
            f.write_text(json.dumps(dict(FACTS, **(facts or {}))))
            p = subprocess.run([sys.executable, "-I", str(TUI), "--script", keys, "--no-run", "--facts", str(f),
                                "--width", str(width), *args], capture_output=True, text=True, timeout=60)
        self.assertEqual(p.returncode, 0, p.stderr)
        frames = p.stdout.split("\f\n")
        command = next(line for line in p.stdout.splitlines() if line.startswith("COMMAND: "))
        return frames, command.split()[1:]

    def test_the_defaults(self):
        frames, cmd = self.run_tui("enter enter enter enter enter enter enter")
        self.assertIn("Set up iRice", frames[0])
        self.assertIn("Choose a look", frames[1])
        self.assertIn("Review the plan", frames[-2])
        self.assertTrue(cmd[0].endswith("install.sh"))
        self.assertEqual(cmd[1:3], ["--cli", "--yes"])
        for flag in ("--look=glass", "--mode=auto", "--login=look", "--icons=none", "--extras=pointer"):
            self.assertIn(flag, cmd)
        hw = next(a for a in cmd if a.startswith("--hardware="))
        # The fingerprint reader is offered (ticked: you see it and can untick
        # it); NVIDIA's driver never without a tick; builds aren't flags
        self.assertIn("fingerprint", hw)
        self.assertNotIn("gpu-nvidia", hw)
        self.assertNotIn("build-", hw)

    def test_choices_become_flags(self):
        # Material, dark, plain login; tick Colloid (Papirus is installed: shown, not offered)
        frames, cmd = self.run_tui("enter down enter down down enter down down enter space enter space enter")
        self.assertIn("--look=material", cmd)
        self.assertIn("--mode=dark", cmd)
        self.assertIn("--login=plain", cmd)
        self.assertIn("--icons=colloid", cmd)
        self.assertIn("--extras=none", cmd)  # (the pointer unticked)
        icons = next(f for f in frames if "Icon themes (optional)" in f)
        self.assertIn("[•] Papirus", icons)

    def test_nvidia_ticked_by_hand(self):
        keys = "enter " * 6 + "down space enter"  # hardware page: down to NVIDIA, tick it
        frames, cmd = self.run_tui(keys.strip())
        hw = next(a for a in cmd if a.startswith("--hardware="))
        self.assertIn("gpu-nvidia", hw)
        page = next(f for f in frames if "› [ ] NVIDIA driver" in f)  # (the cursor on it: its packages shown)
        self.assertIn("Packages: nvidia-open-dkms", page)

    def test_another_login_screen_is_kept(self):
        frames, cmd = self.run_tui("enter " * 7, {"display_manager": "sddm"})
        self.assertIn("--login=keep", cmd)
        self.assertTrue(any("Keep sddm" in f for f in frames))

    def test_your_system_page_only_when_needed(self):
        frames, cmd = self.run_tui("enter " * 8, {"pulseaudio": True, "network": "systemd-networkd"})
        self.assertTrue(any("Replace PulseAudio with PipeWire" in f for f in frames))
        self.assertIn("--replace-pulseaudio", cmd)
        self.assertIn("--network=networkmanager", cmd)
        frames, cmd = self.run_tui("enter " * 7)
        self.assertFalse(any("Your system" in f for f in frames))
        self.assertFalse(any(a.startswith("--network") or a == "--replace-pulseaudio" for a in cmd))

    def test_passed_on_and_left_out(self):
        _, cmd = self.run_tui("enter " * 5, None, "--dev", "--no-extras", "--no-snapshot", "--bogus")
        self.assertIn("--dev", cmd)
        self.assertIn("--no-snapshot", cmd)
        self.assertNotIn("--bogus", cmd)
        self.assertFalse(any(a.startswith(("--icons", "--extras")) for a in cmd))

    def test_quit_changes_nothing(self):
        p = self.run_tui("enter q")
        self.assertIn("ACTION: quit", "\f\n".join(p[0]))

    def test_narrow_terminal(self):
        frames, _ = self.run_tui("enter", width=60)
        self.assertIn("Look", frames[1])
        self.assertTrue(all(len(line) <= 60 for f in frames for line in f.splitlines() if not line.startswith("COMMAND: ")))


@unittest.skipUnless(HAS_RICH, "python-rich isn't installed")
class Progress(unittest.TestCase):
    """install.sh's output (no colours: it isn't a terminal) as steps."""
    OUTPUT = """
iRice  macOS Tahoe for Hyprland

[1/14] Checking your system
  System: Arch Linux, x86_64
[2/14] Safety snapshot
  No snapper or Timeshift set up: no snapshot (what iRice replaces is backed up as it goes).
[3/14] Installing packages
  Updating the system and installing 90 packages (already installed ones are kept)…
[8/14] For this computer (optional)
  Fingerprint: Broadcom ControlVault 3
  ! Fingerprint: Broadcom ControlVault 3 didn't finish (see above); the rest goes on.
[10/14] Installing iRice
  ✗ Installing iRice didn't finish. Details are in /home/x/.cache/irice/install.log
  ! Skipped (optional).
[✓] Checking the result
  ✓ Hyprland 0.56.2
  ! The Liquid Glass plugin isn't built (glass looks frosted)
      → irice rebuild
  ✓ App icons: the apps' own, styled by iRice

iRice is installed, with problems above.
"""

    def test_steps_checks_and_summary(self):
        spec = importlib.util.spec_from_file_location("irice_tui", TUI)
        tui = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(tui)
        ins = tui.Installer(FACTS, [], {"apps_missing": False, "walls_ready": True})
        ins.page = len(ins.pages) - 1
        for line in self.OUTPUT.splitlines():
            ins.feed(line)
        ins.finish(1)
        states = [(s["title"], s["state"]) for s in ins.steps]
        self.assertEqual(states, [("Checking your system", "ok"), ("Safety snapshot", "ok"), ("Installing packages", "ok"),
                                  ("For this computer (optional)", "ok"), ("Installing iRice", "skipped"),
                                  ("Checking the result", "failed")])
        self.assertIn(("!", "The Liquid Glass plugin isn't built (glass looks frosted)"), ins.checks)
        self.assertIn("Fingerprint: Broadcom ControlVault 3 didn't finish (see above); the rest goes on.", ins.warnings)
        self.assertEqual(ins.result, "iRice is installed, with problems above.")
        from rich.console import Console
        import io
        c = Console(file=io.StringIO(), width=100, color_system=None)
        c.print(ins.render(100, 40))
        c.print(ins.summary("/tmp/install.log"))
        text = c.file.getvalue()
        self.assertIn("Installing iRice: skipped", text)
        self.assertIn("To look at", text)


if __name__ == "__main__":
    unittest.main()
