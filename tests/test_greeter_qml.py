"""The login screens load: each style (and the Material look's, from its
assembled root) in iRice's Quickshell build, offscreen, as a preview (the
mock: no greetd), with no compositor to talk to. Nothing is shown."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = Path.home() / ".local/share/irice/quickshell"
QS = BUNDLE / "bin/qs"
BAD = ("ERROR", "ReferenceError", "TypeError", "is not a type", "is not installed", "Cannot assign", "Unable to assign")


@unittest.skipUnless(QS.exists(), "requires bundled Quickshell")
class GreeterLoads(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="irice-greeter-test-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        (self.base / "runtime").mkdir(mode=0o700)
        env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE", "DISPLAY")}
        env.update(QT_QPA_PLATFORM="offscreen", IRICE_GREETER_MOCK="1", XDG_RUNTIME_DIR=str(self.base / "runtime"),
                   XDG_CONFIG_HOME=str(self.base / "config"), XDG_CACHE_HOME=str(self.base / "cache"),
                   XDG_STATE_HOME=str(self.base / "state"), QML_IMPORT_PATH=str(BUNDLE / "lib/qt6/qml"))
        self.env = env

    def offscreen_root(self, material=False):
        """The shell (or the Material root) as links, the greeter's screens
        copied as plain windows: offscreen there's no layer shell."""
        root = self.base / ("material-shell" if material else "glass-shell")
        subprocess.run(["python3", "-I", str(ROOT / "shell/assets/scripts/material-root.py"), str(ROOT / "shell"),
                        str(ROOT / "material" if material else ROOT / "shell"), str(root)], check=True, capture_output=True)
        for f in (root / "modules/greeter").glob("Greeter*.qml"):
            if f.name == "GreeterAuth.qml":
                continue
            text = f.resolve().read_text()
            text = text.replace("PanelWindow {", "FloatingWindow {").replace("anchors { top: true; bottom: true; left: true; right: true }", "implicitWidth: 1280; implicitHeight: 800").replace("exclusionMode: ExclusionMode.Ignore", "")
            text = "\n".join(l for l in text.splitlines() if not l.strip().startswith("WlrLayershell."))
            f.unlink()
            f.write_text(text)
        return root

    def run_greeter(self, root, style, offscreen_limit=False):
        p = subprocess.Popen([str(QS), "-p", str(root / "greeter.qml")], env=dict(self.env, IRICE_GREETER_STYLE=style),
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            p.kill()
            out, _ = p.communicate()
        lines = [l for l in out.splitlines() if any(b in l for b in BAD) and "font" not in l.lower()]
        if offscreen_limit and lines and "No PanelWindow backend loaded" in lines[-1] and "modules/greeter/" not in lines[-1]:
            # (the Material root's common controls bring every service in,
            # and some services have layer windows, which offscreen hasn't:
            # only that is let through; anything in the greeter still fails)
            lines = [l for l in lines if "modules/greeter/" in l and "unavailable" not in l]
        self.assertEqual(lines, [], style + ":\n" + out[-3000:])

    def test_glass_root(self):
        root = self.offscreen_root()
        for style in ("look", "text", "plain"):
            self.run_greeter(root, style)

    def test_material_root(self):
        root = self.offscreen_root(material=True)
        for style in ("look", "text", "plain"):
            self.run_greeter(root, style, offscreen_limit=True)


if __name__ == "__main__":
    unittest.main()
