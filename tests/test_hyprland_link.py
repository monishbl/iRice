"""The shell recovers from a dropped Hyprland event socket
(services/HyprlandLink.qml): it's started by both looks, compares with
hyprctl only after a quiet spell, re-reads the state, and replaces the shell
at most once every few minutes, never in the sandbox."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINK = (ROOT / "shell/services/HyprlandLink.qml").read_text()


class HyprlandLink(unittest.TestCase):
    def test_both_looks_start_it(self):
        for p in ("shell/shell.qml", "material/shell.qml"):
            self.assertIn("HyprlandLink.linked", (ROOT / p).read_text(), p)

    def test_it_checks_only_when_quiet_and_rereads_state(self):
        self.assertIn("onRawEvent", LINK)
        self.assertRegex(LINK, r"Date\.now\(\) - root\.lastEvent > root\.quiet")
        for f in ("refreshMonitors", "refreshWorkspaces", "refreshToplevels"):
            self.assertIn(f"Hyprland.{f}()", LINK)

    def test_repairs_are_rare_and_never_in_the_sandbox(self):
        self.assertRegex(LINK, r'Quickshell\.env\("IRICE_SANDBOX"\) !== "1"')
        every = int(re.search(r"repairEvery:\s*(\d+)", LINK).group(1))
        self.assertGreaterEqual(every, 120000)
        self.assertIn("repair-shell.sh", LINK)


if __name__ == "__main__":
    unittest.main()
